// Paneled settings modal — its own left nav + content, opened from the sidebar
// gear. Only backed sections appear (Provider / Enhance / Trigger / Config /
// About); the design's unbacked model-picker & device/permission rows are out.

import { useEffect, useState, type ReactNode } from "react";

import { Icon, IconButton, type IconName } from "../ui";
import type { UseSettings } from "../../hooks/useSettings";
import { ProviderSection } from "./ProviderSection";
import { EnhanceSection, TriggerSection, AboutSection } from "./GeneralSections";
import { ConfigSection } from "./ConfigSection";

type SectionDef = { id: string; icon: IconName; label: string; render: (s: UseSettings) => ReactNode };

const SECTIONS: SectionDef[] = [
  {
    id: "provider",
    icon: "cpu",
    label: "服务商",
    render: ({ settings, asrProviders, llmProviders, update }) =>
      settings ? (
        <ProviderSection
          settings={settings}
          asrProviders={asrProviders}
          llmProviders={llmProviders}
          update={update}
        />
      ) : null,
  },
  {
    id: "enhance",
    icon: "sparkles",
    label: "润色",
    render: ({ settings, update }) => (settings ? <EnhanceSection settings={settings} update={update} /> : null),
  },
  {
    id: "trigger",
    icon: "command",
    label: "触发",
    render: ({ settings, update }) => (settings ? <TriggerSection settings={settings} update={update} /> : null),
  },
  {
    id: "config",
    icon: "arrow-down-up",
    label: "配置",
    render: ({ applyImported }) => <ConfigSection onImported={applyImported} />,
  },
  { id: "about", icon: "book", label: "关于", render: () => <AboutSection /> },
];

type SettingsDialogProps = {
  open: boolean;
  onClose: () => void;
  data: UseSettings;
};

export function SettingsDialog({ open, onClose, data }: SettingsDialogProps) {
  const [activeId, setActiveId] = useState(SECTIONS[0].id);

  useEffect(() => {
    if (open) setActiveId(SECTIONS[0].id);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;
  const active = SECTIONS.find((s) => s.id === activeId) ?? SECTIONS[0];

  return (
    <div
      onMouseDown={onClose}
      className="absolute inset-0 z-50 flex items-center justify-center bg-black/50 p-6 backdrop-blur-[2px]"
    >
      <div
        role="dialog"
        aria-modal="true"
        onMouseDown={(e) => e.stopPropagation()}
        className="relative flex h-[min(540px,100%)] w-[min(800px,100%)] flex-col overflow-hidden rounded-md bg-surface-app shadow-modal"
      >
        <div className="absolute right-2.5 top-2.5 z-10">
          <IconButton name="x" label="关闭" onClick={onClose} />
        </div>

        <div className="flex min-h-0 flex-1">
          <nav className="flex w-48 shrink-0 flex-col gap-0.5 overflow-y-auto bg-surface-sidebar p-2.5">
            <div className="px-2.5 pb-2 pt-1 text-sm font-semibold text-text-primary">设置</div>
            {SECTIONS.map((s) => (
              <NavItem
                key={s.id}
                icon={s.icon}
                label={s.label}
                active={s.id === active.id}
                onClick={() => setActiveId(s.id)}
              />
            ))}
          </nav>
          <div
            key={active.id}
            className="min-w-0 flex-1 overflow-y-auto [overscroll-behavior:contain] bg-surface-app px-5 py-5"
          >
            {active.render(data)}
          </div>
        </div>
      </div>
    </div>
  );
}

function NavItem({
  icon,
  label,
  active,
  onClick,
}: {
  icon: IconName;
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      aria-current={active ? "page" : undefined}
      onClick={onClick}
      className={[
        "flex h-8 w-full items-center gap-2.5 rounded-sm border-0 px-2.5 text-left text-[13px]",
        "cursor-pointer transition-colors duration-150 ease-[var(--ease-out)]",
        active
          ? "bg-gray-alpha-200 text-text-primary font-medium"
          : "bg-transparent text-text-secondary hover:bg-gray-alpha-100 hover:text-text-primary",
      ].join(" ")}
    >
      <Icon name={icon} size={16} className={active ? "text-text-secondary" : "text-text-tertiary"} />
      <span className="flex-1">{label}</span>
    </button>
  );
}
