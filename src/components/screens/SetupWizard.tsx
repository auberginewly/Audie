// First-run setup wizard — a paneled modal (left numbered steps, right config).
// Steps: Permissions → Hotkey → ASR (required) → LLM (optional), with an optional
// welcome pre-step. Model picks + hotkey write real settings; permission grants
// are mock (real TCC flow is P3). Picked state is local to the wizard demo.

import { useEffect, useState, type ReactNode } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { Hotkey } from "../../types/settings";
import {
  EVENT_STATE_CHANGE,
  EVENT_ERROR,
  StateChangeSchema,
  AppErrorSchema,
  type AppErrorEvent,
} from "../../types/events";
import type { UseSettings } from "../../hooks/useSettings";
import { usePermissions, type PermissionState } from "../../hooks/usePermissions";
import { useConfiguredModels } from "../../hooks/useConfiguredModels";
import { useRecordingStore } from "../../store/recording";
import { Badge, Button, Icon, IconButton, InlineNotice, type IconName } from "../ui";
import { openExternal } from "../../lib/open";
import { HotkeyRecorder } from "../Settings/HotkeyRecorder";
import { ModelConfigDialog } from "../Settings/ModelConfigDialog";
import { MODELS, asrProviderForModelId, llmPickPatch, type ModelMeta } from "../Settings/models";

type StepId = "welcome" | "permissions" | "hotkey" | "asr" | "llm" | "test";

const NUMBERED: StepId[] = ["permissions", "hotkey", "asr", "llm", "test"];
const STEP_LABEL: Record<StepId, string> = {
  welcome: "欢迎",
  permissions: "权限",
  hotkey: "快捷键",
  asr: "听写模型",
  llm: "润色模型",
  test: "试一下",
};

function StepItem({
  index,
  label,
  sub,
  current,
  done,
  onClick,
}: {
  index: number;
  label: string;
  sub: string;
  current: boolean;
  done: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={[
        "flex w-full items-center gap-2.5 rounded-sm border-0 px-2.5 py-2.5 text-left cursor-pointer",
        "transition-colors duration-150 ease-[var(--ease-out)]",
        current ? "bg-gray-alpha-200" : "bg-transparent hover:bg-gray-alpha-100",
      ].join(" ")}
    >
      <span
        className={[
          "inline-flex h-[22px] w-[22px] shrink-0 items-center justify-center rounded-full text-[11px] font-semibold",
          current ? "bg-accent-fill text-surface-card" : "bg-gray-200 text-text-tertiary",
        ].join(" ")}
      >
        {index + 1}
      </span>
      <span className="min-w-0">
        <span
          className={[
            "block text-[13px]",
            current ? "font-medium text-text-primary" : done ? "text-text-primary" : "text-text-secondary",
          ].join(" ")}
        >
          {label}
        </span>
        <span className="mt-px block text-[10px] text-text-tertiary">{sub}</span>
      </span>
      {done ? <Icon name="check" size={15} className="ml-auto shrink-0 text-success-text" /> : null}
    </button>
  );
}

function StepHeader({ title, desc, tag }: { title: string; desc: string; tag?: string }) {
  return (
    <div className="mb-[18px]">
      <div className="flex items-center gap-2.5">
        <h2 className="text-lg font-semibold leading-[1.3] text-text-primary">{title}</h2>
        {tag ? <Badge tone="neutral">{tag}</Badge> : null}
      </div>
      <p className="mt-[7px] max-w-[44ch] text-[13px] leading-[18px] text-text-secondary">{desc}</p>
    </div>
  );
}

function PermItem({
  icon,
  name,
  desc,
  hint,
  state,
}: {
  icon: IconName;
  name: string;
  desc: string;
  hint?: string;
  state: PermissionState;
}) {
  const granted = state.granted === true;
  return (
    <div className="flex items-center gap-3 rounded-md bg-surface-card p-3.5">
      <span className="inline-flex h-[34px] w-[34px] shrink-0 items-center justify-center rounded-sm bg-gray-200 text-text-secondary">
        <Icon name={icon} size={17} />
      </span>
      <div className="min-w-0 flex-1">
        <div className="text-sm font-medium text-text-primary">{name}</div>
        <div className="mt-0.5 text-xs text-text-tertiary">{desc}</div>
        {/* macOS won't re-prompt after a denial; Input Monitoring also only
            reflects a fresh grant after relaunch (P3.9). */}
        {!granted && hint ? <div className="mt-1 text-xs text-warning-text">{hint}</div> : null}
      </div>
      {granted ? (
        <Badge tone="success">已授权</Badge>
      ) : (
        <div className="flex shrink-0 items-center gap-2">
          <Button size="sm" variant="secondary" onClick={state.request}>
            授权
          </Button>
          <Button size="sm" variant="ghost" onClick={state.openSettings}>
            打开系统设置
          </Button>
        </div>
      )}
    </div>
  );
}

