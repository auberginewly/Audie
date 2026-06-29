// 文本处理 — intent tabs (润色 / 改写 / 写作)，三卡统一成润色卡的结构：模型行
// (三模式共享同一个 LLM，点击跳设置) + 提示词折叠 + 说明折叠。写作触发键在「通用」，
// 改写复用主触发键（靠选中态，逻辑见片2）。

import { useState } from "react";

import type { Settings } from "../../types/settings";
import { Icon, Segmented, Select, Switch, Textarea } from "../ui";
import { SettingRow } from "./SettingSection";

type Mode = "polish" | "rewrite" | "compose";

// "" = follow system locale (backend resolves it). The backend prepends the picked
// label as a line to the prompt, so these read naturally (e.g. "用户主要语言：中文").
const LANGUAGES = ["中文", "English"];

const POLISH_NOTE =
  "打开「AI 润色」后，Audie 会在插入前用上面的模型把口述整理干净 —— 去掉「嗯、那个」这类口水话、修正口误、补好标点和分段，可以用上面的提示词调教风格（需先配好 LLM 模型）。关掉开关、或没配模型，就原样插入转写文字（更快、不消耗额度）。万一润色失败，也会自动退回插入原文、不丢内容。";
const COMPOSE_NOTE =
  "写作不插入逐字稿，而是把你的口述要点交给上面的模型生成成稿 —— 比如说「写一封请假邮件」「列个周报提纲」，光标处就会出现写好的文本。先到「通用 → 触发键」设一个写作触发键，按它说要点即可；生成失败会退回插入你的原话。";
const REWRITE_NOTE =
  "改写是先选中已有文字，再按「润色 / 改写触发键」说出指令（比如「翻译成英文」「改得更正式」「精简一下」），AI 按指令改写选中内容并替换。没选中文字时按该键则走润色。";

type TextSectionProps = {
  settings: Settings;
  update: (patch: Partial<Settings>) => void;
  onJumpToModelLlm: () => void;
};

export function TextSection({ settings, update, onJumpToModelLlm }: TextSectionProps) {
  const [mode, setMode] = useState<Mode>("polish");

  return (
    <section className="mb-7">
      <div className="mb-3 flex items-start gap-2.5 pl-1">
        <Icon name="sparkles" size={16} className="mt-px text-text-tertiary" />
        <h2 className="text-sm font-semibold leading-5 tracking-[-0.28px] text-text-primary">文本处理</h2>
      </div>

      <div className="mb-3 flex flex-wrap items-center gap-2.5">
        <Segmented
          value={mode}
          onChange={setMode}
          options={[
            { id: "polish", label: "润色" },
            { id: "rewrite", label: "改写" },
            { id: "compose", label: "写作" },
          ]}
        />
      </div>

      {mode === "polish" ? (
        <div className="overflow-hidden rounded-md bg-surface-card">
          <SettingRow
            label="AI 润色"
            description="关掉只插入语音转写原文，不经 AI 整理"
            divider={false}
            control={<Switch checked={settings.enhance_enabled} onChange={(v) => update({ enhance_enabled: v })} />}
          />
          <ModelRow label="润色模型" model={settings.openai_compatible_model} onJump={onJumpToModelLlm} />
          <SettingRow
            label="主语言"
            description="润色按此语言整理，并保留口述里的混合语言"
            control={
              <div className="w-40">
                <Select value={settings.primary_language} onChange={(e) => update({ primary_language: e.target.value })}>
                  <option value="">跟随系统</option>
                  {LANGUAGES.map((lang) => (
                    <option key={lang} value={lang}>
                      {lang}
                    </option>
                  ))}
                </Select>
              </div>
            }
          />
          <PromptDisclosure label="润色提示词" value={settings.enhance_prompt} onChange={(v) => update({ enhance_prompt: v })} />
          <NoteDisclosure label="AI 润色说明" text={POLISH_NOTE} />
        </div>
      ) : null}

      {mode === "rewrite" ? (
        <div className="overflow-hidden rounded-md bg-surface-card">
          <ModelRow label="改写模型" model={settings.openai_compatible_model} onJump={onJumpToModelLlm} divider={false} />
          <PromptDisclosure label="改写提示词" value={settings.rewrite_prompt} onChange={(v) => update({ rewrite_prompt: v })} />
          <NoteDisclosure label="改写说明" text={REWRITE_NOTE} />
        </div>
      ) : null}

      {mode === "compose" ? (
        <div className="overflow-hidden rounded-md bg-surface-card">
          <ModelRow label="写作模型" model={settings.openai_compatible_model} onJump={onJumpToModelLlm} divider={false} />
          <PromptDisclosure label="写作提示词" value={settings.compose_prompt} onChange={(v) => update({ compose_prompt: v })} />
          <NoteDisclosure label="写作说明" text={COMPOSE_NOTE} />
        </div>
      ) : null}
    </section>
  );
}

