import type { ReactNode } from "react";
import type { ErrorCode } from "./events";
import type { I18nContextValue } from "../../i18n";
import { callOverlayCommand as call } from "./overlayCommands";

// Filled / ghost text button inside a terminal toast card.
export function CardButton({ label, tone, onClick }: { label: string; tone: "ghost" | "accent"; onClick: () => void }) {
  const accent = tone === "accent";
  return (
    <button
      type="button"
      // Keep the user's app key so 撤销 / 重试 / 插入原文 inject lands at the caret
      // (see CapsuleButton).
      onMouseDown={(e) => {
        e.preventDefault();
      }}
      onClick={onClick}
      className={[
        "inline-flex h-[34px] shrink-0 items-center justify-center rounded-[10px] border-0 px-4 cursor-pointer",
        "text-[13px] font-medium whitespace-nowrap",
        "transition-colors duration-150 ease-[var(--ease-out)]",
        accent
          ? "bg-accent-fill text-text-on-accent hover:bg-accent-fill-hover"
          : "bg-gray-alpha-200 text-text-primary hover:bg-gray-alpha-300",
      ].join(" ")}
    >
      {label}
    </button>
  );
}

// Terminal-state card: centered title (+ subtitle) over a row of action buttons.
export function ToastCard({ title, subtitle, buttons }: { title: string; subtitle?: string; buttons: ReactNode }) {
  return (
    <div
      role="status"
      className={[
        "inline-flex min-w-[200px] max-w-[320px] flex-col items-center gap-3 p-4",
        "rounded-[14px] [corner-shape:superellipse(3)] border-0 bg-surface-capsule text-text-primary shadow-capsule",
      ].join(" ")}
      style={{ animation: "audie-rise 0.22s var(--ease-out)" }}
    >
      <div className="flex flex-col items-center gap-0.5 text-center">
        <span className="text-[13px] font-medium text-balance">{title}</span>
        {subtitle ? <span className="text-xs text-text-tertiary">{subtitle}</span> : null}
      </div>
      {buttons ? <div className="flex flex-wrap items-center justify-center gap-2">{buttons}</div> : null}
    </div>
  );
}

// Error toast actions by category: inject failed → 插入原文 (re-paste) + 重试;
// network / provider failed → 重试; permission / device / internal → message only.
export function errorActions(code: ErrorCode | undefined, t: I18nContextValue["t"]): ReactNode {
  if (code === "inject") {
    return (
      <>
        <CardButton
          label={t("overlay.insertRaw")}
          tone="ghost"
          onClick={() => {
            call("insert_raw_last");
          }}
        />
        <CardButton
          label={t("overlay.retry")}
          tone="accent"
          onClick={() => {
            call("retry_last");
          }}
        />
      </>
    );
  }
  if (code === "network" || code === "provider") {
    return (
      <CardButton
        label={t("overlay.retry")}
        tone="accent"
        onClick={() => {
          call("retry_last");
        }}
      />
    );
  }
  return null;
}
