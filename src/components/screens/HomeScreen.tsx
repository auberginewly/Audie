// Home — the landing, mirroring the design's Typeless-style rhythm: slogan +
// fn hint over a "this week" stat grid. The four stats are mock — the backend
// tracks none yet (see plan). State lives in the capsule overlay, not here.

import { Icon, Keycap, type IconName } from "../ui";

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

// mock: backend tracks no usage stats yet.
const STATS: { icon: IconName; value: string; unit: string; label: string }[] = [
  { icon: "clock", value: "19", unit: "分钟", label: "口述时间" },
  { icon: "mic", value: "1.9K", unit: "字", label: "口述字数" },
  { icon: "zap", value: "53", unit: "分钟", label: "节省时间" },
  { icon: "audio-lines", value: "150", unit: "字/分", label: "平均口述速度" },
];

export function HomeScreen() {
  return (
    <div className="px-1">
      <div className="mb-6 pl-1">
        <h1 className="max-w-[36ch] text-balance text-xl font-semibold leading-[26px] tracking-[-0.4px] text-text-primary">
          言为心声，出口成章
        </h1>
        <div className="mt-3 flex items-center gap-2 text-sm text-text-tertiary">
          <span>按住</span>
          <Keycap>fn</Keycap>
          <span>开始和停止语音输入。</span>
        </div>
      </div>

      <div className="mb-3 pl-1 font-mono text-xs uppercase tracking-[0.04em] text-text-tertiary">本周</div>
      <div className="grid grid-cols-4 gap-3">
        {STATS.map((s) => (
          <StatCard key={s.label} icon={s.icon} value={s.value} unit={s.unit} label={s.label} />
        ))}
      </div>
    </div>
  );
}