// 当前在用的 LLM 模型行（润色 / 改写 / 写作共享同一个 openai_compatible 模型）。点击跳设置 → 模型 → LLM。
function ModelRow({
  label,
  model,
  onJump,
  divider = true,
}: {
  label: string;
  model: string;
  onJump: () => void;
  divider?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onJump}
      className="relative flex w-full cursor-pointer items-center justify-between gap-4 border-0 bg-transparent px-3.5 py-3 text-left transition-colors hover:bg-gray-alpha-100"
    >
      {divider ? <div className="absolute inset-x-3.5 top-0 h-px bg-border-subtle" /> : null}
      <span className="text-sm text-text-primary">{label}</span>
      <span className="flex shrink-0 items-center gap-2">
        <Icon name="sparkles" size={13} className="text-aubergine-900" />
        <span className="font-mono text-[13px] text-text-secondary">{model}</span>
        <Icon name="chevron-right" size={14} className="text-text-tertiary" />
      </span>
    </button>
  );
}

// 可折叠提示词编辑器（默认收起 + 单行预览）。自带 open state，三卡各一个、互不影响。
function PromptDisclosure({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="relative">
      <div className="absolute inset-x-3.5 top-0 h-px bg-border-subtle" />
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="flex w-full items-center gap-2 border-0 bg-transparent px-3.5 py-3 text-left cursor-pointer"
      >
        <span className="shrink-0 text-[13px] text-text-secondary">{label}</span>
        {open ? (
          <span className="flex-1" />
        ) : (
          <span className="min-w-0 flex-1 truncate text-[13px] text-text-tertiary">{value}</span>
        )}
        <Icon
          name="chevron-down"
          size={15}
          className={["shrink-0 text-text-tertiary transition-transform duration-150", open ? "rotate-180" : ""].join(" ")}
        />
      </button>
      {open ? (
        <div className="px-3.5 pb-3.5">
          <Textarea value={value} onChange={(e) => onChange(e.target.value)} className="min-h-[120px]" />
        </div>
      ) : null}
    </div>
  );
}

// 可折叠的功能说明（默认收起）。三卡共用，模仿原「AI 润色说明」那一行。
function NoteDisclosure({ label, text }: { label: string; text: string }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="relative">
      <div className="absolute inset-x-3.5 top-0 h-px bg-border-subtle" />
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="flex w-full items-center gap-2 border-0 bg-transparent px-3.5 py-3 text-left cursor-pointer"
      >
        <Icon name="sparkles" size={14} className="shrink-0 text-text-tertiary" />
        <span className="flex-1 text-[13px] text-text-secondary">{label}</span>
        <Icon
          name="chevron-down"
          size={15}
          className={["shrink-0 text-text-tertiary transition-transform duration-150", open ? "rotate-180" : ""].join(" ")}
        />
      </button>
      {open ? <div className="px-3.5 pb-3.5 text-[13px] leading-[18px] text-text-secondary">{text}</div> : null}
    </div>
  );
}
