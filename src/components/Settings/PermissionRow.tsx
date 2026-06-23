import { Badge, Button, Icon, type BadgeTone, type IconName } from "../ui";

export type PermissionStatus = "granted" | "denied" | "pending";

const STATUS: Record<PermissionStatus, { tone: BadgeTone; label: string; icon: IconName; color: string }> = {
  granted: { tone: "success", label: "已授权", icon: "check-circle", color: "text-success-text" },
  denied: { tone: "danger", label: "已拒绝", icon: "x-circle", color: "text-danger-text" },
  pending: { tone: "warning", label: "未授权", icon: "alert", color: "text-warning-text" },
};

type PermissionRowProps = {
  icon?: IconName;
  name: string;
  description?: string;
  status?: PermissionStatus;
  onGrant?: () => void;
  grantLabel?: string;
  divider?: boolean;
};

/**
 * A macOS permission row — Microphone, Accessibility. Shows status and, when not
 * granted, a grant action. Color always pairs with an icon + label (never alone).
 */
export function PermissionRow({
  icon = "shield",
  name,
  description,
  status = "pending",
  onGrant,
  grantLabel,
  divider = true,
}: PermissionRowProps) {
  const st = STATUS[status];
  const needsAction = status !== "granted";
  return (
    <div className="relative flex items-center gap-3 px-3.5 py-3">
      {divider ? <div className="absolute inset-x-3.5 top-0 h-px bg-border-subtle" /> : null}
      <span className={["inline-flex", st.color].join(" ")}>
        <Icon name={icon} size={18} />
      </span>
      <div className="min-w-0 flex-1">
        <div className="text-sm font-medium text-text-primary">{name}</div>
        {description ? <div className="mt-px text-xs text-text-tertiary">{description}</div> : null}
      </div>
      <Badge tone={st.tone} icon={st.icon}>
        {st.label}
      </Badge>
      {needsAction && onGrant ? (
        <Button size="sm" variant={status === "denied" ? "secondary" : "accent"} onClick={onGrant}>
          {grantLabel ?? (status === "denied" ? "打开设置" : "授权")}
        </Button>
      ) : null}
    </div>
  );
}
