// The recording capsule. Pure mirror of Rust events — no business logic here
// (PROJECT_SPEC.md §3.8 / §6.2).
// P0.2: three placeholder bars track the last three audio-level snapshots.
// P0.6: plain text for PROCESSING/SUCCESS, red + message for ERROR. CANCEL and
// IDLE fade out. Typeless-style polish lands in P2/P3.

import { useAudioLevels } from "../../hooks/useAudioLevels";
import { useRecordingStore } from "../../store/recording";

const BAR_MIN_PX = 4;
const BAR_MAX_PX = 28;
// Boost: typical speech peaks live in the 0.1–0.4 range, so a linear mapping
// looks too flat. A mild gamma curve gives the bars more "alive" feel without
// pretending to be a real spectrum.
const LEVEL_GAMMA = 0.6;

function barHeight(level: number): number {
  const eased = Math.min(1, Math.pow(level, LEVEL_GAMMA));
  return Math.max(BAR_MIN_PX, eased * BAR_MAX_PX);
}

export function Capsule() {
  const state = useRecordingStore((s) => s.state);
  const error = useRecordingStore((s) => s.error);
  const enhanceProgress = useRecordingStore((s) => s.enhanceProgress);
  const levels = useAudioLevels();

  // CANCEL and IDLE fade out; everything else keeps the capsule on screen.
  const visible =
    state === "RECORDING" ||
    state === "PROCESSING" ||
    state === "SUCCESS" ||
    state === "ERROR";
  const isError = state === "ERROR";

  return (
    <div
      className={`h-14 w-80 rounded-full backdrop-blur-md shadow-lg
                  flex items-center justify-center gap-1.5 px-4
                  transition-all duration-150 ease-out
                  ${isError ? "bg-error/90" : "bg-base-300/85"}
                  ${visible ? "opacity-100 translate-y-0" : "opacity-0 translate-y-2 pointer-events-none"}`}
    >
      {state === "RECORDING" &&
        levels.map((level, i) => (
          <span
            key={i}
            className="w-1 rounded-full bg-base-content/85 transition-[height] duration-75 ease-out"
            style={{ height: `${barHeight(level)}px` }}
          />
        ))}

      {state === "PROCESSING" && (
        <span className="text-sm text-base-content/70">
          {enhanceProgress?.message ?? "处理中…"}
        </span>
      )}

      {state === "SUCCESS" && (
        <span className="text-sm text-base-content/70">完成</span>
      )}

      {isError && (
        <span className="text-sm text-error-content text-center truncate">
          {error?.message ?? "出错了"}
        </span>
      )}
    </div>
  );
}
