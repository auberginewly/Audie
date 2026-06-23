import { Icon, type IconName } from "../ui";

export type UpdateState = "idle" | "checking" | "up-to-date" | "available" | "downloading";

export type UpdateLabels = {
  check: string;
  checking: string;
  upToDate: string;
  update: string;
  downloading: string;
};

const ICONS: Record<UpdateState, IconName> = {
  idle: "refresh",
  checking: "loader",
  "up-to-date": "check",
  available: "download",
  downloading: "loader",
};

type UpdateButtonProps = {
  state?: UpdateState;
  availableVersion?: string;
  labels: UpdateLabels;
  compact?: boolean;
  onClick?: () => void;
  className?: string;
};

/**
 * Update affordance, state-driven. compact = next to the version/traffic lights:
 * an icon-only ghost for check/checking/up-to-date, an accent pill when an update
 * is available/downloading. full = a sidebar-width button.
 */
export function UpdateButton({
  state = "idle",
  availableVersion,
  labels,
  compact = false,
  onClick,
  className = "",
}: UpdateButtonProps) {
  const icon = ICONS[state];
  const spin = state === "checking" || state === "downloading";
  const accent = state === "available" || state === "downloading";
  const disabled = state === "checking" || state === "downloading";
  const dim = state === "up-to-date";
  const fullLabel: Record<UpdateState, string> = {
    idle: labels.check,
    checking: labels.checking,
    "up-to-date": labels.upToDate,
    available: availableVersion ? `${labels.update} ${availableVersion}` : labels.update,
    downloading: labels.downloading,
  };
  const spinCls = spin ? "animate-spin" : undefined;

  if (compact && accent) {
    return (
      <button
        onClick={onClick}
        disabled={disabled}
        title={fullLabel[state]}
        className={[
          "inline-flex h-[22px] items-center gap-1 rounded-full border border-transparent px-[9px]",
          "bg-accent-fill text-text-on-accent font-sans text-[11px] font-semibold",
          disabled ? "cursor-default" : "cursor-pointer",
          className,
        ].join(" ")}
      >
        <Icon name={icon} size={12} strokeWidth={2} className={spinCls} />
        <span>{state === "downloading" ? labels.downloading : labels.update}</span>
      </button>
    );
  }

  if (compact) {
    return (
      <button
        onClick={onClick}
        disabled={disabled}
        aria-label={fullLabel[state]}
        title={fullLabel[state]}
        className={[
          "inline-flex h-[22px] w-6 items-center justify-center rounded-sm border-0 p-0",
          "transition-colors duration-150 ease-[var(--ease-out)]",
          dim
            ? "text-success-text"
            : "text-text-tertiary hover:bg-gray-alpha-200 hover:text-text-primary",
          disabled ? "cursor-default" : "cursor-pointer",
          className,
        ].join(" ")}
      >
        <Icon name={icon} size={14} className={spinCls} />
      </button>
    );
  }

  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={[
        "flex h-[30px] w-full items-center justify-center gap-[7px] rounded-sm px-2.5",
        "font-sans text-xs font-medium transition-colors duration-150 ease-[var(--ease-out)]",
        accent
          ? "border border-transparent bg-accent-fill text-text-on-accent"
          : dim
            ? "border border-border-default bg-gray-100 text-text-tertiary"
            : "border border-border-default bg-gray-100 text-text-secondary",
        disabled ? "cursor-default" : "cursor-pointer",
        className,
      ].join(" ")}
    >
      <Icon name={icon} size={14} className={spinCls} />
      <span>{fullLabel[state]}</span>
    </button>
  );
}
