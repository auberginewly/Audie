import type { LevelRing } from "./useAudioLevels";

// ── Waveform tuning (tweak these to taste) ──────────────────────────────────
// Resting/peak bar heights in px — center tallest, mirrored to the edges.
const BASE_H = [11, 20, 28, 34, 28, 20, 11];
const CENTER = 3; // (7 - 1) / 2
// Rust sends the raw audio peak (max|sample| over ~33ms, ~0.05–0.4 for normal
// speech). That value is honest but small, so map → bar scale with a boost or
// the bars barely move. THIS is the sensitivity knob:
//   GAIN  — sensitivity: how tall bars get per loudness (↑ = more reactive)
//   GAMMA — curve <1 lifts quiet speech so soft sounds register (↓ = punchier)
//   FLOOR — resting scaleY when silent
const LEVEL_GAIN = 2.6;
const LEVEL_GAMMA = 0.5;
const LEVEL_FLOOR = 0.1;

function barScale(level: number): number {
  const raw = Math.min(1, level * LEVEL_GAIN);
  return LEVEL_FLOOR + (1 - LEVEL_FLOOR) * Math.pow(raw, LEVEL_GAMMA);
}

export function Waveform({ levels }: { levels: LevelRing }) {
  return (
    <div className="flex h-9 items-center gap-[3px]">
      {BASE_H.map((h, i) => {
        const d = Math.abs(i - CENTER);
        const scale = barScale(levels[d] ?? 0);
        return (
          <span
            key={i}
            className="w-[3px] rounded-full bg-aubergine-900"
            style={{ height: `${h}px`, transform: `scaleY(${scale})`, transition: "transform 80ms linear" }}
          />
        );
      })}
    </div>
  );
}
