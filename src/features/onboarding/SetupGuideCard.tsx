import { Button } from "../../components/ui";
import { useI18n } from "../../i18n";
import type { OnboardingProgress } from "./steps/progress";

// Sidebar and wizard consume the same derived snapshot, so their checkmarks and
// x/5 value cannot diverge when a saved provider or keychain secret changes.
export function SetupGuideCard({ progress, onContinue }: { progress: OnboardingProgress; onContinue: () => void }) {
  const { t } = useI18n();
  return (
    <div className="rounded-md bg-gray-100 p-3">
      <div className="mb-2 flex items-center justify-between">
        <span className="text-[13px] font-medium text-text-primary">{t("app.setup.progressTitle")}</span>
        <span className="font-mono text-[11px] text-text-tertiary">
          {progress.done}/{progress.total}
        </span>
      </div>
      <div className="mb-2.5 h-1 overflow-hidden rounded-full bg-gray-300">
        <div
          className="h-full rounded-full bg-accent-fill"
          style={{ width: `${(progress.done / progress.total) * 100}%` }}
        />
      </div>
      <Button size="sm" variant="accent" block iconRight="chevron-right" onClick={onContinue}>
        {t("app.setup.continue")}
      </Button>
    </div>
  );
}
