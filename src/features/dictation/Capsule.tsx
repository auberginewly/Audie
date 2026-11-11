// The recording capsule — Audie's signature overlay. Pure mirror of the Rust
// state machine (no business logic; PROJECT_SPEC §3.8 / §6.2). Live states render
// the pill (recording / transcribing / polishing / success); terminal states
// render a rounded toast card with actions (cancelled→撤销 / polish-unavailable→
// 去设置 / error→插入原文·重试). The backend keeps the take so those actions can
// resume it (fe.8c).

import { useEffect, useRef, useState } from "react";

import { useAudioLevels } from "./useAudioLevels";
import { callOverlayCommand as call } from "./overlayCommands";
import { ToastCard, CardButton, errorActions } from "./CapsuleToast";
import { Waveform } from "./Waveform";
import { useRecordingStore } from "./recording";
import type { AppState } from "./events";
import { useI18n } from "../../i18n";
import { Icon, type IconName } from "../../components/ui";

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
      onMouseDown={(e) => {
        e.preventDefault();
      }}
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
  "recording" | "transcribing" | "polishing" | "success" | "polish-unavailable" | "error" | "cancelled" | null;

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
  const { t } = useI18n();
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
    return () => {
      window.clearInterval(id);
    };
  }, [state]);

  if (view === null) return null;

  // ── Terminal toasts (rounded card, keyed so it re-mounts → rises in) ──
  if (view === "cancelled") {
    return (
      <ToastCard
        key="toast"
        title={t("overlay.cancelled")}
        buttons={
          <CardButton
            label={t("overlay.undo")}
            tone="accent"
            onClick={() => {
              call("undo_last");
            }}
          />
        }
      />
    );
  }
  if (view === "polish-unavailable") {
    const reason = enhanceProgress?.message.trim();
    return (
      <ToastCard
        key="toast"
        title={t("overlay.rawInserted")}
        subtitle={reason?.length ? reason : t("overlay.polishFailed")}
        buttons={
          <CardButton
            label={t("overlay.openSettings")}
            tone="ghost"
            onClick={() => {
              call("open_main_window");
            }}
          />
        }
      />
    );
  }
  if (view === "error") {
    return (
      <ToastCard key="toast" title={error?.message ?? t("overlay.modelError")} buttons={errorActions(error?.code, t)} />
    );
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
          <CapsuleButton
            name="x"
            label={t("overlay.cancel")}
            tone="danger"
            onClick={() => {
              call("cancel_recording");
            }}
          />
          <div className="flex items-center gap-2.5 px-0.5">
            <Waveform levels={levels} />
            <span className="min-w-[34px] text-center font-mono text-xs text-text-tertiary">
              {formatElapsed(elapsed)}
            </span>
          </div>
          <CapsuleButton
            name="check"
            label={t("overlay.finishPolish")}
            tone="accent"
            onClick={() => {
              call("confirm_recording");
            }}
          />
        </>
      ) : null}

      {view === "transcribing" || view === "polishing" ? (
        <>
          <CapsuleButton
            name="x"
            label={t("overlay.cancel")}
            tone="danger"
            onClick={() => {
              call("cancel_recording");
            }}
          />
          <div className="inline-flex items-center gap-2 px-1.5">
            {view === "polishing" ? (
              <span
                className="inline-flex text-aubergine-900"
                style={{ animation: "audie-twinkle 1.3s var(--ease-out) infinite" }}
              >
                <Icon name="sparkles" size={15} strokeWidth={2} />
              </span>
            ) : (
              <span className="inline-flex text-aubergine-900" style={{ animation: "audie-spin 0.8s linear infinite" }}>
                <Icon name="loader" size={15} strokeWidth={2} />
              </span>
            )}
            <span className={LABEL}>{view === "polishing" ? t("overlay.polishing") : t("overlay.transcribing")}</span>
          </div>
          {/* 32px spacer mirrors the ✕ so the spinner+label stays centered. */}
          <span className="w-8 shrink-0" aria-hidden="true" />
        </>
      ) : null}

      {view === "success" ? (
        <div className="inline-flex items-center gap-2 px-2.5">
          <DrawCheck />
          <span className={LABEL}>{t("overlay.inserted")}</span>
        </div>
      ) : null}
    </div>
  );
}
