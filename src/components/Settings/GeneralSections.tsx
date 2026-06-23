// 润色 · 触发 · 关于 — the remaining backed settings sections. Polish toggle +
// prompt, hotkey preset, and a static About block (version + repo links).

import { HOTKEY_PRESETS, type Hotkey, type Settings } from "../../types/settings";
import { Icon, Select, Switch, Textarea } from "../ui";
import { SettingSection, SettingRow } from "./SettingSection";

// macOS keycaps read ⌃⇧⌥, not "Ctrl/Shift/Alt" — show the printed symbols.
const HOTKEY_LABELS: Record<Hotkey, string> = {
  "Ctrl+Shift+Space": "⌃ ⇧ Space",
  "Alt+Space": "⌥ Space",
  "Ctrl+Alt+Space": "⌃ ⌥ Space",
};

const REPO_URL = "https://github.com/auberginewly/Audie";

type SectionProps = {
  settings: Settings;
  update: (patch: Partial<Settings>) => void;
};

export function EnhanceSection({ settings, update }: SectionProps) {
  return (
    <SettingSection icon="sparkles" title="文本润色" description="把口语整理成干净的书面表达，再插入光标处">
      <SettingRow
        label="AI 润色"
        description="关闭时插入转写原文"
        divider={false}
        control={<Switch checked={settings.enhance_enabled} onChange={(v) => update({ enhance_enabled: v })} />}
      />
      <div className="relative px-3.5 pb-3.5 pt-3">
        <div className="absolute inset-x-3.5 top-0 h-px bg-border-subtle" />
        <div className="mb-1.5 text-[13px] text-text-secondary">润色 Prompt</div>
        <Textarea
          value={settings.enhance_prompt}
          onChange={(e) => update({ enhance_prompt: e.target.value })}
        />
      </div>
    </SettingSection>
  );
}

export function TriggerSection({ settings, update }: SectionProps) {
  return (
    <SettingSection icon="command" title="触发" description="按住快捷键说话，松手插入文字">
      <SettingRow
        label="快捷键"
        divider={false}
        control={
          <div className="w-44">
            <Select value={settings.hotkey} onChange={(e) => update({ hotkey: e.target.value as Hotkey })}>
              {HOTKEY_PRESETS.map((preset) => (
                <option key={preset} value={preset}>
                  {HOTKEY_LABELS[preset]}
                </option>
              ))}
            </Select>
          </div>
        }
      />
    </SettingSection>
  );
}

export function AboutSection() {
  return (
    <SettingSection icon="book" title="关于" description="开源 · BYOK · 音频与 key 只在你的设备和你的 API 之间流动">
      <SettingRow label="项目仓库" divider={false} control={<ExtLink href={REPO_URL}>auberginewly/Audie</ExtLink>} />
      <SettingRow label="反馈" control={<ExtLink href={`${REPO_URL}/issues`}>GitHub Issues</ExtLink>} />
      <SettingRow label="作者" control={<ExtLink href="https://auberginewly.vercel.app">auberginewly</ExtLink>} />
    </SettingSection>
  );
}

function ExtLink({ href, children }: { href: string; children: string }) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noreferrer"
      className="inline-flex items-center gap-1.5 text-sm text-text-secondary no-underline transition-colors hover:text-text-primary"
    >
      <span>{children}</span>
      <Icon name="arrow-up-right" size={14} className="text-text-tertiary" />
    </a>
  );
}
