// The recording capsule — Audie's signature overlay. Pure mirror of the Rust
// state machine (no business logic; PROJECT_SPEC §3.8 / §6.2). fe.8a restores
// the design's visuals (7-bar symmetric waveform, spinners, self-drawing check,
// colored status pills) mapped from the existing events. Interactive controls
// (✕/✓/undo/retry) land in fe.8b/8c when the overlay becomes clickable.

import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { useAudioLevels, type LevelRing } from "../../hooks/useAudioLevels";
import { useRecordingStore } from "../../store/recording";
import type { AppState } from "../../types/events";
import { Icon, type IconName } from "../ui";

// Overlay → Rust. The overlay window is non-focusable, so these clicks don't
// steal focus from the user's app (injection still targets it). Best-effort.
function call(cmd: string) {
  void invoke(cmd).catch((err) => console.error(`${cmd} failed:`, err));
}

// Small round control inside the capsule — color block, no outline.
function CapsuleButton({
  name,
  label,
  tone,
  onClick,
}: {
  name: IconName;
  label: string;
  tone: "danger" | "accent";
  onClick: () => void;
}) {
  const accent = tone === "accent";
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className={[
        "inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-full border-0 cursor-pointer",
        "transition-colors duration-150 ease-[var(--ease-out)]",
        accent
          ? "bg-accent-fill text-text-on-accent hover:bg-accent-fill-hover"
          : "bg-gray-alpha-200 text-text-secondary hover:bg-danger-bg hover:text-danger-text",
      ].join(" ")}
    >
      <Icon name={name} size={16} strokeWidth={accent ? 2.25 : 2} />
    </button>
  );
}

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
        "inline-flex h-12 min-w-[200px] items-center gap-2.5 px-2",
        // Recording pins ✕/✓ to the ends (justify-between) so each round button
        // is concentric with the pill's rounded end-arc (8px on all sides);
        // other states center their content.
        view === "recording" ? "justify-between" : "justify-center",
        // No backdrop-blur: on the transparent macOS overlay window it renders as
        // an opaque white box instead of frosting the desktop. surface-capsule is
        // ~95% opaque dark, so the pill reads solid without it.
        // corner-shape: iOS-style continuous (squircle) corners — curvature ramps
        // in smoothly instead of jumping at the arc/line join. Tune the exponent:
        // 2 = plain round, ~3 = soft iOS squircle, higher = flatter. Needs a recent
        // WebKit; older ones ignore it and fall back to rounded-full.
        "rounded-full [corner-shape:superellipse(3)] border-0 bg-surface-capsule text-text-primary shadow-capsule",
        "transition-all duration-200 ease-[var(--ease-out)]",
        visible ? "opacity-100 translate-y-0" : "pointer-events-none translate-y-2 opacity-0",
      ].join(" ")}
    >
      {view === "recording" ? (
        <>
          <CapsuleButton name="x" label="取消" tone="danger" onClick={() => call("cancel_recording")} />
          <div className="flex items-center gap-2.5 px-0.5">
            <Waveform levels={levels} />
            <span className="min-w-[34px] text-center font-mono text-xs text-text-tertiary">
              {formatElapsed(elapsed)}
            </span>
          </div>
          <CapsuleButton name="check" label="完成并润色" tone="accent" onClick={() => call("confirm_recording")} />
        </>
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
