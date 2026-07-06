import { useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import {
  AppErrorSchema,
  EVENT_ERROR,
  EVENT_STATE_CHANGE,
  StateChangeSchema,
  type AppErrorEvent,
} from "../../../types/events";
import { openExternal } from "../../../lib/open";
import { useI18n } from "../../../i18n";
import { StepHeader } from "./StepHeader";
import type { TestPhase } from "./types";

// "Try it" step: the user focuses the textarea, presses the real trigger (fn) and
// speaks; the dictation pipeline injects into the focused box. Success/failure is
// judged from the Rust state-change/error events — NOT the textarea contents — so
// it stays reliable regardless of where injection focus lands. Reuses the real
// hotkey path; no new backend command.
export function TestStep() {
  const { t } = useI18n();
  const [phase, setPhase] = useState<TestPhase>("idle");
  const [err, setErr] = useState<AppErrorEvent | null>(null);

  useEffect(() => {
    const unsubs: UnlistenFn[] = [];
    let cancelled = false;
    const track = (fn: UnlistenFn) => {
      if (cancelled) {
        fn();
      } else {
        unsubs.push(fn);
      }
    };

    listen(EVENT_STATE_CHANGE, (e) => {
      const parsed = StateChangeSchema.safeParse(e.payload);
      if (!parsed.success) return;
      switch (parsed.data.to) {
        case "RECORDING":
          setErr(null);
          setPhase("recording");
          break;
        case "PROCESSING":
          setPhase("processing");
          break;
        case "SUCCESS":
          setPhase("success");
          break;
        case "ERROR":
          setPhase("idle"); // the message arrives via the `error` event below
          break;
        // IDLE (incl. the ~150ms post-SUCCESS settle) leaves "success" shown.
      }
    })
      .then(track)
      .catch((e2: unknown) => {
        console.error("test state-change subscribe failed:", e2);
      });

    listen(EVENT_ERROR, (e) => {
      const parsed = AppErrorSchema.safeParse(e.payload);
      if (parsed.success) {
        setErr(parsed.data);
        setPhase("idle");
      }
    })
      .then(track)
      .catch((e2: unknown) => {
        console.error("test error subscribe failed:", e2);
      });

    return () => {
      cancelled = true;
      unsubs.forEach((fn) => {
        fn();
      });
    };
  }, []);

  return (
    <>
      <StepHeader title={t("setup.test.title")} desc={t("setup.test.desc")} tag={t("setup.optional")} />
      <textarea
        rows={3}
        placeholder={t("setup.test.placeholder")}
        className="w-full resize-none rounded-md bg-surface-card px-3.5 py-3 text-sm text-text-primary outline-none placeholder:text-text-tertiary focus:ring-1 focus:ring-accent-fill"
      />
      <div className="mt-3 text-xs">
        {phase === "recording" ? (
          <span className="text-accent-text">{t("setup.test.recording")}</span>
        ) : phase === "processing" ? (
          <span className="text-text-secondary">{t("setup.test.processing")}</span>
        ) : phase === "success" ? (
          <span className="text-success-text">{t("setup.test.success")}</span>
        ) : err ? (
          <span className="text-warning-text">
            {err.message}
            {err.code === "permission" ? (
              <button
                className="ml-1 underline"
                onClick={() => {
                  openExternal("x-apple.systempreferences:com.apple.preference.security");
                }}
              >
                {t("setup.permission.openSettings")}
              </button>
            ) : (
              t("setup.test.retryAfterFix")
            )}
          </span>
        ) : (
          <span className="text-text-tertiary">{t("setup.test.help")}</span>
        )}
      </div>
    </>
  );
}
