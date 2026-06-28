// Model config dialog — opened from a model card's 配置 button. Body is driven by
// the model id (Doubao / Groq / OpenAI Transcribe / Whisper / LLM cards). Key,
// base_url, model, and asr_model fields write to real backend commands.

import { useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";

import { ProviderTestResultSchema, type SecretKeyId, type Settings } from "../../types/settings";
import type { UseSettings } from "../../hooks/useSettings";
import { Button, IconButton, Input, Select, StatusMessage, type StatusTone } from "../ui";
import {
  asrModelOptionsForModelId,
  llmKeyIdForModelId,
  llmModelIdForBaseUrl,
  llmPresetForModelId,
  requiredSecretsForModel,
  type ModelMeta,
  type ModelOption,
} from "./models";
import { LOCAL_MODEL_RECOMMENDATIONS, type LocalModelRecommendation } from "./localModelRecommendations";

// Cloud ASR cards driven by the generic "模型 + API Key" body: model id written to
// asr_model, key to each provider's own keychain id. doubao keeps its own body
// (resource_id, dual fields); whisper-local is a file path.
const GENERIC_ASR_MODEL_IDS = new Set([
  "groq",
  "openai-asr",
  "glm-asr",
  "aliyun-asr",
  "stepfun-asr",
]);

// ASR cards without a reachable test probe yet — no 测试 button (like whisper-local).
// glm / aliyun_fun / stepfun probes land in a later slice (see types/settings.ts
// TestProviderKeyIdSchema). doubao routes to its dedicated WS connectivity command.
const NO_TEST_BUTTON_IDS = new Set([
  "whisper-local",
  "glm-asr",
  "aliyun-asr",
  "stepfun-asr",
]);

const CUSTOM_OPTION = "__custom__";

// Tauri serializes AppError as { code, message } (error.rs serde tag/content), so
// a rejected command surfaces its user-facing message here.
function errorMessage(err: unknown): string {
  if (err && typeof err === "object" && "message" in err) {
    const m = (err as { message: unknown }).message;
    if (typeof m === "string" && m) return m;
  }
  return typeof err === "string" && err ? err : "测试失败，请查看日志";
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-[7px]">
      <label className="text-[13px] text-text-secondary">{label}</label>
      {children}
    </div>
  );
}

// A model picker over a curated list with a "自定义…" escape hatch. When the saved
// value is off-list (or the user picks 自定义), it falls back to a free-text Input
// so anything outside the curated list still works (the backend takes the raw id).
function OptionSelect({
  options,
  value,
  placeholder,
  onChange,
}: {
  options: { id: string; title: string }[];
  value: string;
  placeholder: string;
  onChange: (next: string) => void;
}) {
  const inList = options.some((o) => o.id === value);
  const [custom, setCustom] = useState(value !== "" && !inList);

  if (custom) {
    return (
      <div className="flex flex-col gap-[7px]">
        <Select
          value={CUSTOM_OPTION}
          onChange={(e) => {
            if (e.target.value !== CUSTOM_OPTION) {
              setCustom(false);
              onChange(e.target.value);
            }
          }}
        >
          {options.map((o) => (
            <option key={o.id} value={o.id}>
              {o.title}
            </option>
          ))}
          <option value={CUSTOM_OPTION}>自定义…</option>
        </Select>
        <Input
          mono
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
        />
      </div>
    );
  }

  return (
    <Select
      value={inList ? value : (options[0]?.id ?? "")}
      onChange={(e) => {
        if (e.target.value === CUSTOM_OPTION) {
          setCustom(true);
          return;
        }
        onChange(e.target.value);
      }}
    >
      {options.map((o) => (
        <option key={o.id} value={o.id}>
          {o.title}
        </option>
      ))}
      <option value={CUSTOM_OPTION}>自定义…</option>
    </Select>
  );
}

