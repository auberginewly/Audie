// First-run setup wizard — a paneled modal (left numbered steps, right config).
// The screen owns the wizard flow; step UI lives in `setup-wizard/`.

import { useEffect, useState, type ReactNode } from "react";

import type { Hotkey } from "../../types/settings";
import type { UseSettings } from "../../hooks/useSettings";
import type { UsePermissions } from "../../hooks/usePermissions";
import { Button, IconButton } from "../ui";
import { ModelConfigDialog } from "../Settings/ModelConfigDialog";
import { MODELS, asrProviderForModelId, llmPickPatch, type ModelMeta } from "../Settings/models";
import { useI18n } from "../../i18n";
import { HotkeyStep } from "./setup-wizard/HotkeyStep";
import { ModelStep } from "./setup-wizard/ModelStep";
import { PermissionStep } from "./setup-wizard/PermissionStep";
import { StepNav } from "./setup-wizard/StepNav";
import { TestStep } from "./setup-wizard/TestStep";
import { NUMBERED, type StepId } from "./setup-wizard/types";
import { WelcomeStep } from "./setup-wizard/WelcomeStep";
import type { OnboardingProgress } from "./setup-wizard/progress";

interface SetupWizardProps {
  open: boolean;
  onClose: () => void;
  onComplete?: () => void;
  data: UseSettings;
  permissions: UsePermissions;
  progress: OnboardingProgress;
  configured: (modelId: string) => boolean;
  onRefreshModels: () => void;
  welcome?: boolean;
}

export function SetupWizard({
  open,
  onClose,
  onComplete,
  data,
  permissions,
  progress,
  configured,
  onRefreshModels,
  welcome = true,
}: SetupWizardProps) {
  const { t } = useI18n();
  const [step, setStep] = useState(0);
  const [configModel, setConfigModel] = useState<ModelMeta | null>(null);

  useEffect(() => {
    if (open) setStep(0);
  }, [open, welcome]);
  if (!open) return null;

  const ids: StepId[] = (welcome ? (["welcome"] as StepId[]) : []).concat(NUMBERED);
  const last = ids.length - 1;
  const cur = Math.min(step, last);
  const id = ids[cur];

  const subMap: Record<string, string> = {
    permissions: t("setup.required"),
    hotkey: t("setup.required"),
    asr: t("setup.required"),
    llm: t("setup.optional"),
    test: t("setup.optional"),
  };

  const isLast = id === "test";
  const blockNext = id === "asr" && !progress.steps.asr;
  const next = () => {
    if (!blockNext) setStep(Math.min(last, cur + 1));
  };
  const back = () => {
    setStep(Math.max(0, cur - 1));
  };

  const pickAsr = (m: ModelMeta) => {
    const provider = asrProviderForModelId(m.id);
    // Match the settings picker: a model id belongs to its previous ASR provider,
    // so clear it when switching and let the selected provider use its default.
    if (provider) void data.update({ asr_provider: provider, asr_model: "" });
  };
  const pickLlm = (m: ModelMeta) => {
    if (data.settings) void data.update(llmPickPatch(m.id, data.settings));
  };

  const asrModels = MODELS.filter((m) => m.type === "asr");
  const llmModels = MODELS.filter((m) => m.type === "llm");

  let body: ReactNode;
  if (id === "welcome") {
    body = <WelcomeStep />;
  } else if (id === "permissions") {
    body = (
      <PermissionStep
        microphone={permissions.microphone}
        accessibility={permissions.accessibility}
        inputMonitoring={permissions.inputMonitoring}
        hotkey={data.settings?.hotkey}
      />
    );
  } else if (id === "hotkey") {
    body = <HotkeyStep hotkey={data.settings?.hotkey} onChange={(h: Hotkey) => data.update({ hotkey: h })} />;
  } else if (id === "asr") {
    body = (
      <ModelStep
        kind="asr"
        models={asrModels}
        pickedModelId={progress.pickedAsr}
        configured={configured}
        onPick={pickAsr}
        onConfigure={setConfigModel}
        asrDone={progress.steps.asr}
      />
    );
  } else if (id === "llm") {
    body = (
      <ModelStep
        kind="llm"
        models={llmModels}
        pickedModelId={progress.pickedLlm}
        configured={configured}
        onPick={pickLlm}
        onConfigure={setConfigModel}
      />
    );
  } else {
    body = <TestStep />;
  }

  return (
    <div
      onMouseDown={onClose}
      className="absolute inset-0 z-[70] flex items-center justify-center bg-black/50 p-6 backdrop-blur-[2px]"
    >
      <div
        role="dialog"
        aria-modal="true"
        onMouseDown={(e) => {
          e.stopPropagation();
        }}
        className="relative flex h-[min(520px,100%)] w-[min(780px,100%)] flex-col overflow-hidden rounded-md border border-gray-alpha-100 bg-surface-app shadow-modal"
      >
        <div className="absolute right-2.5 top-2.5 z-10">
          <IconButton name="x" label={t("settings.close")} onClick={onClose} />
        </div>

        <div className="flex min-h-0 flex-1">
          <StepNav current={id} ids={ids} doneMap={progress.steps} subMap={subMap} onSelect={setStep} />

          <div className="flex min-w-0 flex-1 flex-col bg-surface-app">
            <div key={id} className="min-h-0 flex-1 overflow-y-auto px-7 py-[26px] [overscroll-behavior:contain]">
              {body}
            </div>
            <div className="flex shrink-0 items-center gap-2 border-t border-border-subtle px-[18px] py-3.5">
              {cur > 0 ? (
                <Button variant="ghost" onClick={back}>
                  {t("setup.back")}
                </Button>
              ) : null}
              <div className="flex-1" />
              {isLast ? (
                <Button variant="accent" disabled={!progress.requiredComplete} onClick={onComplete ?? onClose}>
                  {t("setup.startUsing")}
                </Button>
              ) : (
                <Button variant="accent" disabled={blockNext} onClick={next}>
                  {id === "welcome" ? t("setup.startConfig") : t("setup.next")}
                </Button>
              )}
            </div>
          </div>
        </div>

        <ModelConfigDialog
          model={configModel}
          data={data}
          onClose={() => {
            setConfigModel(null);
            onRefreshModels(); // a just-saved key should flip the shared progress
          }}
        />
      </div>
    </div>
  );
}
