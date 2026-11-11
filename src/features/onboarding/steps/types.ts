import type { I18nKey } from "../../../i18n";

export type StepId = "welcome" | "permissions" | "hotkey" | "asr" | "llm" | "test";

export type TestPhase = "idle" | "recording" | "processing" | "success";

export const NUMBERED: StepId[] = ["permissions", "hotkey", "asr", "llm", "test"];

export const STEP_LABEL: Record<StepId, I18nKey> = {
  welcome: "setup.step.welcome",
  permissions: "setup.step.permissions",
  hotkey: "setup.step.hotkey",
  asr: "setup.step.asr",
  llm: "setup.step.llm",
  test: "setup.step.test",
};