// A keychain-backed password field, standard "masked input + eye toggle". The
// stored key is loaded into the field on open (masked, eye closed) so it sits
// right in the row; the eye toggles show/hide. With stable app signing, reading
// the app's own keychain item doesn't re-prompt. Editing overwrites; save()
// persists a non-empty value (clearing leaves the stored key untouched).
function KeyInput({ keyId, placeholder }: { keyId: SecretKeyId; placeholder: string }) {
  const [value, setValue] = useState("");
  const [revealed, setRevealed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    invoke("get_secret_for_settings", { keyId })
      .then((raw) => {
        if (!cancelled && typeof raw === "string") setValue(raw);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [keyId]);

  return (
    <div className="relative">
      <Input
        mono
        type={revealed ? "text" : "password"}
        value={value}
        onChange={(e) => setValue(e.target.value)}
        placeholder={placeholder}
        data-key-id={keyId}
        className="pr-9"
      />
      <div className="absolute inset-y-0 right-0 flex items-center pr-0.5">
        <IconButton
          name={revealed ? "eye" : "eye-off"}
          label={revealed ? "隐藏" : "查看"}
          size="sm"
          onClick={() => setRevealed((r) => !r)}
        />
      </div>
    </div>
  );
}

// Recommended local models by RAM tier (Ollama tags). Shown only for local LLM
// cards. Each row is clickable to fill the model field — no auto-detection of the
// host's RAM (out of scope), the user picks the tier matching their machine.
function RecommendedLocalModels({ onPick }: { onPick: (tag: string) => void }) {
  // Group by RAM tier, preserving declaration order (主推 first within each tier).
  const tiers = LOCAL_MODEL_RECOMMENDATIONS.reduce<{ ram: string; items: LocalModelRecommendation[] }[]>(
    (acc, rec) => {
      const tier = acc.find((t) => t.ram === rec.ram);
      if (tier) tier.items.push(rec);
      else acc.push({ ram: rec.ram, items: [rec] });
      return acc;
    },
    [],
  );
  return (
    <div className="flex flex-col gap-[7px]">
      <label className="text-[13px] text-text-secondary">推荐模型（按内存，点击填入）</label>
      <div className="flex flex-col gap-2">
        {tiers.map((tier) => (
          <div key={tier.ram} className="overflow-hidden rounded-sm border border-border-subtle">
            <div className="bg-surface-card px-2.5 py-1 text-[11px] font-medium text-text-tertiary">
              {tier.ram}
            </div>
            {tier.items.map((rec) => (
              <button
                key={rec.ram + rec.name}
                type="button"
                onClick={() => onPick(rec.tag)}
                className="flex w-full flex-col gap-0.5 border-t border-border-subtle px-2.5 py-2 text-left hover:bg-gray-alpha-100"
              >
                <div className="flex items-baseline gap-2">
                  <span
                    className={
                      rec.primary
                        ? "text-[12px] font-medium text-text-primary"
                        : "text-[12px] text-text-secondary"
                    }
                  >
                    {rec.name}
                  </span>
                  {rec.primary ? (
                    <span className="shrink-0 rounded border border-border-subtle px-1 text-[10px] text-text-secondary">
                      主推
                    </span>
                  ) : null}
                  <span className="ml-auto shrink-0 font-mono text-[11px] text-text-tertiary">{rec.tag}</span>
                </div>
                <span className="text-[11px] text-text-tertiary">{rec.note}</span>
              </button>
            ))}
          </div>
        ))}
      </div>
      <StatusMessage tone="neutral" icon={null}>
        Qwen3 主推（中文最佳），Gemma / Granite 更快更省但中文偏弱。用 ollama pull &lt;tag&gt; 下载，或在 LM Studio 搜模型名。思考型已自动 /no_think + 输出剥离。
      </StatusMessage>
    </div>
  );
}

type ModelConfigDialogProps = {
  model: ModelMeta | null;
  data: UseSettings;
  onClose: () => void;
};

export function ModelConfigDialog({ model, data, onClose }: ModelConfigDialogProps) {
  const [status, setStatus] = useState<{ tone: StatusTone; message: string } | null>(null);
  const { settings, update } = data;

  // LLM endpoint + model shown/edited in the dialog. All LLM cards share one
  // backend slot (openai_compatible), so we seed from the saved slot ONLY when
  // this card is the active provider; otherwise from the card's own preset — so
  // opening Kimi shows api.moonshot.cn, not whatever the shared slot holds.
  // Saving applies the draft (switches the slot to this provider), instead of
  // writing live on open (which would leave the new endpoint paired with the old
  // provider's key).
  const [llmDraft, setLlmDraft] = useState({ baseUrl: "", model: "" });
  // Live model list fetched from the provider's /models (null = use curated list).
  const [liveModels, setLiveModels] = useState<ModelOption[] | null>(null);
  const [modelFetch, setModelFetch] = useState<{ tone: StatusTone; message: string } | null>(null);
  useEffect(() => {
    if (!model || model.type !== "llm" || !settings) return;
    const preset = llmPresetForModelId(model.id);
    const active = llmModelIdForBaseUrl(settings.openai_compatible_base_url) === model.id;
    setLlmDraft({
      baseUrl: active ? settings.openai_compatible_base_url : preset.baseUrl,
      // Leave the model blank for a not-yet-active card — no guessed/hardcoded
      // model. The user fetches the live list (获取最新模型) or types one in.
      model: active ? settings.openai_compatible_model : "",
    });
    setLiveModels(null); // dropping the previous card's fetched list
    setModelFetch(null);
    // Re-seed only when the opened card changes, not on every settings write.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [model?.id]);

  // Local providers (Ollama / LM Studio) need no key and run on a fixed localhost
  // endpoint, so auto-fetch their model list on open (and auto-select the first) —
  // the user just hits 保存. Cloud cards stay manual (need a key first).
  useEffect(() => {
    if (!model || model.type !== "llm" || model.source !== "local") return;
    let cancelled = false;
    const baseUrl = llmPresetForModelId(model.id).baseUrl;
    setModelFetch({ tone: "pending", message: "获取中…" });
    invoke("list_provider_models", { baseUrl, apiKey: null })
      .then((raw) => {
        if (cancelled) return;
        const ids = Array.isArray(raw) ? raw.filter((x): x is string => typeof x === "string") : [];
        if (!ids.length) {
          setModelFetch({ tone: "danger", message: "未获取到模型，请确认本地服务在运行" });
          return;
        }
        setLiveModels(ids.map((id) => ({ id, title: id })));
        setLlmDraft((d) => (d.model.trim() ? d : { ...d, model: ids[0] }));
        setModelFetch({ tone: "success", message: `已获取 ${ids.length} 个模型` });
      })
      .catch((err) => {
        if (!cancelled) setModelFetch({ tone: "danger", message: errorMessage(err) });
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [model?.id]);

  if (!model || !settings) return null;

  // Persist every keychain field currently rendered (reads the DOM inputs the
  // KeyInput components own). For an LLM card, also apply its endpoint+model draft
  // to the shared openai_compatible slot — so saving Kimi's config switches the
  // backend to Kimi's endpoint, not just stores a key under DeepSeek's endpoint.
  const save = async (): Promise<boolean> => {
    // An LLM provider with no model can't polish (it becomes 使用中 but errors at
    // request time). Block saving until a model is fetched/typed.
    if (model.type === "llm" && !llmDraft.model.trim()) {
      setStatus({ tone: "danger", message: "请先「获取最新模型」或手动填写模型名" });
      return false;
    }
    const inputs = document.querySelectorAll<HTMLInputElement>("input[data-key-id]");
    try {
      if (model.type === "llm") {
        update({
          llm_provider: "openai_compatible",
          openai_compatible_base_url: llmDraft.baseUrl.trim(),
          openai_compatible_model: llmDraft.model.trim(),
          // 4b: bind the backend to this provider's own key slot.
          llm_api_key_id: llmKeyIdForModelId(model.id) ?? "",
          // Remember this provider's chosen model so 选用 can restore it later.
          llm_models: { ...(settings.llm_models ?? {}), [model.id]: llmDraft.model.trim() },
        });
      }
      let savedKey = false;
      for (const el of inputs) {
        const keyId = el.getAttribute("data-key-id");
        const val = el.value.trim();
        if (!keyId) continue;
        if (val) {
          await invoke("set_secret", { keyId, value: val });
          savedKey = true;
        }
      }
      // Only claim the keychain when a key was actually written (local providers
      // store no key — just endpoint/model).
      setStatus({ tone: "success", message: savedKey ? "已保存到系统 keychain" : "已保存" });
      return true;
    } catch {
      setStatus({ tone: "danger", message: "保存失败，请查看日志" });
      return false;
    }
  };

  // Live connection test (replaces the old mock). Doubao is WebSocket-only so it
  // routes to a dedicated command; the rest probe /models via test_provider. Keys
  // come from the visible input, else the saved keychain value (user-initiated, so
  // a keychain prompt is acceptable).
  const readKey = async (keyId: SecretKeyId): Promise<string | null> => {
    const el = document.querySelector<HTMLInputElement>(`input[data-key-id="${keyId}"]`);
    const typed = el?.value.trim();
    if (typed) return typed;
    try {
      const raw = await invoke("get_secret_for_settings", { keyId });
      return typeof raw === "string" && raw ? raw : null;
    } catch {
      return null;
    }
  };

  // Fetch the live model list from this card's endpoint /models (4b refresh button).
  // Falls back silently to the curated list on failure (liveModels stays null).
  const refreshModels = async () => {
    setModelFetch({ tone: "pending", message: "获取中…" });
    try {
      const keyId = llmKeyIdForModelId(model.id);
      const apiKey = keyId ? await readKey(keyId) : null;
      const raw = await invoke("list_provider_models", { baseUrl: llmDraft.baseUrl, apiKey });
      const ids = Array.isArray(raw) ? raw.filter((x): x is string => typeof x === "string") : [];
      if (!ids.length) {
        setModelFetch({ tone: "danger", message: "未返回模型，已保留内置列表" });
        return;
      }
      setLiveModels(ids.map((id) => ({ id, title: id })));
      // Auto-select the first fetched model when none is chosen yet, so the dropdown's
      // displayed value is the ACTUAL llmDraft.model (otherwise it shows the first
      // option but the draft stays "" → save is blocked / persists empty).
      setLlmDraft((d) => (d.model.trim() ? d : { ...d, model: ids[0] }));
      setModelFetch({ tone: "success", message: `已获取 ${ids.length} 个模型` });
    } catch (err) {
      setModelFetch({ tone: "danger", message: errorMessage(err) });
    }
  };

  const runTest = async () => {
    setStatus({ tone: "pending", message: "测试中…" });
    const startedAt = performance.now();
    try {
      let raw: unknown;
      if (model.id === "doubao") {
        raw = await invoke("test_doubao_connection");
      } else if (model.id === "groq") {
        raw = await invoke("test_provider", {
          request: {
            kind: "asr",
            provider_id: "groq",
            key_id: "groq_api_key",
            api_key: await readKey("groq_api_key"),
            base_url: null,
          },
        });
      } else if (model.id === "openai-asr") {
        raw = await invoke("test_provider", {
          request: {
            kind: "asr",
            provider_id: "openai",
            key_id: "openai_api_key",
            api_key: await readKey("openai_api_key"),
            base_url: null,
          },
        });
      } else {
        // LLM card: test THIS card's endpoint (draft) with its own key (4b), not
        // the saved shared slot — so testing Kimi probes api.moonshot.cn + Kimi's key.
        const llmKeyId = llmKeyIdForModelId(model.id);
        raw = await invoke("test_provider", {
          request: {
            kind: "llm",
            provider_id: "openai_compatible",
            key_id: llmKeyId ?? "openai_compatible_api_key",
            api_key: llmKeyId ? await readKey(llmKeyId) : null,
            base_url: llmDraft.baseUrl,
          },
        });
      }
      const ms = Math.round(performance.now() - startedAt);
      const parsed = ProviderTestResultSchema.safeParse(raw);
      const base = parsed.success ? parsed.data.message : "连接测试通过";
      setStatus({ tone: "success", message: `${base} · ${ms}ms` });
    } catch (err) {
      setStatus({ tone: "danger", message: errorMessage(err) });
    }
  };

  const setField = (patch: Partial<Settings>) => update(patch);

  return (
    <div
      onMouseDown={onClose}
      className="absolute inset-0 z-[90] flex items-center justify-center bg-black/55 p-8 backdrop-blur-[3px]"
    >
      <div
        role="dialog"
        aria-modal="true"
        onMouseDown={(e) => e.stopPropagation()}
        className="flex max-h-full w-[min(460px,100%)] flex-col overflow-hidden rounded-lg bg-surface-overlay shadow-modal"
      >
        <div className="flex shrink-0 items-center gap-2.5 px-[18px] pb-3.5 pt-[18px]">
          <div className="min-w-0 flex-1 text-base font-semibold tracking-[-0.32px] text-text-primary">
            配置 {model.name}
          </div>
          <IconButton name="x" label="关闭" onClick={onClose} />
        </div>

        <div className="flex flex-col gap-4 overflow-y-auto px-[18px] pb-1 pt-0.5">
          <ModelBody
            model={model}
            settings={settings}
            setField={setField}
            llmDraft={llmDraft}
            setLlmDraft={setLlmDraft}
            liveModels={liveModels}
            modelFetch={modelFetch}
            onRefreshModels={refreshModels}
          />
        </div>

        <div className="flex shrink-0 items-center gap-2 px-[18px] py-4">
          {!NO_TEST_BUTTON_IDS.has(model.id) ? (
            <Button variant="secondary" disabled={status?.tone === "pending"} onClick={runTest}>
              测试
            </Button>
          ) : null}
          {status ? (
            <StatusMessage tone={status.tone} icon={null}>
              {status.message}
            </StatusMessage>
          ) : null}
          <div className="flex-1" />
          <Button variant="ghost" onClick={onClose}>
            取消
          </Button>
          <Button
            variant="accent"
            onClick={async () => {
              if (await save()) onClose();
            }}
          >
            保存
          </Button>
        </div>
      </div>
    </div>
  );
}

function ModelBody({
  model,
  settings,
  setField,
  llmDraft,
  setLlmDraft,
  liveModels,
  modelFetch,
  onRefreshModels,
}: {
  model: ModelMeta;
  settings: Settings;
  setField: (patch: Partial<Settings>) => void;
  llmDraft: { baseUrl: string; model: string };
  setLlmDraft: (next: { baseUrl: string; model: string }) => void;
  liveModels: ModelOption[] | null;
  modelFetch: { tone: StatusTone; message: string } | null;
  onRefreshModels: () => void;
}) {
  if (model.id === "doubao") {
    // Doubao's model variant is selected via Resource ID (not asr_model), so there
    // is no model dropdown here — Resource ID below is the real control.
    return (
      <>
        <Field label="App ID">
          <KeyInput keyId="doubao_app_id" placeholder="旧版控制台 App ID" />
        </Field>
        <Field label="Access Token">
          <KeyInput keyId="doubao_access_token" placeholder="粘贴 Access Token" />
        </Field>
        <Field label="Endpoint">
          <Input
            mono
            defaultValue={settings.doubao_endpoint}
            onChange={(e) => setField({ doubao_endpoint: e.target.value })}
            placeholder="wss://openspeech.bytedance.com/…"
          />
        </Field>
        <Field label="Resource ID">
          <Input
            mono
            defaultValue={settings.doubao_resource_id}
            onChange={(e) => setField({ doubao_resource_id: e.target.value })}
            placeholder="volc.seedasr.sauc.duration"
          />
        </Field>
      </>
    );
  }

  if (GENERIC_ASR_MODEL_IDS.has(model.id)) {
    // Each ASR card declares exactly one keychain secret in requiredSecretsForModel,
    // so it's the single source of truth for which key this body writes.
    const keyId = requiredSecretsForModel(model.id)[0];
    const options = asrModelOptionsForModelId(model.id);
    return (
      <>
        <Field label="模型">
          <OptionSelect
            options={options}
            value={settings.asr_model}
            placeholder={options[0]?.id ?? ""}
            onChange={(asr_model) => setField({ asr_model })}
          />
        </Field>
        {keyId ? (
          <Field label="API Key">
            <KeyInput keyId={keyId} placeholder="粘贴 API Key" />
          </Field>
        ) : null}
      </>
    );
  }

  if (model.id === "whisper-local") {
    return (
      <Field label="模型文件路径">
        <Input
          mono
          defaultValue={settings.whisper_cpp_model_path ?? ""}
          onChange={(e) => setField({ whisper_cpp_model_path: e.target.value || null })}
          placeholder="/path/to/ggml-large-v3.bin"
        />
      </Field>
    );
  }

  // LLM cards — all drive the single openai_compatible slot. The endpoint+model
  // come from llmDraft (seeded per-card in the parent: this card's preset unless
  // it's the active provider), and save() applies the draft. Editing here only
  // touches the draft, so opening a card never silently switches the live slot.
  const preset = llmPresetForModelId(model.id);
  // 4b: each cloud LLM card writes/reads its own keychain key; local cards
  // (Ollama / LM Studio) have no key id (key-optional localhost endpoint).
  const llmKeyId = llmKeyIdForModelId(model.id);
  return (
    <>
      <Field label="端点">
        <Input
          mono
          value={llmDraft.baseUrl}
          onChange={(e) => setLlmDraft({ ...llmDraft, baseUrl: e.target.value })}
          placeholder={preset.baseUrl}
        />
      </Field>
      {/* Local providers (Ollama / LM Studio) need no API key — omit the field. */}
      {llmKeyId ? (
        <Field label="API Key">
          <KeyInput keyId={llmKeyId} placeholder="粘贴 API Key" />
        </Field>
      ) : null}
      {/* Custom field: 获取最新模型 sits on the 模型 label line (right), not below. */}
      <div className="flex flex-col gap-[7px]">
        <div className="flex items-center justify-between gap-2">
          <label className="text-[13px] text-text-secondary">模型</label>
          <button
            type="button"
            disabled={modelFetch?.tone === "pending"}
            onClick={onRefreshModels}
            className="text-[12px] text-accent hover:underline disabled:opacity-50"
          >
            获取最新模型
          </button>
        </div>
        {/* No hardcoded model list: a dropdown only once /models is fetched, else a
            free-text field the user fills in (or fetches via 获取最新模型). */}
        {liveModels && liveModels.length ? (
          <OptionSelect
            options={liveModels}
            value={llmDraft.model}
            placeholder="选择模型"
            onChange={(value) => setLlmDraft({ ...llmDraft, model: value })}
          />
        ) : (
          <Input
            mono
            value={llmDraft.model}
            onChange={(e) => setLlmDraft({ ...llmDraft, model: e.target.value })}
            placeholder={
              llmKeyId
                ? "填 API Key 后点「获取最新模型」，或手动填写"
                : "点「获取最新模型」，或手动填写"
            }
          />
        )}
        {modelFetch ? (
          <StatusMessage tone={modelFetch.tone} icon={null}>
            {modelFetch.message}
          </StatusMessage>
        ) : null}
      </div>
      {/* Local cards (Ollama / LM Studio): suggest RAM-tier-matched tags so the
          user doesn't have to know which Ollama model fits their machine. */}
      {model.source === "local" ? (
        <RecommendedLocalModels
          onPick={(tag) => setLlmDraft({ ...llmDraft, model: tag })}
        />
      ) : null}
    </>
  );
}
