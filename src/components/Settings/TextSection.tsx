// 文本处理 — intent tabs (润色 / 改写 / 创作). Polish is wired to the real
// enhance settings; rewrite & compose are mock previews of planned modes
// (the design's chips + notice), with local enable toggles (see plan).

import { useState } from "react";

import type { Settings } from "../../types/settings";
import { Badge, Icon, InlineNotice, Segmented, Switch, Textarea } from "../ui";
import { SettingRow } from "./SettingSection";

type Mode = "polish" | "rewrite" | "compose";

const REWRITE_EX = ["翻译成英文", "改得更正式", "精简一下", "修一下语法"];
const COMPOSE_EX = ["写一封请假邮件", "写一条状态同步", "列个周报提纲"];

type TextSectionProps = {
  settings: Settings;
  update: (patch: Partial<Settings>) => void;
};

export function TextSection({ settings, update }: TextSectionProps) {
  const [mode, setMode] = useState<Mode>("polish");
  const [rewriteOn, setRewriteOn] = useState(false);
  const [composeOn, setComposeOn] = useState(false);
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
        {mode !== "polish" ? (
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
          <SettingRow
            label="润色模型"
            control={
              <div className="flex items-center gap-2">
                <Icon name="sparkles" size={13} className="text-aubergine-900" />
                <span className="text-[13px] text-text-primary">OpenAI-compatible</span>
                <span className="font-mono text-[11px] text-text-tertiary">{settings.openai_compatible_model}</span>
              </div>
            }
          />
          <div className="relative px-3.5 pb-3.5 pt-3">
            <div className="absolute inset-x-3.5 top-0 h-px bg-border-subtle" />
            <div className="mb-1.5 text-[13px] text-text-secondary">润色提示词</div>
            <Textarea value={settings.enhance_prompt} onChange={(e) => update({ enhance_prompt: e.target.value })} />
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
                转写完成后，你选用的 LLM 会按上面的提示词改写原文 —— 去掉口水话、修正口误、补上标点，再插入到光标处。若润色失败，Audie 会退回插入转写原文并告知你。
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
        <MockModeCard
          enabled={composeOn}
          onToggle={setComposeOn}
          body="不再插入逐字稿，而是按你的口述简述生成内容。"
          examples={COMPOSE_EX}
          note="在光标处插入生成的文本"
        />
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
