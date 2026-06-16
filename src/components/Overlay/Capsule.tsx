// The recording capsule. Pure mirror of `state-change` — no business logic.
// PROJECT_SPEC.md §3.8 — RECORDING shows the placeholder "录音中…" (P0.1);
// P2 will swap in real partial transcript text.

import { useRecordingStore } from "../../store/recording";

export function Capsule() {
  const state = useRecordingStore((s) => s.state);

  // P0.1: only Idle vs Recording matter. Other states arrive in P0.4+.
  const visible = state === "RECORDING";

  return (
    <div
      className={`h-14 w-80 rounded-full bg-base-300/85 backdrop-blur-md
                  shadow-lg flex items-center justify-center
                  transition-all duration-150 ease-out
                  ${visible ? "opacity-100 translate-y-0" : "opacity-0 translate-y-2 pointer-events-none"}`}
    >
      <span className="text-sm text-base-content font-medium tracking-wide">
        {state === "RECORDING" ? "录音中…" : ""}
      </span>
    </div>
  );
}