function WizModelRow({
  m,
  configured,
  inUse,
  onPick,
  onConfigure,
}: {
  m: ModelMeta;
  configured: boolean;
  inUse: boolean;
  onPick: () => void;
  onConfigure: () => void;
}) {
  return (
    <div className="flex items-center gap-3 rounded-md bg-surface-card px-3.5 py-[13px]">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-text-primary">{m.name}</span>
          <Badge tone="neutral">{m.source === "local" ? "本地" : "云端"}</Badge>
          {inUse ? (
            <Badge tone="accent">使用中</Badge>
          ) : configured ? (
            <Badge tone="success">已配置</Badge>
          ) : (
            <Badge tone="neutral">未配置</Badge>
          )}
        </div>
        <div className="mt-[3px] font-mono text-[11px] text-text-tertiary">{m.model}</div>
      </div>
      {!inUse && configured ? (
        <Button size="sm" variant="secondary" onClick={onPick}>
          选用
        </Button>
      ) : null}
      <Button size="sm" variant="secondary" onClick={onConfigure}>
        配置
      </Button>
    </div>
  );
}

function WelPoint({ title, desc }: { title: string; desc: string }) {
  return (
    <div className="rounded-md bg-surface-card p-3.5">
      <div className="text-sm font-medium text-text-primary">{title}</div>
      <div className="mt-0.5 text-xs text-text-tertiary">{desc}</div>
    </div>
  );
}

type TestPhase = "idle" | "recording" | "processing" | "success";

// "Try it" step: the user focuses the textarea, presses the real trigger (fn) and
// speaks; the dictation pipeline injects into the focused box. Success/failure is
// judged from the Rust state-change/error events — NOT the textarea contents — so
// it stays reliable regardless of where injection focus lands. Reuses the real
// hotkey path; no new backend command.
function TestStep() {
  const [phase, setPhase] = useState<TestPhase>("idle");
  const [err, setErr] = useState<AppErrorEvent | null>(null);

  useEffect(() => {
    const unsubs: UnlistenFn[] = [];
    let cancelled = false;
    const track = (fn: UnlistenFn) => (cancelled ? fn() : unsubs.push(fn));

    listen(EVENT_STATE_CHANGE, (e) => {
      const parsed = StateChangeSchema.safeParse(e.payload);
      if (!parsed.success) return;
      switch (parsed.data.to) {
        case "RECORDING":
          setErr(null);
          setPhase("recording");
          break;
        case "PROCESSING":
          setPhase("processing");
          break;
        case "SUCCESS":
          setPhase("success");
          break;
        case "ERROR":
          setPhase("idle"); // the message arrives via the `error` event below
          break;
        // IDLE (incl. the ~150ms post-SUCCESS settle) leaves "success" shown.
      }
    })
      .then(track)
      .catch((e2) => console.error("test state-change subscribe failed:", e2));

    listen(EVENT_ERROR, (e) => {
      const parsed = AppErrorSchema.safeParse(e.payload);
      if (parsed.success) {
        setErr(parsed.data);
        setPhase("idle");
      }
    })
      .then(track)
      .catch((e2) => console.error("test error subscribe failed:", e2));

    return () => {
      cancelled = true;
      unsubs.forEach((fn) => fn());
    };
  }, []);

  return (
    <>
      <StepHeader
        title="试一下"
        desc="把光标点进下面的输入框，按一下触发键（默认 fn）说句话，松手后文字会插入进去。"
        tag="可选"
      />
      <textarea
        rows={3}
        placeholder="光标点这里，然后按 fn 说话…"
        className="w-full resize-none rounded-md bg-surface-card px-3.5 py-3 text-sm text-text-primary outline-none placeholder:text-text-tertiary focus:ring-1 focus:ring-accent-fill"
      />
      <div className="mt-3 text-xs">
        {phase === "recording" ? (
          <span className="text-accent-text">录音中…</span>
        ) : phase === "processing" ? (
          <span className="text-text-secondary">处理中…</span>
        ) : phase === "success" ? (
          <span className="text-success-text">已插入 ✓ 看到框里出现文字就成了。</span>
        ) : err ? (
          <span className="text-warning-text">
            {err.message}
            {err.code === "permission" ? (
              <button
                className="ml-1 underline"
                onClick={() => openExternal("x-apple.systempreferences:com.apple.preference.security")}
              >
                打开系统设置
              </button>
            ) : (
              "（修好后再按一次 fn 重试）"
            )}
          </span>
        ) : (
          <span className="text-text-tertiary">
            没反应？确认「输入监控」已授权、🌐 键已设为「无操作」，并重启过 Audie。
          </span>
        )}
      </div>
    </>
  );
}

