import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Icon, IconButton, type IconName } from "../ui";
import { openExternal } from "../../lib/open";
import { useI18n, type I18nKey } from "../../i18n";

type SidebarItemProps = {
  icon?: IconName;
  label: ReactNode;
  active?: boolean;
  trailing?: ReactNode;
} & ButtonHTMLAttributes<HTMLButtonElement>;

/** A sidebar navigation row. Active rows get a faint fill; inactive tint on hover. */
export function SidebarItem({ icon, label, active = false, trailing, className = "", ...rest }: SidebarItemProps) {
  return (
    <button
      aria-current={active ? "page" : undefined}
      className={[
        "relative flex h-[34px] w-full items-center gap-2.5 px-3 text-left border-0 rounded-sm",
        "font-sans text-sm leading-5 cursor-pointer",
        "transition-colors duration-150 ease-[var(--ease-out)]",
        active
          ? "bg-gray-alpha-200 text-text-primary font-medium"
          : "bg-transparent text-text-secondary font-normal hover:bg-gray-alpha-100 hover:text-text-primary",
        className,
      ].join(" ")}
      {...rest}
    >
      {icon ? <Icon name={icon} size={17} /> : null}
      <span className="flex-1">{label}</span>
      {trailing}
    </button>
  );
}

/** Sidebar header — Audie waveform mark + wordmark + version badge. */
export function SidebarHeader({ version = "—" }: { version?: string }) {
  return (
    <div className="flex items-center gap-[9px]">
      <span className="inline-flex h-[26px] w-[26px] items-center justify-center gap-0.5 rounded-[7px] bg-gray-200">
        <i className="w-0.5 rounded-sm bg-aubergine-900" style={{ height: 7 }} />
        <i className="w-0.5 rounded-sm bg-aubergine-700" style={{ height: 13 }} />
        <i className="w-0.5 rounded-sm bg-aubergine-700" style={{ height: 10 }} />
        <i className="w-0.5 rounded-sm bg-aubergine-900" style={{ height: 6 }} />
      </span>
      <span className="font-sans text-base font-semibold tracking-[-0.32px] text-text-primary">Audie</span>
      <span className="ml-0.5 rounded-full bg-gray-alpha-200 px-[7px] py-px font-mono text-[11px] text-text-tertiary">
        {version}
      </span>
    </div>
  );
}

export interface SidebarNavItem {
  key: string;
  icon: IconName;
  labelKey: I18nKey;
  trailing?: ReactNode;
}

const DEFAULT_NAV: SidebarNavItem[] = [
  { key: "home", icon: "home", labelKey: "app.sidebar.home" },
  { key: "history", icon: "history", labelKey: "app.sidebar.history" },
];

interface AppSidebarProps {
  active?: string;
  onNavigate?: (key: string) => void;
  items?: SidebarNavItem[];
  version?: string;
  githubUrl?: string;
  onSettings?: () => void;
  settingsActive?: boolean;
  aboveDock?: ReactNode;
}

/**
 * The Audie desktop left rail. Brand header on top, primary nav in the middle,
 * and a bottom dock with a GitHub pill (left) + a settings gear (right).
 * Settings opens as a dialog, not a page — wire `onSettings`.
 */
export function AppSidebar({
  active = "home",
  onNavigate,
  items = DEFAULT_NAV,
  version = "—",
  githubUrl = "https://github.com/auberginewly/Audie",
  onSettings,
  settingsActive = false,
  aboveDock,
}: AppSidebarProps) {
  const { t } = useI18n();
  return (
    <aside
      data-tauri-drag-region
      className="flex h-full w-[var(--sidebar-width)] shrink-0 flex-col bg-surface-sidebar box-border pt-9 pr-2.5 pb-3 pl-4"
    >
      <SidebarHeader version={version} />

      <div className="mt-[18px] flex flex-col gap-0.5 pt-1">
        {items.map((it) => (
          <SidebarItem
            key={it.key}
            icon={it.icon}
            label={t(it.labelKey)}
            trailing={it.trailing}
            active={active === it.key}
            onClick={() => onNavigate?.(it.key)}
          />
        ))}
      </div>

      <div data-tauri-drag-region className="flex-1" />

      {aboveDock ? <div className="mb-2.5">{aboveDock}</div> : null}

      <div className="flex items-center justify-between gap-2 pt-2.5">
        <a
          href={githubUrl}
          onClick={(e) => {
            e.preventDefault();
            openExternal(githubUrl);
          }}
          title="GitHub · auberginewly/Audie"
          className={[
            "inline-flex h-[30px] items-center gap-[7px] rounded-full bg-gray-100 px-[11px]",
            "font-sans text-xs font-medium text-text-secondary no-underline",
            "transition-colors duration-150 ease-[var(--ease-out)]",
            "hover:bg-surface-card-hover hover:text-text-primary",
          ].join(" ")}
        >
          <Icon name="github" size={15} />
          <span>GitHub</span>
        </a>
        <IconButton
          name="settings"
          label={t("app.sidebar.settings")}
          size="md"
          variant={settingsActive ? "outline" : "ghost"}
          onClick={() => onSettings?.()}
          className={settingsActive ? "bg-gray-alpha-200 text-text-primary" : ""}
        />
      </div>
    </aside>
  );
}
