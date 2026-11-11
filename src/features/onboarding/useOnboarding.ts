import { useEffect, useRef, useState } from "react";
import type { UseSettings } from "../settings/useSettings";
import type { usePermissions } from "../permissions/usePermissions";
import type { RuntimePlatform } from "../../lib/runtimePlatform";
import type { AppState } from "../dictation/events";
import type { useConfiguredModels } from "../providers/useConfiguredModels";
import { deriveOnboardingProgress } from "./steps/progress";

export function useOnboarding(
  data: UseSettings,
  permissions: ReturnType<typeof usePermissions>,
  configuredModels: ReturnType<typeof useConfiguredModels>,
  platform: RuntimePlatform,
  recordingState: AppState,
  closeSettings: () => void,
) {
  const [setupOpen, setSetupOpen] = useState(false);
  // First-run onboarding is persisted in settings (P3.12). Auto-open the wizard once
  // when a fresh install reports it incomplete; the ref stops it reopening if the
  // user closes it without finishing (it auto-opens again next launch, like Voxt).
  const onboardingCompleted = data.settings?.onboarding_completed;
  const onboardingProgress = deriveOnboardingProgress(
    data.settings,
    {
      microphone: permissions.microphone.granted,
      accessibility: permissions.accessibility.granted,
      inputMonitoring: permissions.inputMonitoring.granted,
    },
    platform,
    configuredModels.configured,
  );
  const autoOpened = useRef(false);
  useEffect(() => {
    if (onboardingCompleted === false && !autoOpened.current) {
      autoOpened.current = true;
      setSetupOpen(true);
    }
  }, [onboardingCompleted]);

  // SUCCESS is emitted only after the real dictation path completes. Persist it
  // through the existing Settings store so reopening/restarting keeps "Try it" done.
  useEffect(() => {
    if (recordingState === "SUCCESS" && data.settings && !data.settings.onboarding_test_completed) {
      void data.update({ onboarding_test_completed: true });
    }
  }, [data, recordingState]);

  // "重新运行配置向导" (About page): mark onboarding incomplete and reopen the
  // wizard, closing Settings so it's visible — saves the user editing the store.
  const rerunSetup = () => {
    void data.update({ onboarding_completed: false });
    closeSettings();
    setSetupOpen(true);
  };

  return { setupOpen, setSetupOpen, onboardingCompleted, onboardingProgress, rerunSetup };
}
