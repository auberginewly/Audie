import type { ReactNode } from "react";
import { Icon, type IconName } from "./Icon";

// ---- InlineNotice -----------------------------------------------------------

export type NoticeTone = "info" | "success" | "warning" | "danger";

const NOTICE: Record<NoticeTone, { box: string; fg: string; icon: IconName }> = {
  info: { box: "bg-aubergine-100", fg: "text-aubergine-900", icon: "info" },
  success: { box: "bg-green-100", fg: "text-green-900", icon: "check-circle" },
  warning: { box: "bg-amber-100", fg: "text-amber-700", icon: "alert" },
  danger: { box: "bg-red-100", fg: "text-red-900", icon: "alert" },
};

interface InlineNoticeProps {
  tone?: NoticeTone;
  title?: ReactNode;
  icon?: IconName;
  action?: ReactNode;
  children?: ReactNode;
}

/** A boxed inline notice — fallback warnings, tips, recoverable errors. */
export function InlineNotice({ tone = "info", title, icon, action, children }: InlineNoticeProps) {
  const t = NOTICE[tone];
  return (
    <div className={["flex gap-2.5 rounded-sm px-3 py-2.5", t.box].join(" ")}>
      <span className={["mt-px inline-flex shrink-0", t.fg].join(" ")}>
        <Icon name={icon ?? t.icon} size={16} strokeWidth={2} />
      </span>
      <div className="min-w-0 flex-1">
        {title ? (
          <div className={["text-[13px] font-medium", t.fg, children ? "mb-0.5" : ""].join(" ")}>{title}</div>
        ) : null}
        {children ? <div className="text-[13px] leading-[18px] text-text-secondary">{children}</div> : null}
      </div>
      {action ? <div className="shrink-0">{action}</div> : null}
    </div>
  );
}

// ---- StatusMessage ----------------------------------------------------------

export type StatusTone = "neutral" | "success" | "warning" | "danger" | "pending";

const STATUS: Record<StatusTone, { color: string; icon: IconName }> = {
  neutral: { color: "text-text-tertiary", icon: "info" },
  success: { color: "text-success-text", icon: "check-circle" },
  warning: { color: "text-warning-text", icon: "alert" },
  danger: { color: "text-danger-text", icon: "x-circle" },
  pending: { color: "text-text-tertiary", icon: "loader" },
};

interface StatusMessageProps {
  tone?: StatusTone;
  icon?: IconName | null; // null = no leading icon (text only)
  children: ReactNode;
}

/** Inline status line — the small "未检查 / 已保存 / key 无效" feedback under a field. */
export function StatusMessage({ tone = "neutral", icon, children }: StatusMessageProps) {
  const t = STATUS[tone];
  const spin = tone === "pending";
  return (
    <span className={["inline-flex items-center gap-1.5 text-xs leading-4", t.color].join(" ")}>
      {icon !== null ? (
        <Icon name={icon ?? t.icon} size={13} strokeWidth={2} className={spin ? "animate-spin" : undefined} />
      ) : null}
      <span className="min-w-0">{children}</span>
    </span>
  );
}
