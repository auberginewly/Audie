import { Badge, Button, Icon, type BadgeTone, type IconName } from "../ui";
import { useI18n, type I18nKey } from "../../i18n";

export type PermissionStatus = "granted" | "denied" | "pending";

const STATUS: Record<PermissionStatus, { tone: BadgeTone; labelKey: I18nKey; icon?: IconName; color: string }> = {
  granted: { tone: "success", labelKey: "settings.permission.granted", color: "text-success-text" },
  denied: { tone: "danger", labelKey: "settings.permission.denied", icon: "x-circle", color: "text-danger-text" },
  pending: { tone: "warning", labelKey: "settings.permission.pending", icon: "alert", color: "text-warning-text" },
};

interface PermissionRowProps {
  icon?: IconName;
  name: string;
  description?: string;
  status?: PermissionStatus;
  onGrant?: () => void;
  grantLabel?: string;
  divider?: boolean;
}

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
  const { t } = useI18n();
  const st = STATUS[status];
  const needsAction = status !== "granted";
  return (
    <div className="relative flex items-center gap-3 px-3.5 py-3">
      {divider ? <div className="absolute inset-x-3.5 top-0 h-px bg-border-subtle" /> : null}
      <span className="inline-flex text-text-tertiary">
        <Icon name={icon} size={18} />
      </span>
      <div className="min-w-0 flex-1">
        <div className="text-sm font-medium text-text-primary">{name}</div>
        {description ? <div className="mt-px text-xs text-text-tertiary">{description}</div> : null}
      </div>
      <Badge tone={st.tone} icon={st.icon}>
        {t(st.labelKey)}
      </Badge>
      {needsAction && onGrant ? (
        <Button size="sm" variant={status === "denied" ? "secondary" : "accent"} onClick={onGrant}>
          {grantLabel ?? (status === "denied" ? t("settings.permission.openSettings") : t("settings.permission.grant"))}
        </Button>
      ) : null}
    </div>
  );
}
