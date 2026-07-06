import type { PermissionState } from "../../../hooks/usePermissions";
import { openExternal } from "../../../lib/open";
import { Badge, Button, Icon, InlineNotice, type IconName } from "../../ui";
import { useI18n } from "../../../i18n";
import { StepHeader } from "./StepHeader";

function PermItem({
  icon,
  name,
  desc,
  hint,
  state,
}: {
  icon: IconName;
  name: string;
  desc: string;
  hint?: string;
  state: PermissionState;
}) {
  const { t } = useI18n();
  const granted = state.granted === true;
  return (
    <div className="flex items-center gap-3 rounded-md bg-surface-card p-3.5">
      <span className="inline-flex h-[34px] w-[34px] shrink-0 items-center justify-center rounded-sm bg-gray-200 text-text-secondary">
        <Icon name={icon} size={17} />
      </span>
      <div className="min-w-0 flex-1">
        <div className="text-sm font-medium text-text-primary">{name}</div>
        <div className="mt-0.5 text-xs text-text-tertiary">{desc}</div>
        {/* macOS won't re-prompt after a denial; Input Monitoring also only
            reflects a fresh grant after relaunch (P3.9). */}
        {!granted && hint ? <div className="mt-1 text-xs text-warning-text">{hint}</div> : null}
      </div>
      {granted ? (
        <Badge tone="success">{t("setup.permission.granted")}</Badge>
      ) : (
        <div className="flex shrink-0 items-center gap-2">
          <Button size="sm" variant="secondary" onClick={state.request}>
            {t("setup.permission.request")}
          </Button>
          <Button size="sm" variant="ghost" onClick={state.openSettings}>
            {t("setup.permission.openSettings")}
          </Button>
        </div>
      )}
    </div>
  );
}

interface PermissionStepProps {
  microphone: PermissionState;
  accessibility: PermissionState;
  inputMonitoring: PermissionState;
  hotkey?: string;
}

export function PermissionStep({ microphone, accessibility, inputMonitoring, hotkey }: PermissionStepProps) {
  const { t } = useI18n();

  return (
    <>
      <StepHeader title={t("setup.permissions.title")} desc={t("setup.permissions.desc")} tag={t("setup.required")} />
      <div className="flex flex-col gap-2">
        <PermItem
          icon="mic"
          name={t("settings.general.microphone")}
          desc={t("setup.permissions.micDesc")}
          state={microphone}
        />
        <PermItem
          icon="command"
          name={t("settings.general.accessibility")}
          desc={t("setup.permissions.accessibilityDesc")}
          state={accessibility}
        />
        <PermItem
          icon="key"
          name={t("setup.permissions.inputMonitoring")}
          desc={t("setup.permissions.inputMonitoringDesc")}
          hint={t("setup.permissions.inputMonitoringHint")}
          state={inputMonitoring}
        />
      </div>
      {/* Default trigger is fn/Globe, which macOS consumes unless reassigned. */}
      {hotkey === "Fn" ? (
        <div className="mt-3">
          <InlineNotice
            tone="warning"
            title={t("setup.fn.title")}
            action={
              <Button
                size="sm"
                variant="secondary"
                onClick={() => {
                  openExternal("x-apple.systempreferences:com.apple.preference.keyboard");
                }}
              >
                {t("setup.fn.keyboardSettings")}
              </Button>
            }
          >
            {t("setup.fn.body")}
          </InlineNotice>
        </div>
      ) : null}
    </>
  );
}
