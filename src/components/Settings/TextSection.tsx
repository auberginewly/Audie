// 文本处理 — intent tabs (润色 / 改写 / 创作). Polish is wired to the real
// enhance settings; rewrite & compose are mock previews of planned modes
// (the design's chips + notice), with local enable toggles (see plan).

import { useState } from "react";

import type { Settings } from "../../types/settings";
import { Badge, Icon, InlineNotice, Segmented, Select, Switch, Textarea } from "../ui";
import { HotkeyRecorder } from "./HotkeyRecorder";
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
  const [rewriteOn, setRewriteOn] = useState(false);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [promptOpen, setPromptOpen] = useState(false);
  const [composePromptOpen, setComposePromptOpen] = useState(false);

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
            探索中
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
          <button
            type="button"
            onClick={onJumpToModelLlm}
            className="relative flex w-full cursor-pointer items-center justify-between gap-4 border-0 bg-transparent px-3.5 py-3 text-left transition-colors hover:bg-gray-alpha-100"
          >
            <div className="absolute inset-x-3.5 top-0 h-px bg-border-subtle" />
            <span className="text-sm text-text-primary">润色模型</span>
            <span className="flex shrink-0 items-center gap-2">
              <Icon name="sparkles" size={13} className="text-aubergine-900" />
              <span className="font-mono text-[13px] text-text-secondary">{settings.openai_compatible_model}</span>
              <Icon name="chevron-right" size={14} className="text-text-tertiary" />
            </span>
          </button>
          <SettingRow
            label="主语言"
            description="润色按此语言整理，并保留口述里的混合语言"
            control={
              <div className="w-40">
                <Select
                  value={settings.primary_language}
                  onChange={(e) => update({ primary_language: e.target.value })}
                >
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
          <div className="relative">
            <div className="absolute inset-x-3.5 top-0 h-px bg-border-subtle" />
            <button
              onClick={() => setPromptOpen((o) => !o)}
              className="flex w-full items-center gap-2 border-0 bg-transparent px-3.5 py-3 text-left cursor-pointer"
            >
              <span className="shrink-0 text-[13px] text-text-secondary">润色提示词</span>
              {promptOpen ? (
                <span className="flex-1" />
              ) : (
                <span className="min-w-0 flex-1 truncate text-[13px] text-text-tertiary">{settings.enhance_prompt}</span>
              )}
              <Icon
                name="chevron-down"
                size={15}
                className={["shrink-0 text-text-tertiary transition-transform duration-150", promptOpen ? "rotate-180" : ""].join(" ")}
              />
            </button>
            {promptOpen ? (
              <div className="px-3.5 pb-3.5">
                <Textarea
                  value={settings.enhance_prompt}
                  onChange={(e) => update({ enhance_prompt: e.target.value })}
                  className="min-h-[100px]"
                />
              </div>
            ) : null}
          </div>
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
        <MockModeCard
          enabled={rewriteOn}
          onToggle={setRewriteOn}
          body="选中文字后按住快捷键说出指令，Audie 用结果替换选中的内容。"
          examples={REWRITE_EX}
          note="需先选中文字 · 替换选中内容"
        />
      ) : null}

      {mode === "compose" ? (
        <div className="overflow-hidden rounded-md bg-surface-card">
          <SettingRow
            label="启用写作"
            divider={false}
            control={
              <Switch
                checked={settings.compose_enabled}
                onChange={(v) => update({ compose_enabled: v })}
              />
            }
          />
          <SettingRow
            label="写作触发键"
            description="独立于主触发键；按它说出要点，生成的文本插入光标处"
            control={
              <HotkeyRecorder
                value={settings.compose_hotkey}
                onChange={(h) => update({ compose_hotkey: h })}
              />
            }
          />
          <div className="relative">
            <div className="absolute inset-x-3.5 top-0 h-px bg-border-subtle" />
            <button
              onClick={() => setComposePromptOpen((o) => !o)}
              className="flex w-full items-center gap-2 border-0 bg-transparent px-3.5 py-3 text-left cursor-pointer"
            >
              <span className="shrink-0 text-[13px] text-text-secondary">写作提示词</span>
              {composePromptOpen ? (
                <span className="flex-1" />
              ) : (
                <span className="min-w-0 flex-1 truncate text-[13px] text-text-tertiary">{settings.compose_prompt}</span>
              )}
              <Icon
                name="chevron-down"
                size={15}
                className={["shrink-0 text-text-tertiary transition-transform duration-150", composePromptOpen ? "rotate-180" : ""].join(" ")}
              />
            </button>
            {composePromptOpen ? (
              <div className="px-3.5 pb-3.5">
                <Textarea
                  value={settings.compose_prompt}
                  onChange={(e) => update({ compose_prompt: e.target.value })}
                  className="min-h-[120px]"
                />
              </div>
            ) : null}
          </div>
          <div className="relative p-3.5">
            <div className="absolute inset-x-3.5 top-0 h-px bg-border-subtle" />
            <div className="mb-2 text-xs text-text-tertiary">可以这样说</div>
            <div className="mb-3 flex flex-wrap gap-2">
              {COMPOSE_EX.map((x) => (
                <span
                  key={x}
                  className="inline-flex h-[26px] items-center rounded-full bg-gray-200 px-3 text-[13px] text-text-secondary"
                >
                  {x}
                </span>
              ))}
            </div>
            {settings.compose_enabled && !settings.compose_hotkey ? (
              <InlineNotice tone="info" icon="info">
                已启用写作，但还没设置写作触发键
              </InlineNotice>
            ) : (
              <InlineNotice tone="info" icon="info">
                按写作触发键说要点 · 在光标处插入生成的文本
              </InlineNotice>
            )}
          </div>
        </div>
      ) : null}
    </section>
  );
}

// mock: rewrite/compose pipelines aren't implemented — local toggle only.
function MockModeCard({
  enabled,
  onToggle,
  body,
  examples,
  note,
}: {
  enabled: boolean;
  onToggle: (v: boolean) => void;
  body: string;
  examples: string[];
  note: string;
}) {
  return (
    <div className="overflow-hidden rounded-md bg-surface-card">
      <SettingRow label="启用此模式" divider={false} control={<Switch checked={enabled} onChange={onToggle} />} />
      <div className="relative p-3.5">
        <div className="absolute inset-x-3.5 top-0 h-px bg-border-subtle" />
        <div className="mb-3 text-[13px] leading-[18px] text-text-secondary">{body}</div>
        <div className="mb-2 text-xs text-text-tertiary">可以这样说</div>
        <div className="mb-3 flex flex-wrap gap-2">
          {examples.map((x) => (
            <span
              key={x}
              className="inline-flex h-[26px] items-center rounded-full bg-gray-200 px-3 text-[13px] text-text-secondary"
            >
              {x}
            </span>
          ))}
        </div>
        <InlineNotice tone="info" icon="info">
          {note}
        </InlineNotice>
      </div>
    </div>
  );
}
