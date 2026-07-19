import assert from "node:assert/strict";
import test from "node:test";

import { CHART_RANGES, createEvenTicks, fillUsageWindow, labelForTick } from "../src/components/screens/homeChart.ts";

test("offers the 7, 30, and 60 day windows", () => {
  assert.deepEqual(CHART_RANGES, [7, 30, 60]);
});

test("zero-fills a continuous local-date window across a month boundary", () => {
  const points = fillUsageWindow(
    [{ day: "2026-06-30", spoken_words: 120, ai_output_words: 48 }],
    7,
    new Date(2026, 6, 2),
  );

  assert.equal(points.length, 7);
  assert.deepEqual(
    points.map(({ day, dayIndex }) => ({ day, dayIndex })),
    [
      { day: "2026-06-26", dayIndex: 0 },
      { day: "2026-06-27", dayIndex: 1 },
      { day: "2026-06-28", dayIndex: 2 },
      { day: "2026-06-29", dayIndex: 3 },
      { day: "2026-06-30", dayIndex: 4 },
      { day: "2026-07-01", dayIndex: 5 },
      { day: "2026-07-02", dayIndex: 6 },
    ],
  );
  assert.deepEqual(points[4], {
    dayIndex: 4,
    day: "2026-06-30",
    label: "6/30",
    spoken: 120,
    ai: 48,
  });
  assert.equal(points[0].spoken, 0);
  assert.equal(points[6].ai, 0);
});

test("creates seven pixel-even ticks including both edges", () => {
  for (const days of CHART_RANGES) {
    const ticks = createEvenTicks(days);
    const expectedGap = (days - 1) / 6;
    assert.equal(ticks.length, 7);
    assert.equal(ticks[0], 0);
    assert.equal(ticks.at(-1), days - 1);
    ticks.slice(1).forEach((tick, index) => {
      assert.ok(Math.abs(tick - ticks[index] - expectedGap) < 1e-10);
    });
  }
});

test("maps the first and last numeric ticks back to date labels", () => {
  const points = fillUsageWindow([], 7, new Date(2026, 6, 19));
  const ticks = createEvenTicks(7);
  assert.equal(labelForTick(points, ticks[0]), "7/13");
  assert.equal(labelForTick(points, ticks.at(-1)), "7/19");
});
