// Home — the landing, mirroring the design's Typeless-style rhythm: slogan +
// fn hint over a usage stat grid. Stats are real (all-time, from the
// HistoryManager); state lives in the capsule overlay, not here.

import { Icon, Keycap, type IconName } from "../ui";
import { useUsageStats } from "../../hooks/useUsageStats";
import { useI18n } from "../../i18n";

function StatCard({ icon, value, unit, label }: { icon: IconName; value: string; unit: string; label: string }) {
  return (
    <div className="rounded-md bg-surface-card p-3.5">
      <div className="flex items-center justify-between gap-2">
        <span className="inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-sm bg-gray-200 text-text-tertiary">
          <Icon name={icon} size={15} />
        </span>
        <div className="flex min-w-0 items-baseline gap-1">
          <span className="text-2xl font-semibold leading-none tracking-[-0.8px] text-text-primary">{value}</span>
          <span className="text-[11px] text-text-tertiary">{unit}</span>
        </div>
      </div>
      <div className="mt-2.5 text-xs text-text-secondary">{label}</div>
    </div>
  );
}

// "1900" → "1.9K"; smaller counts stay literal.
function formatCount(n: number): string {
  return n >= 1000 ? `${(n / 1000).toFixed(1)}K` : String(n);
}

export function HomeScreen() {
  const { t } = useI18n();
  const stats = useUsageStats();
  // 「口述」三卡只算纯口述听写（mode=polish），不被写作/改写产出虚高（见 history.rs）。
  const words = stats?.spoken_words ?? 0;
  const durationMin = (stats?.spoken_duration_ms ?? 0) / 60000;
  const spokenMin = Math.round(durationMin);
  const wpm = durationMin > 0 ? Math.round(words / durationMin) : 0;
  const aiWords = stats?.ai_output_words ?? 0;

  const cards: { icon: IconName; value: string; unit: string; label: string }[] = [
    { icon: "clock", value: String(spokenMin), unit: t("home.stat.minutes"), label: t("home.stat.spokenTime") },
    { icon: "mic", value: formatCount(words), unit: t("home.stat.characters"), label: t("home.stat.spokenWords") },
    { icon: "sparkles", value: formatCount(aiWords), unit: t("home.stat.characters"), label: t("home.stat.aiOutput") },
    {
      icon: "audio-lines",
      value: String(wpm),
      unit: t("home.stat.charactersPerMinute"),
      label: t("home.stat.averageSpeed"),
    },
  ];

  return (
    <div data-tauri-drag-region className="px-7 pt-6">
      <div className="mb-6 pl-1">
        <h1 className="max-w-[36ch] text-balance text-xl font-semibold leading-[26px] tracking-[-0.4px] text-text-primary">
          {t("home.hero.title")}
        </h1>
        <div className="mt-3 flex items-center gap-2 text-sm text-text-tertiary">
          <span>{t("home.hero.prefix")}</span>
          <Keycap>fn</Keycap>
          <span>{t("home.hero.suffix")}</span>
        </div>
      </div>

      <div className="grid grid-cols-4 gap-3">
        {cards.map((s) => (
          <StatCard key={s.label} icon={s.icon} value={s.value} unit={s.unit} label={s.label} />
        ))}
      </div>
    </div>
  );
}
