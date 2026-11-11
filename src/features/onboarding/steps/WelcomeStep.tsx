import { useI18n } from "../../../i18n";
import { StepHeader } from "./StepHeader";

function WelPoint({ title, desc }: { title: string; desc: string }) {
  return (
    <div className="rounded-md bg-surface-card p-3.5">
      <div className="text-sm font-medium text-text-primary">{title}</div>
      <div className="mt-0.5 text-xs text-text-tertiary">{desc}</div>
    </div>
  );
}

export function WelcomeStep() {
  const { t } = useI18n();

  return (
    <>
      <StepHeader title={t("setup.welcome.title")} desc={t("setup.welcome.desc")} />
      <div className="flex flex-col gap-2">
        <WelPoint title={t("setup.welcome.point1Title")} desc={t("setup.welcome.point1Desc")} />
        <WelPoint title={t("setup.welcome.point2Title")} desc={t("setup.welcome.point2Desc")} />
        <WelPoint title={t("setup.welcome.point3Title")} desc={t("setup.welcome.point3Desc")} />
      </div>
    </>
  );
}
