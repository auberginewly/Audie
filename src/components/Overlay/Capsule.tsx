// The recording capsule — Audie's signature overlay. Pure mirror of the Rust
// state machine (no business logic; PROJECT_SPEC §3.8 / §6.2). fe.8a restores
// the design's visuals (7-bar symmetric waveform, spinners, self-drawing check,
// colored status pills) mapped from the existing events. Interactive controls
// (✕/✓/undo/retry) land in fe.8b/8c when the overlay becomes clickable.

import { useEffect, useRef, useState } from "react";

import { useAudioLevels, type LevelRing } from "../../hooks/useAudioLevels";
import { useRecordingStore } from "../../store/recording";
import type { AppState } from "../../types/events";
import { Icon } from "../ui";

type CapsuleView =
  | "recording"
  | "transcribing"
  | "polishing"
  | "success"
  | "polish-unavailable"
  | "error"
  | "cancelled"
  | null;

// ── Waveform tuning (tweak these to taste) ──────────────────────────────────
// Resting/peak bar heights in px — center tallest, mirrored to the edges.
const BASE_H = [11, 20, 28, 34, 28, 20, 11];
const CENTER = 3; // (7 - 1) / 2
// Rust sends raw audio peaks (~0.05–0.4 for normal speech), so map → bar scale
// with a boost or the bars barely move:
//   GAIN  — overall multiplier before clamp (↑ = louder swing)
//   GAMMA — curve <1 lifts quiet speech (↓ = punchier)
//   FLOOR — resting scaleY when silent
const LEVEL_GAIN = 1.8;
const LEVEL_GAMMA = 0.55;
const LEVEL_FLOOR = 0.1;

function barScale(level: number): number {
  const raw = Math.min(1, level * LEVEL_GAIN);
  return LEVEL_FLOOR + (1 - LEVEL_FLOOR) * Math.pow(raw, LEVEL_GAMMA);
}

function deriveView(state: AppState, enhancePhase: string | undefined): CapsuleView {
  switch (state) {
    case "RECORDING":
      return "recording";
    case "PROCESSING":
      // Treat polish "completed" as still polishing so the capsule jumps
      // straight to 已插入 (SUCCESS) — no transient "润色完成" step.
      return enhancePhase === "started" || enhancePhase === "completed" ? "polishing" : "transcribing";
    case "SUCCESS":
      return enhancePhase === "failed" ? "polish-unavailable" : "success";
    case "ERROR":
      return "error";
    case "CANCEL":
      return "cancelled";
    default:
      return null;
  }
}

function Waveform({ levels }: { levels: LevelRing }) {
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

function DrawCheck() {
  return (
    <svg
      width={18}
      height={18}
      viewBox="0 0 24 24"
      fill="none"
      stroke="var(--success-text)"
      strokeWidth={2.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      className="block"
      aria-hidden="true"
    >
      <path
        d="M20 6 9 17l-5-5"
        style={{ strokeDasharray: 24, strokeDashoffset: 24, animation: "audie-draw 0.42s var(--ease-out) forwards" }}
      />
    </svg>
  );
}

const LABEL = "text-[13px] text-text-secondary whitespace-nowrap";

function formatElapsed(sec: number): string {
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export function Capsule() {
  const state = useRecordingStore((s) => s.state);
  const error = useRecordingStore((s) => s.error);
  const enhanceProgress = useRecordingStore((s) => s.enhanceProgress);
  const levels = useAudioLevels();

  const view = deriveView(state, enhanceProgress?.phase);
  const visible = view !== null;

  // View-only elapsed timer: counts while recording, resets otherwise.
  const [elapsed, setElapsed] = useState(0);
  const startRef = useRef<number | null>(null);
  useEffect(() => {
    if (state !== "RECORDING") {
      startRef.current = null;
      setElapsed(0);
      return;
    }
    let n = 0;
    setElapsed(0);
    const id = window.setInterval(() => {
      n += 1;
      setElapsed(n);
    }, 1000);
    return () => window.clearInterval(id);
  }, [state]);

  return (
    <div
      role="status"
      className={[
        "inline-flex h-12 min-w-[200px] items-center justify-center gap-2.5 px-4",
        // No backdrop-blur: on the transparent macOS overlay window it renders as
        // an opaque white box instead of frosting the desktop. surface-capsule is
        // ~95% opaque dark, so the pill reads solid without it.
        "rounded-full border-0 bg-surface-capsule text-text-primary shadow-capsule",
        "transition-all duration-200 ease-[var(--ease-out)]",
        visible ? "opacity-100 translate-y-0" : "pointer-events-none translate-y-2 opacity-0",
      ].join(" ")}
    >
      {view === "recording" ? (
        <div className="flex items-center gap-2.5 px-0.5">
          <Waveform levels={levels} />
          <span className="min-w-[34px] text-center font-mono text-xs text-text-tertiary">{formatElapsed(elapsed)}</span>
        </div>
      ) : null}

      {view === "transcribing" || view === "polishing" ? (
        <div className="inline-flex items-center gap-2 px-1.5">
          {view === "polishing" ? (
            <span className="inline-flex text-aubergine-900" style={{ animation: "audie-twinkle 1.3s var(--ease-out) infinite" }}>
              <Icon name="sparkles" size={15} strokeWidth={2} />
            </span>
          ) : (
            <span className="inline-flex text-aubergine-900" style={{ animation: "audie-spin 0.8s linear infinite" }}>
              <Icon name="loader" size={15} strokeWidth={2} />
            </span>
          )}
          <span className={LABEL}>{view === "polishing" ? "润色中…" : "转写中…"}</span>
        </div>
      ) : null}

      {view === "success" ? (
        <div className="inline-flex items-center gap-2 px-2.5">
          <DrawCheck />
          <span className={LABEL}>已插入</span>
        </div>
      ) : null}

      {view === "polish-unavailable" ? (
        <div className="inline-flex items-center gap-2 px-2.5">
          <Icon name="alert" size={15} strokeWidth={2} className="text-warning-text" />
          <span className={LABEL}>{enhanceProgress?.message ?? "已插入原文"}</span>
        </div>
      ) : null}

      {view === "error" ? (
        <div className="inline-flex items-center gap-2 px-2.5">
          <Icon name="alert" size={15} strokeWidth={2} className="text-danger-text" />
          <span className="truncate text-[13px] text-danger-text">{error?.message ?? "出错了"}</span>
        </div>
      ) : null}

      {view === "cancelled" ? <span className={LABEL}>已取消</span> : null}
    </div>
  );
}
