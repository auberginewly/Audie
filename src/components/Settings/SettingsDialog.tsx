// Paneled settings modal — its own left nav + content, opened from the sidebar
// gear. Restored to the design's IA: 模型 · 文本处理 · 通用 · 关于. Backed paths
// wire to real commands; unbacked rows are mock (see each section + plan).

import { useEffect, useState, type ReactNode } from "react";

import { Icon, IconButton, type IconName } from "../ui";
import type { UseSettings } from "../../hooks/useSettings";
import { ModelSection } from "./ModelSection";
import type { ModelType } from "./models";
import { TextSection } from "./TextSection";
import { GeneralSection } from "./GeneralSection";
import { AboutSection } from "./AboutSection";
import { useI18n, type I18nKey } from "../../i18n";

interface SectionCtx {
  onRerunSetup: () => void;
  modelType: ModelType;
  setModelType: (t: ModelType) => void;
  goToModelLlm: () => void;
}
interface SectionDef {
  id: string;
  icon: IconName;
  labelKey: I18nKey;
  render: (s: UseSettings, ctx: SectionCtx) => ReactNode;
}

const SECTIONS: SectionDef[] = [
  {
    id: "model",
    icon: "cpu",
    labelKey: "settings.tabs.model",
    render: (data, { modelType, setModelType }) => <ModelSection data={data} type={modelType} onType={setModelType} />,
  },
  {
    id: "text",
    icon: "sparkles",
    labelKey: "settings.tabs.text",
    render: ({ settings, update }, { goToModelLlm }) =>
      settings ? <TextSection settings={settings} update={update} onJumpToModelLlm={goToModelLlm} /> : null,
  },
  {
    id: "general",
    icon: "sliders",
    labelKey: "settings.tabs.general",
    render: ({ settings, update, microphones, autoDevice }) =>
      settings ? (
        <GeneralSection settings={settings} update={update} microphones={microphones} autoDevice={autoDevice} />
      ) : null,
  },
  {
    id: "about",
    icon: "book",
    labelKey: "settings.tabs.about",
    render: (_data, { onRerunSetup }) => <AboutSection onRerunSetup={onRerunSetup} />,
  },
];

interface SettingsDialogProps {
  open: boolean;
  onClose: () => void;
  data: UseSettings;
  onRerunSetup: () => void;
}

export function SettingsDialog({ open, onClose, data, onRerunSetup }: SettingsDialogProps) {
  const { t } = useI18n();
  const [activeId, setActiveId] = useState(SECTIONS[0].id);
  const [modelType, setModelType] = useState<ModelType>("asr");

  useEffect(() => {
    if (open) {
      setActiveId(SECTIONS[0].id);
      setModelType("asr");
    }
  }, [open]);

  // 文本处理「润色模型」行 → 跳到 模型 tab 的 LLM 子页。
  const goToModelLlm = () => {
    setActiveId("model");
    setModelType("llm");
  };

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
    };
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
        onMouseDown={(e) => {
          e.stopPropagation();
        }}
        className="relative flex h-[min(540px,100%)] w-[min(800px,100%)] flex-col overflow-hidden rounded-md bg-surface-app shadow-modal"
      >
        <div className="absolute right-2.5 top-2.5 z-10">
          <IconButton name="x" label={t("settings.close")} onClick={onClose} />
        </div>

        <div className="flex min-h-0 flex-1">
          <nav className="flex w-48 shrink-0 flex-col gap-0.5 overflow-y-auto bg-surface-sidebar p-2.5">
            <div className="px-2.5 pb-2 pt-1 text-sm font-semibold text-text-primary">{t("settings.title")}</div>
            {SECTIONS.map((s) => (
              <NavItem
                key={s.id}
                icon={s.icon}
                label={t(s.labelKey)}
                active={s.id === active.id}
                onClick={() => {
                  setActiveId(s.id);
                }}
              />
            ))}
          </nav>
          <div
            key={active.id}
            className="min-w-0 flex-1 overflow-y-auto [overscroll-behavior:contain] bg-surface-app px-5 py-5"
          >
            {active.render(data, { onRerunSetup, modelType, setModelType, goToModelLlm })}
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
