// Dev-only gallery to verify the design-system foundation renders against the
// Geist tokens (dark + aubergine accent). Not shipped to users; mounted from
// App.tsx under import.meta.env.DEV. Mirrors the design's guidelines cards.

import { useState, type ReactNode } from "react";
import {
  Badge,
  Button,
  Card,
  Icon,
  IconButton,
  Input,
  KeyCombo,
  Keycap,
  Select,
  Switch,
  Textarea,
  ICON_NAMES,
  type ButtonVariant,
} from "../components/ui";
import { AppShell, AppSidebar, ShellHeader } from "../components/shell";

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="mb-8">
      <h2 className="mb-3 font-mono text-xs uppercase tracking-[0.04em] text-text-tertiary">{title}</h2>
      <Card>{children}</Card>
    </section>
  );
}

function Swatch({ name, className }: { name: string; className: string }) {
  return (
    <div className="flex flex-col gap-1.5">
      <div className={["h-12 rounded-sm border border-border-subtle", className].join(" ")} />
      <span className="font-mono text-[11px] text-text-tertiary">{name}</span>
    </div>
  );
}

const BUTTON_VARIANTS: ButtonVariant[] = ["primary", "secondary", "accent", "danger", "ghost"];

export function DesignSystemPreview() {
  const [enhance, setEnhance] = useState(true);
  const [lang, setLang] = useState("zh");

  return (
    <div className="h-screen w-screen">
      <AppShell
        sidebar={<AppSidebar active="home" version="0.4.1" settingsLabel="设置" />}
        header={<ShellHeader title="Design System" subtitle="地基预览 · Geist tokens + aubergine" />}
      >
        <Section title="Surfaces">
          <div className="grid grid-cols-4 gap-3">
            <Swatch name="surface-app" className="bg-surface-app" />
            <Swatch name="surface-sidebar" className="bg-surface-sidebar" />
            <Swatch name="surface-card" className="bg-surface-card" />
            <Swatch name="surface-inset" className="bg-surface-inset" />
            <Swatch name="gray-200" className="bg-gray-200" />
            <Swatch name="gray-300" className="bg-gray-300" />
            <Swatch name="gray-400" className="bg-gray-400" />
            <Swatch name="gray-1000" className="bg-gray-1000" />
          </div>
        </Section>

        <Section title="Accent · State">
          <div className="grid grid-cols-4 gap-3">
            <Swatch name="accent-fill" className="bg-accent-fill" />
            <Swatch name="aubergine-900" className="bg-aubergine-900" />
            <Swatch name="success-fill" className="bg-success-fill" />
            <Swatch name="warning-fill" className="bg-warning-fill" />
            <Swatch name="danger-fill" className="bg-danger-fill" />
          </div>
        </Section>

        <Section title="Typography">
          <div className="space-y-2">
            <p className="text-[32px] font-semibold tracking-[-1.28px] text-text-primary">Heading 32 · 安静的系统工具</p>
            <p className="text-2xl font-semibold tracking-[-0.96px] text-text-primary">Heading 24</p>
            <p className="text-xl font-semibold tracking-[-0.4px] text-text-primary">Heading 20</p>
            <p className="text-sm text-text-secondary">Copy 14 — 按住快捷键说话，松手后文字出现在光标处。</p>
            <p className="font-mono text-[13px] text-text-tertiary">Geist Mono · whisper-large-v3-turbo · ⌃⇧Space</p>
          </div>
        </Section>

        <Section title="Buttons">
          <div className="flex flex-wrap items-center gap-2">
            {BUTTON_VARIANTS.map((v) => (
              <Button key={v} variant={v}>
                {v}
              </Button>
            ))}
            <Button variant="accent" icon="mic">
              测试录音
            </Button>
            <Button variant="secondary" size="sm" iconRight="chevron-right">
              小号
            </Button>
            <Button variant="secondary" disabled>
              禁用
            </Button>
            <IconButton name="settings" label="设置" />
            <IconButton name="x" label="关闭" variant="outline" />
          </div>
        </Section>

        <Section title="Badges · Keycaps">
          <div className="flex flex-wrap items-center gap-2">
            <Badge tone="neutral">中性</Badge>
            <Badge tone="accent" dot>
              使用中
            </Badge>
            <Badge tone="success" icon="check">
              已连接
            </Badge>
            <Badge tone="warning" dot>
              待处理
            </Badge>
            <Badge tone="danger" icon="alert">
              已拒绝
            </Badge>
            <KeyCombo keys={["ctrl", "shift", "space"]} />
            <Keycap>fn</Keycap>
          </div>
        </Section>

        <Section title="Form controls">
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <Input placeholder="API key" />
              <Input mono placeholder="https://api.example.com/v1" />
              <Input invalid placeholder="无效输入" />
            </div>
            <div className="space-y-2">
              <Select value={lang} onChange={(e) => setLang(e.target.value)}>
                <option value="zh">简体中文</option>
                <option value="en">English</option>
              </Select>
              <div className="flex items-center gap-3">
                <Switch checked={enhance} onChange={setEnhance} />
                <span className="text-sm text-text-secondary">AI 润色</span>
                <Switch checked={false} onChange={() => {}} size="sm" />
              </div>
            </div>
          </div>
          <div className="mt-3">
            <Textarea defaultValue="把口语整理成书面表达，保留原意，去掉口头禅。" />
          </div>
        </Section>

        <Section title="Icons">
          <div className="flex flex-wrap gap-3 text-text-secondary">
            {ICON_NAMES.map((n) => (
              <span key={n} className="flex flex-col items-center gap-1" title={n}>
                <Icon name={n} size={18} />
              </span>
            ))}
          </div>
        </Section>
      </AppShell>
    </div>
  );
}
