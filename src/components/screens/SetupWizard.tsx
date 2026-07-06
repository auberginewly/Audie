// First-run setup wizard — a paneled modal (left numbered steps, right config).
// The screen owns the wizard flow; step UI lives in `setup-wizard/`.

import { useEffect, useState, type ReactNode } from "react";

import type { Hotkey } from "../../types/settings";
import type { UseSettings } from "../../hooks/useSettings";
import { usePermissions } from "../../hooks/usePermissions";
import { useConfiguredModels } from "../../hooks/useConfiguredModels";
import { useRecordingStore } from "../../store/recording";
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

interface SetupWizardProps {
  open: boolean;
  onClose: () => void;
  onComplete?: () => void;
  data: UseSettings;
  welcome?: boolean;
}

export function SetupWizard({ open, onClose, onComplete, data, welcome = true }: SetupWizardProps) {
  const { t } = useI18n();
  const [step, setStep] = useState(0);
  const perms = usePermissions();
  const configuredModels = useConfiguredModels();
  const [pickedAsr, setPickedAsr] = useState<string | null>(null);
  const [pickedLlm, setPickedLlm] = useState<string | null>(null);
  const [configModel, setConfigModel] = useState<ModelMeta | null>(null);
  // 试一下 completion is persistent (a dictation has succeeded) via the recording
  // store, so the checkmark survives reopening the wizard.
  const everSucceeded = useRecordingStore((s) => s.everSucceeded);

  useEffect(() => {
    if (open) setStep(0);
  }, [open, welcome]);
  if (!open) return null;

  const ids: StepId[] = (welcome ? (["welcome"] as StepId[]) : []).concat(NUMBERED);
  const last = ids.length - 1;
  const cur = Math.min(step, last);
  const id = ids[cur];

  const permDone =
    perms.microphone.granted === true && perms.accessibility.granted === true && perms.inputMonitoring.granted === true;
  // ASR step needs a picked model whose key is actually configured (real
  // has_secret), so onboarding can't "complete" with an unusable transcriber.
  const asrDone = !!pickedAsr && configuredModels.configured(pickedAsr);
  // A step is "done" when its own requirement is actually met (not merely passed),
  // so the sidebar checks each step the moment it's complete — current step included.
  const doneMap: Record<string, boolean> = {
    permissions: permDone,
    hotkey: !!data.settings?.hotkey,
    asr: asrDone,
    llm: !!pickedLlm,
    test: everSucceeded,
  };
  const subMap: Record<string, string> = {
    permissions: t("setup.required"),
    hotkey: t("setup.required"),
    asr: t("setup.required"),
    llm: t("setup.optional"),
    test: t("setup.optional"),
  };

  const isLast = id === "test";
  const blockNext = id === "asr" && !asrDone;
  const next = () => {
    if (!blockNext) setStep(Math.min(last, cur + 1));
  };
  const back = () => {
    setStep(Math.max(0, cur - 1));
  };

  const pickAsr = (m: ModelMeta) => {
    setPickedAsr(m.id);
    const provider = asrProviderForModelId(m.id);
    if (provider) void data.update({ asr_provider: provider });
  };
  const pickLlm = (m: ModelMeta) => {
    setPickedLlm(m.id);
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
        microphone={perms.microphone}
        accessibility={perms.accessibility}
        inputMonitoring={perms.inputMonitoring}
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
        pickedModelId={pickedAsr}
        configured={configuredModels.configured}
        onPick={pickAsr}
        onConfigure={setConfigModel}
        asrDone={asrDone}
      />
    );
  } else if (id === "llm") {
    body = (
      <ModelStep
        kind="llm"
        models={llmModels}
        pickedModelId={pickedLlm}
        configured={configuredModels.configured}
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
        className="relative flex h-[min(520px,100%)] w-[min(780px,100%)] flex-col overflow-hidden rounded-md bg-surface-app shadow-modal"
      >
        <div className="absolute right-2.5 top-2.5 z-10">
          <IconButton name="x" label={t("settings.close")} onClick={onClose} />
        </div>

        <div className="flex min-h-0 flex-1">
          <StepNav current={id} ids={ids} doneMap={doneMap} subMap={subMap} onSelect={setStep} />

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
                <Button variant="accent" onClick={onComplete ?? onClose}>
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
            configuredModels.refresh(); // a just-saved key should flip the badge
          }}
        />
      </div>
    </div>
  );
}
