// Home — the landing, mirroring the design's Typeless-style rhythm: slogan +
// fn hint over a usage stat grid, then a daily line chart (口述字数 vs AI 产出)
// with a range picker (近 7/30/60 天). All data is real (HistoryManager
// aggregates); state lives in the capsule overlay, not here. New installs
// render the cards at zero and a flat line.

import { useMemo, useState } from "react";
import { CartesianGrid, Line, LineChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";

import { Icon, Keycap, Select, type IconName } from "../ui";
import { useUsageStats } from "../../hooks/useUsageStats";
import { useDailyUsage } from "../../hooks/useDailyUsage";
import { useI18n, type I18nContextValue } from "../../i18n";
import {
  CHART_RANGES,
  createEvenTicks,
  fillUsageWindow,
  isChartRange,
  labelForTick,
  type ChartPoint,
  type ChartRange,
} from "./homeChart";

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

// Recharts paints SVG stroke/fill *attributes*, where var() is unreliable in
// WKWebView — resolve the theme token to its concrete value once (getComputedStyle
// already substitutes var() chains; the theme itself is static per launch).
function useThemeColor(varName: string, fallback: string): string {
  const [color] = useState(() => {
    const resolved = getComputedStyle(document.documentElement).getPropertyValue(varName).trim();
    return resolved || fallback;
  });
  return color;
}

interface DateAxisTickProps {
  x?: number;
  y?: number;
  payload?: { value?: number };
  data: ChartPoint[];
  lastIndex: number;
  color: string;
}

function DateAxisTick({ x = 0, y = 0, payload, data, lastIndex, color }: DateAxisTickProps) {
  const value = Number(payload?.value);
  if (!Number.isFinite(value)) return null;

  const textAnchor = value === 0 ? "start" : value === lastIndex ? "end" : "middle";
  return (
    <text x={x} y={y} dy={14} textAnchor={textAnchor} fill={color} fontSize={11}>
      {labelForTick(data, value)}
    </text>
  );
}

// 每日双线折线图：口述字数取亮紫，AI 产出取主题深紫；350ms 线性生长
// 使用 Recharts 3.8.1 的稳定 dash 末帧。数据更新由 history-updated 驱动。
const RANGE_LABEL_KEYS: Record<ChartRange, Parameters<I18nContextValue["t"]>[0]> = {
  7: "home.chart.range.7",
  30: "home.chart.range.30",
  60: "home.chart.range.60",
};
const RANGES = CHART_RANGES.map((days) => ({ days, labelKey: RANGE_LABEL_KEYS[days] }));

function UsageChart() {
  const { t } = useI18n();
  const [days, setDays] = useState<ChartRange>(7);
  const rows = useDailyUsage(days);
  const data = useMemo(() => fillUsageWindow(rows, days), [rows, days]);
  const ticks = useMemo(() => createEvenTicks(days), [days]);
  const spokenColor = useThemeColor("--accent-text", "#c08cf0");
  const aiColor = useThemeColor("--accent-fill", "#8e4ec6");
  const axisColor = useThemeColor("--text-tertiary", "#878787");
  const gridColor = useThemeColor("--border-subtle", "#ffffff17");

  const legend = [
    { color: spokenColor, label: t("home.stat.spokenWords") },
    { color: aiColor, label: t("home.stat.aiOutput") },
  ];

  return (
    <section className="mt-6 rounded-md bg-surface-card p-3.5">
      <div className="mb-3 flex items-center justify-between gap-3">
        <h2 className="pl-1 text-sm font-normal text-text-secondary">{t("home.chart.title")}</h2>
        <div className="flex items-center gap-3">
          {legend.map((l) => (
            <span key={l.label} className="flex items-center gap-1.5 text-xs text-text-tertiary">
              <span className="h-2 w-2 rounded-full" style={{ backgroundColor: l.color }} />
              {l.label}
            </span>
          ))}
          <div className="w-[120px]">
            <Select
              size="sm"
              value={String(days)}
              onChange={(e) => {
                const nextDays = Number(e.target.value);
                if (isChartRange(nextDays)) setDays(nextDays);
              }}
            >
              {RANGES.map((r) => (
                <option key={r.days} value={String(r.days)}>
                  {t(r.labelKey)}
                </option>
              ))}
            </Select>
          </div>
        </div>
      </div>
      <ResponsiveContainer width="100%" height={180}>
        <LineChart accessibilityLayer={false} data={data} margin={{ top: 4, right: 0, bottom: 0, left: 0 }}>
          <CartesianGrid vertical={false} stroke={gridColor} />
          <XAxis
            dataKey="dayIndex"
            type="number"
            domain={[0, days - 1]}
            ticks={ticks}
            interval={0}
            padding={{ left: 0, right: 0 }}
            allowDataOverflow
            tickLine={false}
            axisLine={false}
            tick={<DateAxisTick data={data} lastIndex={days - 1} color={axisColor} />}
          />
          <YAxis hide domain={[0, "auto"]} />
          <Tooltip
            cursor={{ stroke: gridColor }}
            contentStyle={{
              background: "var(--surface-overlay)",
              border: "1px solid var(--border-default)",
              borderRadius: 8,
              fontSize: 12,
            }}
            labelStyle={{ color: "var(--text-tertiary)", marginBottom: 4 }}
            labelFormatter={(label) => labelForTick(data, Number(label))}
          />
          <Line
            key={`spoken-${days}`}
            type="linear"
            dataKey="spoken"
            name={t("home.stat.spokenWords")}
            stroke={spokenColor}
            strokeWidth={2}
            dot={false}
            activeDot={{ r: 3 }}
            animationDuration={350}
            animationEasing="linear"
            strokeLinecap="butt"
          />
          <Line
            key={`ai-${days}`}
            type="linear"
            dataKey="ai"
            name={t("home.stat.aiOutput")}
            stroke={aiColor}
            strokeWidth={2}
            dot={false}
            activeDot={{ r: 3 }}
            animationDuration={350}
            animationEasing="linear"
            strokeLinecap="butt"
          />
        </LineChart>
      </ResponsiveContainer>
    </section>
  );
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

      <UsageChart />
    </div>
  );
}
