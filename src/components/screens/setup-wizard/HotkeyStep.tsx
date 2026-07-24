import type { Hotkey } from "../../../types/settings";
import { useI18n } from "../../../i18n";
import { HotkeyRecorder } from "../../Settings/HotkeyRecorder";
import { StepHeader } from "./StepHeader";

interface HotkeyStepProps {
  hotkey?: Hotkey;
  onChange: (hotkey: Hotkey) => Promise<boolean>;
}

export function HotkeyStep({ hotkey, onChange }: HotkeyStepProps) {
  const { t } = useI18n();

  return (
    <>
      <StepHeader title={t("setup.hotkey.title")} desc={t("setup.hotkey.desc")} tag={t("setup.required")} />
      <div className="flex items-center justify-between gap-3 rounded-md bg-surface-card p-3.5">
        <div className="min-w-0">
          <div className="text-sm font-medium text-text-primary">{t("setup.hotkey.recording")}</div>
          <div className="mt-0.5 text-xs text-text-tertiary">{t("setup.hotkey.recordingDesc")}</div>
        </div>
        {hotkey ? <HotkeyRecorder value={hotkey} onChange={onChange} /> : null}
      </div>
    </>
  );
}
