// Entry for the overlay webview (a separate Tauri window).
// Transparent body so the capsule appears to float on top of every app.

import React from "react";
import ReactDOM from "react-dom/client";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Capsule } from "./components/Overlay";
import { useRecordingFlow } from "./hooks/useRecordingFlow";
import { I18nProvider, isLanguage, type Language } from "./i18n";
import { SettingsSchema } from "./types/settings";
import "./index.css";

function OverlayRoot() {
  useRecordingFlow();
  const [language, setLanguage] = useState<Language>("zh-Hans");

  useEffect(() => {
    const setFromRaw = (raw: unknown) => {
      const parsed = SettingsSchema.safeParse(raw);
      if (parsed.success && isLanguage(parsed.data.ui_language)) setLanguage(parsed.data.ui_language);
    };
    invoke("get_settings")
      .then(setFromRaw)
      .catch((err) => {
        console.error("overlay settings load failed:", err);
      });
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listen("settings-updated", (event) => {
      setFromRaw(event.payload);
    })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch((err) => {
        console.error("overlay settings subscribe failed:", err);
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return (
    <I18nProvider language={language}>
      <div className="w-screen h-screen flex items-end justify-center pb-7 bg-transparent">
        <Capsule />
      </div>
    </I18nProvider>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <OverlayRoot />
  </React.StrictMode>,
);
