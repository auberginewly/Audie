import { z } from "zod";

// Separate from dictation audio-level so preview never drives the capsule waveform.
export const AudioLevelSchema = z.object({ level: z.number().min(0).max(1) });
export type AudioLevel = z.infer<typeof AudioLevelSchema>;

export const EVENT_MIC_MONITOR_LEVEL = "mic-monitor-level";