type SetupWizardProps = {
  open: boolean;
  onClose: () => void;
  onComplete?: () => void;
  data: UseSettings;
  welcome?: boolean;
};

export function SetupWizard({ open, onClose, onComplete, data, welcome = true }: SetupWizardProps) {
  const [step, setStep] = useState(0);
  const perms = usePermissions();
  const configuredModels = useConfiguredModels();
  const [pickedAsr, setPickedAsr] = useState<string | null>(null);
  const [pickedLlm, setPickedLlm] = useState<string | null>(null);
  const [configModel, setConfigModel] = useState<ModelMeta | null>(null);
  // 试一下 completion is persistent (a dictation has succeeded) via the recording
  // store, so the checkmark survives reopening the wizard.
  const everSucceeded = useRecordingStore((s) => s.everSucceeded);

  useEffect(() => {
    if (open) setStep(0);
  }, [open, welcome]);
  if (!open) return null;

  const ids: StepId[] = (welcome ? (["welcome"] as StepId[]) : []).concat(NUMBERED);
  const last = ids.length - 1;
  const cur = Math.min(step, last);
  const id = ids[cur];

  const permDone =
    perms.microphone.granted === true &&
    perms.accessibility.granted === true &&
    perms.inputMonitoring.granted === true;
  // ASR step needs a picked model whose key is actually configured (real
  // has_secret), so onboarding can't "complete" with an unusable transcriber.
  const asrDone = !!pickedAsr && configuredModels.configured(pickedAsr);
  // A step is "done" when its own requirement is actually met (not merely passed),
  // so the sidebar checks each step the moment it's complete — current step included.
  const doneMap: Record<string, boolean> = {
    permissions: permDone,
    hotkey: !!data.settings?.hotkey,
    asr: asrDone,
    llm: !!pickedLlm,
    test: everSucceeded,
  };
  const subMap: Record<string, string> = { permissions: "必选", hotkey: "必选", asr: "必选", llm: "可选", test: "可选" };

  const isLast = id === "test";
  const blockNext = id === "asr" && !asrDone;
  const next = () => {
    if (!blockNext) setStep(Math.min(last, cur + 1));
  };
  const back = () => setStep(Math.max(0, cur - 1));

  const pickAsr = (m: ModelMeta) => {
    setPickedAsr(m.id);
    const provider = asrProviderForModelId(m.id);
    if (provider) data.update({ asr_provider: provider });
  };
  const pickLlm = (m: ModelMeta) => {
    setPickedLlm(m.id);
    if (data.settings) data.update(llmPickPatch(m.id, data.settings));
  };

  const asrModels = MODELS.filter((m) => m.type === "asr");
  const llmModels = MODELS.filter((m) => m.type === "llm");

  let body: ReactNode = null;
  if (id === "welcome") {
    body = (
      <>
        <StepHeader title="欢迎使用 Audie" desc="按住快捷键说话 —— 你的话会以文字插入到你正在使用的任何应用里。" />
        <div className="flex flex-col gap-2">
          <WelPoint title="在任意应用里听写" desc="邮件、聊天、文档、代码 —— 哪里有光标，文字就到哪里。" />
          <WelPoint title="可选 AI 润色" desc="去口水话、补标点，或保留逐字原文，由你决定。" />
          <WelPoint title="数据保持私密" desc="音频与文字只在你的设备和你自己的 API 之间流动。" />
        </div>
      </>
    );
  } else if (id === "permissions") {
    body = (
      <>
        <StepHeader title="授予权限" desc="Audie 需要这些权限来录制语音、把文字粘贴到当前应用，并监听触发键。若某项被拒，可在这里再次申请或直接打开系统设置。" tag="必选" />
        <div className="flex flex-col gap-2">
          <PermItem icon="mic" name="麦克风" desc="录制时采集你的语音。" state={perms.microphone} />
          <PermItem icon="command" name="辅助功能" desc="将转写文字粘贴到当前应用。" state={perms.accessibility} />
          <PermItem
            icon="key"
            name="输入监控"
            desc="监听触发键（默认 fn）以开始/结束录音。"
            hint="授权后需重启 Audie 才能生效。"
            state={perms.inputMonitoring}
          />
        </div>
        {/* Default trigger is fn/🌐, which macOS consumes (emoji picker / input
            switch) unless the user reassigns the Globe key. Only nudge when the
            trigger is still the fn default. */}
        {data.settings?.hotkey === "Fn" ? (
          <div className="mt-3">
            <InlineNotice
              tone="warning"
              title="让 fn 键专门触发 Audie"
              action={
                <Button
                  size="sm"
                  variant="secondary"
                  onClick={() => openExternal("x-apple.systempreferences:com.apple.preference.keyboard")}
                >
                  键盘设置
                </Button>
              }
            >
              默认按 🌐(fn) 会弹表情或切换输入法。到「系统设置 → 键盘 → 按下 🌐 键用来」选「无操作」，fn 才能专门触发 Audie。
            </InlineNotice>
          </div>
        ) : null}
      </>
    );
  } else if (id === "hotkey") {
    body = (
      <>
        <StepHeader title="设置快捷键" desc="按住它开始录音，松开插入。点下面的框可录制其它组合键。" tag="必选" />
        <div className="flex items-center justify-between gap-3 rounded-md bg-surface-card p-3.5">
          <div className="min-w-0">
            <div className="text-sm font-medium text-text-primary">录音快捷键</div>
            <div className="mt-0.5 text-xs text-text-tertiary">按住说话 · 松开插入</div>
          </div>
          {data.settings ? (
            <HotkeyRecorder value={data.settings.hotkey} onChange={(h: Hotkey) => data.update({ hotkey: h })} />
          ) : null}
        </div>
      </>
    );
  } else if (id === "asr") {
    body = (
      <>
        <StepHeader title="选择听写模型" desc="Audie 用这个模型把你的语音转写成文字。至少选用一个才能继续。" tag="必选" />
        <div className="flex flex-col gap-2">
          {asrModels.map((m) => (
            <WizModelRow key={m.id} m={m} configured={configuredModels.configured(m.id)} inUse={pickedAsr === m.id} onPick={() => pickAsr(m)} onConfigure={() => setConfigModel(m)} />
          ))}
        </div>
        {!asrDone ? (
          <div className="mt-3 text-xs text-text-tertiary">选用一个已配置的听写模型后继续；未配置的先点「配置」填入 API key。</div>
        ) : null}
      </>
    );
  } else if (id === "llm") {
    body = (
      <>
        <StepHeader title="选择润色模型" desc="插入前先整理转写文本 —— 去口水话、修口误、补标点。不配置则直接插入原文。" tag="可选" />
        <div className="flex flex-col gap-2">
          {llmModels.map((m) => (
            <WizModelRow key={m.id} m={m} configured={configuredModels.configured(m.id)} inUse={pickedLlm === m.id} onPick={() => pickLlm(m)} onConfigure={() => setConfigModel(m)} />
          ))}
        </div>
      </>
    );
  } else {
    body = <TestStep />;
  }

  return (
    <div
      onMouseDown={onClose}
      className="absolute inset-0 z-[70] flex items-center justify-center bg-black/50 p-6 backdrop-blur-[2px]"
    >
      <div
        role="dialog"
        aria-modal="true"
        onMouseDown={(e) => e.stopPropagation()}
        className="relative flex h-[min(520px,100%)] w-[min(780px,100%)] flex-col overflow-hidden rounded-md bg-surface-app shadow-modal"
      >
        <div className="absolute right-2.5 top-2.5 z-10">
          <IconButton name="x" label="关闭" onClick={onClose} />
        </div>

        <div className="flex min-h-0 flex-1">
          <nav className="flex w-[212px] shrink-0 flex-col bg-surface-sidebar px-2.5 pb-2.5 pt-4">
            <div className="px-2.5 pb-3.5">
              <div className="text-sm font-semibold text-text-primary">配置 Audie</div>
              <div className="mt-[3px] text-xs text-text-tertiary">几步即可开始听写。</div>
            </div>
            <div className="flex flex-col gap-0.5">
              {NUMBERED.map((sid, i) => (
                <StepItem
                  key={sid}
                  index={i}
                  label={STEP_LABEL[sid]}
                  sub={subMap[sid]}
                  current={sid === id}
                  done={doneMap[sid] === true}
                  onClick={() => setStep(ids.indexOf(sid))}
                />
              ))}
            </div>
          </nav>

          <div className="flex min-w-0 flex-1 flex-col bg-surface-app">
            <div key={id} className="min-h-0 flex-1 overflow-y-auto px-7 py-[26px] [overscroll-behavior:contain]">
              {body}
            </div>
            <div className="flex shrink-0 items-center gap-2 border-t border-border-subtle px-[18px] py-3.5">
              {cur > 0 ? (
                <Button variant="ghost" onClick={back}>
                  上一步
                </Button>
              ) : null}
              <div className="flex-1" />
              {isLast ? (
                <Button variant="accent" onClick={onComplete ?? onClose}>
                  开始使用 Audie
                </Button>
              ) : (
                <Button variant="accent" disabled={blockNext} onClick={next}>
                  {id === "welcome" ? "开始配置" : "下一步"}
                </Button>
              )}
            </div>
          </div>
        </div>

        <ModelConfigDialog
          model={configModel}
          data={data}
          onClose={() => {
            setConfigModel(null);
            configuredModels.refresh(); // a just-saved key should flip the badge
          }}
        />
      </div>
    </div>
  );
}
