// The recording capsule — Audie's signature overlay. Pure mirror of the Rust
// state machine (no business logic; PROJECT_SPEC §3.8 / §6.2). Live states render
// the pill (recording / transcribing / polishing / success); terminal states
// render a rounded toast card with actions (cancelled→撤销 / polish-unavailable→
// 去设置 / error→插入原文·重试). The backend keeps the take so those actions can
// resume it (fe.8c).

import { type ReactNode, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { useAudioLevels, type LevelRing } from "../../hooks/useAudioLevels";
import { useRecordingStore } from "../../store/recording";
import type { AppState, ErrorCode } from "../../types/events";
import { Icon, type IconName } from "../ui";

// Overlay → Rust. The overlay is a non-activating NSPanel, so these clicks never
// steal focus from the user's app (injection still targets it). Best-effort.
function call(cmd: string) {
  void invoke(cmd).catch((err) => console.error(`${cmd} failed:`, err));
}

// Small round control inside the pill — color block, no outline.
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
      // Don't take focus on click: a focused web control makes the overlay panel
      // key, stealing keyboard focus from the user's app so the synthesized Cmd+V
      // would paste into nothing. preventDefault keeps their app key; onClick still fires.
      onMouseDown={(e) => e.preventDefault()}
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

// Filled / ghost text button inside a terminal toast card.
function CardButton({
  label,
  tone,
  onClick,
}: {
  label: string;
  tone: "ghost" | "accent";
  onClick: () => void;
}) {
  const accent = tone === "accent";
  return (
    <button
      type="button"
      // Keep the user's app key so 撤销 / 重试 / 插入原文 inject lands at the caret
      // (see CapsuleButton).
      onMouseDown={(e) => e.preventDefault()}
      onClick={onClick}
      className={[
        "inline-flex h-[34px] shrink-0 items-center justify-center rounded-[10px] border-0 px-4 cursor-pointer",
        "text-[13px] font-medium whitespace-nowrap",
        "transition-colors duration-150 ease-[var(--ease-out)]",
        accent
          ? "bg-accent-fill text-text-on-accent hover:bg-accent-fill-hover"
          : "bg-gray-alpha-200 text-text-primary hover:bg-gray-alpha-300",
      ].join(" ")}
    >
      {label}
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

// Terminal-state card: centered title (+ subtitle) over a row of action buttons.
function ToastCard({
  title,
  subtitle,
  buttons,
}: {
  title: string;
  subtitle?: string;
  buttons: ReactNode;
}) {
  return (
    <div
      role="status"
      className={[
        "inline-flex min-w-[200px] max-w-[320px] flex-col items-center gap-3 p-4",
        "rounded-[14px] [corner-shape:superellipse(3)] border-0 bg-surface-capsule text-text-primary shadow-capsule",
      ].join(" ")}
      style={{ animation: "audie-rise 0.22s var(--ease-out)" }}
    >
      <div className="flex flex-col items-center gap-0.5 text-center">
        <span className="text-[13px] font-medium text-balance">{title}</span>
        {subtitle ? <span className="text-xs text-text-tertiary">{subtitle}</span> : null}
      </div>
      {buttons ? <div className="flex flex-wrap items-center justify-center gap-2">{buttons}</div> : null}
    </div>
  );
}

// Error toast actions by category: inject failed → 插入原文 (re-paste) + 重试;
// network / provider failed → 重试; permission / device / internal → message only.
function errorActions(code: ErrorCode | undefined): ReactNode {
  if (code === "inject") {
    return (
      <>
        <CardButton label="插入原文" tone="ghost" onClick={() => call("insert_raw_last")} />
        <CardButton label="重试" tone="accent" onClick={() => call("retry_last")} />
      </>
    );
  }
  if (code === "network" || code === "provider") {
    return <CardButton label="重试" tone="accent" onClick={() => call("retry_last")} />;
  }
  return null;
}

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

  if (view === null) return null;

  // ── Terminal toasts (rounded card, keyed so it re-mounts → rises in) ──
  if (view === "cancelled") {
    return (
      <ToastCard
        key="toast"
        title="转录已取消"
        buttons={<CardButton label="撤销操作" tone="accent" onClick={() => call("undo_last")} />}
      />
    );
  }
  if (view === "polish-unavailable") {
    return (
      <ToastCard
        key="toast"
        title={enhanceProgress?.message ?? "已插入原文"}
        subtitle="未配置润色模型"
        buttons={<CardButton label="去设置" tone="ghost" onClick={() => call("open_main_window")} />}
      />
    );
  }
  if (view === "error") {
    return <ToastCard key="toast" title={error?.message ?? "模型出错了"} buttons={errorActions(error?.code)} />;
  }

  // ── Live pill (recording / transcribing / polishing / success) ──
  return (
    <div
      key="pill"
      role="status"
      className={[
        "inline-flex h-12 min-w-[200px] items-center gap-2.5 px-2",
        // Recording + processing pin ✕ to the left and a matching spacer to the
        // right, so the center content is concentric with the pill's end-arcs;
        // success has no controls and centers its content.
        view === "success" ? "justify-center" : "justify-between",
        // No backdrop-blur: on the transparent macOS overlay it renders as an
        // opaque white box. surface-capsule is ~95% opaque dark, so the pill reads
        // solid without it. corner-shape: iOS-style continuous (squircle) corners.
        "rounded-full [corner-shape:superellipse(3)] border-0 bg-surface-capsule text-text-primary shadow-capsule",
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
        <>
          <CapsuleButton name="x" label="取消" tone="danger" onClick={() => call("cancel_recording")} />
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
          {/* 32px spacer mirrors the ✕ so the spinner+label stays centered. */}
          <span className="w-8 shrink-0" aria-hidden="true" />
        </>
      ) : null}

      {view === "success" ? (
        <div className="inline-flex items-center gap-2 px-2.5">
          <DrawCheck />
          <span className={LABEL}>已插入</span>
        </div>
      ) : null}
    </div>
  );
}
