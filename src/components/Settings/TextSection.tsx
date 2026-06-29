// 文本处理 — intent tabs (润色 / 改写 / 写作)，三卡统一成润色卡的结构：一个模型行
// (三模式共享同一个 LLM，点击跳设置) + 一个可折叠的提示词编辑器。写作触发键在「通用」，
// 改写复用主触发键（靠选中态，逻辑见片2）。

import { useState } from "react";

import type { Settings } from "../../types/settings";
import { Badge, Icon, InlineNotice, Segmented, Select, Switch, Textarea } from "../ui";
import { SettingRow } from "./SettingSection";

type Mode = "polish" | "rewrite" | "compose";

const REWRITE_EX = ["翻译成英文", "改得更正式", "精简一下", "修一下语法"];
const COMPOSE_EX = ["写一封请假邮件", "写一条状态同步", "列个周报提纲"];

// "" = follow system locale (backend resolves it). The backend prepends the picked
// label as a line to the prompt, so these read naturally (e.g. "用户主要语言：中文").
const LANGUAGES = ["中文", "English"];

type TextSectionProps = {
  settings: Settings;
  update: (patch: Partial<Settings>) => void;
  onJumpToModelLlm: () => void;
};

export function TextSection({ settings, update, onJumpToModelLlm }: TextSectionProps) {
  const [mode, setMode] = useState<Mode>("polish");
  const [aboutOpen, setAboutOpen] = useState(false);

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
        {mode === "rewrite" ? (
          <Badge tone="neutral" dot>
            即将支持
          </Badge>
        ) : null}
      </div>

      {mode === "polish" ? (
        <div className="overflow-hidden rounded-md bg-surface-card">
          <SettingRow
            label="AI 润色"
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
          <div className="relative">
            <div className="absolute inset-x-3.5 top-0 h-px bg-border-subtle" />
            <button
              onClick={() => setAboutOpen((o) => !o)}
              className="flex w-full items-center gap-2 border-0 bg-transparent px-3.5 py-3 text-left cursor-pointer"
            >
              <Icon name="sparkles" size={14} className="shrink-0 text-text-tertiary" />
              <span className="flex-1 text-[13px] text-text-secondary">AI 润色说明</span>
              <Icon
                name="chevron-down"
                size={15}
                className={["shrink-0 text-text-tertiary transition-transform duration-150", aboutOpen ? "rotate-180" : ""].join(" ")}
              />
            </button>
            {aboutOpen ? (
              <div className="px-3.5 pb-3.5 text-[13px] leading-[18px] text-text-secondary">
                开启后，Audie 会在插入前用你选的 AI 把口述整理干净 —— 去掉「嗯、那个」这类口水话、修正口误、补好标点和分段，你可以用上面的提示词调教它的风格。这一步是可选的：关掉就原样插入转写文字，更快、也不消耗 LLM 额度；万一润色失败，Audie 也会自动退回插入原文，不会丢内容。
              </div>
            ) : null}
          </div>
        </div>
      ) : null}

      {mode === "rewrite" ? (
        <div className="overflow-hidden rounded-md bg-surface-card">
          <ModelRow label="改写模型" model={settings.openai_compatible_model} onJump={onJumpToModelLlm} divider={false} />
          <PromptDisclosure label="改写提示词" value={settings.rewrite_prompt} onChange={(v) => update({ rewrite_prompt: v })} />
          <div className="relative p-3.5">
            <div className="absolute inset-x-3.5 top-0 h-px bg-border-subtle" />
            <div className="mb-2 text-xs text-text-tertiary">可以这样说</div>
            <ExampleChips items={REWRITE_EX} />
            <div className="mt-3">
              <InlineNotice tone="info" icon="info">
                改写功能即将支持：选中文字后按主触发键说出指令，用结果替换选中内容。
              </InlineNotice>
            </div>
          </div>
        </div>
      ) : null}

      {mode === "compose" ? (
        <div className="overflow-hidden rounded-md bg-surface-card">
          <ModelRow label="写作模型" model={settings.openai_compatible_model} onJump={onJumpToModelLlm} divider={false} />
          <PromptDisclosure label="写作提示词" value={settings.compose_prompt} onChange={(v) => update({ compose_prompt: v })} />
          <div className="relative p-3.5">
            <div className="absolute inset-x-3.5 top-0 h-px bg-border-subtle" />
            <div className="mb-2 text-xs text-text-tertiary">可以这样说</div>
            <ExampleChips items={COMPOSE_EX} />
            <div className="mt-3">
              {settings.compose_hotkey ? (
                <InlineNotice tone="info" icon="info">
                  按「通用 → 触发键」里的写作触发键说要点 · 生成的文本插入光标处。
                </InlineNotice>
              ) : (
                <InlineNotice tone="info" icon="info">
                  写作未启用：到「通用 → 触发键」设一个写作触发键即可开启。
                </InlineNotice>
              )}
            </div>
          </div>
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

function ExampleChips({ items }: { items: string[] }) {
  return (
    <div className="flex flex-wrap gap-2">
      {items.map((x) => (
        <span
          key={x}
          className="inline-flex h-[26px] items-center rounded-full bg-gray-200 px-3 text-[13px] text-text-secondary"
        >
          {x}
        </span>
      ))}
    </div>
  );
}
