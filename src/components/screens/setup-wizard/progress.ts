import { isKeyOptionalModel, llmModelIdForBaseUrl, modelIdForAsrProvider } from "../../Settings/models";
import type { Settings } from "../../../types/settings";
import { permissionsAreReady } from "../../../hooks/permissionState";
import type { RuntimePlatform } from "../../../lib/runtimePlatform";

export type OnboardingStepId = "permissions" | "hotkey" | "asr" | "llm" | "test";

export interface OnboardingPermissionStatus {
  microphone: boolean | null;
  accessibility: boolean | null;
  inputMonitoring: boolean | null;
}

export interface OnboardingProgress {
  steps: Record<OnboardingStepId, boolean>;
  done: number;
  total: number;
  requiredComplete: boolean;
  pickedAsr: string | null;
  pickedLlm: string | null;
}

const TOTAL_STEPS = 5;

// Both entry points render this same persisted/system-derived snapshot. A model is
// only complete when it is the saved active provider and can actually be used.
export function deriveOnboardingProgress(
  settings: Settings | null,
  permissions: OnboardingPermissionStatus,
  platform: RuntimePlatform,
  configured: (modelId: string) => boolean,
): OnboardingProgress {
  const pickedAsr = settings ? modelIdForAsrProvider(settings.asr_provider) : null;
  const pickedLlm = settings ? llmModelIdForBaseUrl(settings.openai_compatible_base_url) || null : null;
  const permissionsDone = permissionsAreReady(permissions, platform);
  const hotkeyDone = Boolean(settings?.hotkey.trim());
  const asrDone = Boolean(pickedAsr && configured(pickedAsr));
  const llmDone = Boolean(
    settings &&
    pickedLlm &&
    settings.openai_compatible_model.trim() &&
    (isKeyOptionalModel(pickedLlm) || configured(pickedLlm)),
  );
  const testDone = settings?.onboarding_test_completed === true;
  const steps = {
    permissions: permissionsDone,
    hotkey: hotkeyDone,
    asr: asrDone,
    llm: llmDone,
    test: testDone,
  };

  return {
    steps,
    done: Object.values(steps).filter(Boolean).length,
    total: TOTAL_STEPS,
    requiredComplete: permissionsDone && hotkeyDone && asrDone,
    pickedAsr,
    pickedLlm,
  };
}
