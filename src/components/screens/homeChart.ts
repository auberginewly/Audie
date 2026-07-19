import type { DailyUsage } from "../../types/history";

export const CHART_RANGES = [7, 30, 60] as const;
export type ChartRange = (typeof CHART_RANGES)[number];

export function isChartRange(value: number): value is ChartRange {
  return CHART_RANGES.some((range) => range === value);
}

export interface ChartPoint {
  dayIndex: number;
  day: string;
  label: string;
  spoken: number;
  ai: number;
}

function localDayKey(date: Date): string {
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

function dateLabel(dayKey: string): string {
  const [, month, day] = dayKey.split("-");
  return `${Number(month)}/${Number(day)}`;
}

export function fillUsageWindow(rows: DailyUsage[], days: ChartRange, today = new Date()): ChartPoint[] {
  const byDay = new Map(rows.map((row) => [row.day, row]));
  return Array.from({ length: days }, (_, dayIndex) => {
    const daysBeforeToday = days - dayIndex - 1;
    const day = localDayKey(new Date(today.getFullYear(), today.getMonth(), today.getDate() - daysBeforeToday));
    const row = byDay.get(day);
    return {
      dayIndex,
      day,
      label: dateLabel(day),
      spoken: row?.spoken_words ?? 0,
      ai: row?.ai_output_words ?? 0,
    };
  });
}

export function createEvenTicks(days: ChartRange): number[] {
  const lastIndex = days - 1;
  return Array.from({ length: 7 }, (_, index) => (lastIndex * index) / 6);
}

export function labelForTick(data: ChartPoint[], tick: number): string {
  if (!Number.isFinite(tick) || data.length === 0) return "";
  const index = Math.max(0, Math.min(data.length - 1, Math.round(tick)));
  return data[index]?.label ?? "";
}
