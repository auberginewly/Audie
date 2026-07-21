import { Badge, Button } from "../../ui";
import { useI18n } from "../../../i18n";
import type { ModelMeta } from "../../Settings/models";
import { StepHeader } from "./StepHeader";

function WizardModelRow({
  m,
  configured,
  inUse,
  onPick,
  onConfigure,
}: {
  m: ModelMeta;
  configured: boolean;
  inUse: boolean;
  onPick: () => void;
  onConfigure: () => void;
}) {
  const { t } = useI18n();
  return (
    <div className="flex items-center gap-3 rounded-md bg-surface-card px-3.5 py-[13px]">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-text-primary">{m.name}</span>
          <Badge tone="neutral">{m.source === "local" ? t("setup.model.local") : t("setup.model.cloud")}</Badge>
          {inUse && configured ? (
            <Badge tone="accent">{t("setup.model.inUse")}</Badge>
          ) : configured ? (
            <Badge tone="success">{t("setup.model.configured")}</Badge>
          ) : (
            <Badge tone="neutral">{t("setup.model.unconfigured")}</Badge>
          )}
        </div>
        <div className="mt-[3px] font-mono text-[11px] text-text-tertiary">{m.model}</div>
      </div>
      {!inUse && configured ? (
        <Button size="sm" variant="secondary" onClick={onPick}>
          {t("setup.model.pick")}
        </Button>
      ) : null}
      <Button size="sm" variant="secondary" onClick={onConfigure}>
        {t("setup.model.configure")}
      </Button>
    </div>
  );
}

interface ModelStepProps {
  kind: "asr" | "llm";
  models: ModelMeta[];
  pickedModelId: string | null;
  configured: (modelId: string) => boolean;
  onPick: (model: ModelMeta) => void;
  onConfigure: (model: ModelMeta) => void;
  asrDone?: boolean;
}

export function ModelStep({
  kind,
  models,
  pickedModelId,
  configured,
  onPick,
  onConfigure,
  asrDone = true,
}: ModelStepProps) {
  const { t } = useI18n();
  const isAsr = kind === "asr";

  return (
    <>
      <StepHeader
        title={isAsr ? t("setup.asr.title") : t("setup.llm.title")}
        desc={isAsr ? t("setup.asr.desc") : t("setup.llm.desc")}
        tag={isAsr ? t("setup.required") : t("setup.optional")}
      />
      <div className="flex flex-col gap-2">
        {models.map((m) => (
          <WizardModelRow
            key={m.id}
            m={m}
            configured={configured(m.id)}
            inUse={pickedModelId === m.id}
            onPick={() => {
              onPick(m);
            }}
            onConfigure={() => {
              onConfigure(m);
            }}
          />
        ))}
      </div>
      {isAsr && !asrDone ? (
        <div className="mt-3 text-xs text-text-tertiary">{t("setup.asr.needConfigured")}</div>
      ) : null}
    </>
  );
}
