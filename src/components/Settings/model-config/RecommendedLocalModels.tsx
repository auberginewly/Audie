import { StatusMessage } from "../../ui";
import { useI18n } from "../../../i18n";
import { LOCAL_MODEL_RECOMMENDATIONS, type LocalModelRecommendation } from "../localModelRecommendations";

interface RecommendedLocalModelsProps {
  onPick: (tag: string) => void;
}

// RAM-tiered local LLM suggestions. The user chooses the tier; Audie only fills
// the model id and leaves downloads to Ollama / LM Studio.
export function RecommendedLocalModels({ onPick }: RecommendedLocalModelsProps) {
  const { t } = useI18n();
  const tiers = LOCAL_MODEL_RECOMMENDATIONS.reduce<{ ram: string; items: LocalModelRecommendation[] }[]>((acc, rec) => {
    const tier = acc.find((t2) => t2.ram === rec.ram);
    if (tier) tier.items.push(rec);
    else acc.push({ ram: rec.ram, items: [rec] });
    return acc;
  }, []);

  return (
    <div className="flex flex-col gap-[7px]">
      <label className="text-[13px] text-text-secondary">{t("settings.config.recommendedModels")}</label>
      <div className="flex flex-col gap-2">
        {tiers.map((tier) => (
          <div key={tier.ram} className="overflow-hidden rounded-sm border border-border-subtle">
            <div className="bg-surface-card px-2.5 py-1 text-[11px] font-medium text-text-tertiary">{tier.ram}</div>
            {tier.items.map((rec) => (
              <button
                key={rec.ram + rec.name}
                type="button"
                onClick={() => {
                  onPick(rec.tag);
                }}
                className="flex w-full flex-col gap-0.5 border-t border-border-subtle px-2.5 py-2 text-left hover:bg-gray-alpha-100"
              >
                <div className="flex items-baseline gap-2">
                  <span
                    className={
                      rec.primary ? "text-[12px] font-medium text-text-primary" : "text-[12px] text-text-secondary"
                    }
                  >
                    {rec.name}
                  </span>
                  {rec.primary ? (
                    <span className="shrink-0 rounded border border-border-subtle px-1 text-[10px] text-text-secondary">
                      {t("settings.config.primary")}
                    </span>
                  ) : null}
                  <span className="ml-auto shrink-0 font-mono text-[11px] text-text-tertiary">{rec.tag}</span>
                </div>
                <span className="text-[11px] text-text-tertiary">{rec.noteKey ? t(rec.noteKey) : rec.note}</span>
              </button>
            ))}
          </div>
        ))}
      </div>
      <StatusMessage tone="neutral" icon={null}>
        {t("settings.config.localModelHint")}
      </StatusMessage>
    </div>
  );
}
