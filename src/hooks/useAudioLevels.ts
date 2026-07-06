// Subscribe to Rust `audio-level` events. Returns a 4-slot ring (newest →
// oldest) that the capsule folds around its center: bar distance d from the
// middle reads ring[d], so the newest peak pulses the center and ripples out.
// At ~30 FPS that's ~130ms of history — enough motion without React-side
// animation. PROJECT_SPEC.md §3.6 / §3.8.

import { useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { AudioLevelSchema, EVENT_AUDIO_LEVEL, type AudioLevel } from "../types/events";

export type LevelRing = readonly [number, number, number, number];

const ZERO: LevelRing = [0, 0, 0, 0];

export function useAudioLevels(): LevelRing {
  const [levels, setLevels] = useState<LevelRing>(ZERO);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;

    listen<AudioLevel>(EVENT_AUDIO_LEVEL, (event) => {
      const parsed = AudioLevelSchema.safeParse(event.payload);
      if (!parsed.success) {
        console.warn("invalid audio-level payload", parsed.error, event.payload);
        return;
      }
      setLevels((prev) => [parsed.data.level, prev[0], prev[1], prev[2]]);
    })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((err) => {
        console.error("failed to subscribe audio-level:", err);
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return levels;
}
