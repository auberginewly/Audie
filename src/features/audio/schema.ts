import { z } from "zod";

// Microphone enumerated by `list_microphones` (Rust). `id` is the cpal device
// name and the value persisted into `input_device`; "" / "auto" = automatic.
export const AudioDeviceSchema = z.object({
  id: z.string(),
  label: z.string(),
});
export type AudioDevice = z.infer<typeof AudioDeviceSchema>;

// auto_input_device (Rust): the device the automatic path resolves to, or null.
export const AutoDeviceSchema = z.string().nullable();
