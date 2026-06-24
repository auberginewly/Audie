// Entry for the overlay webview (a separate Tauri window).
// Transparent body so the capsule appears to float on top of every app.

import React from "react";
import ReactDOM from "react-dom/client";
import { Capsule } from "./components/Overlay";
import { useRecordingFlow } from "./hooks/useRecordingFlow";
import "./index.css";

function OverlayRoot() {
  useRecordingFlow();
  return (
    <div className="w-screen h-screen flex items-end justify-center pb-7 bg-transparent">
      <Capsule />
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <OverlayRoot />
  </React.StrictMode>,
);
