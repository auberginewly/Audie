import { useState } from "react";
import { Select } from "./Select";

export interface AudioDevice {
  id: string;
  label: string;
}

interface DevicePickerProps {
  devices?: AudioDevice[];
  value?: string;
  onChange?: (id: string) => void;
  level?: number;
  showMeter?: boolean;
  autoLabel?: string;
}

/**
 * Microphone picker — an Audie-styled Select plus an optional live input-level
 * meter. "Automatic" lets Audie pick a sensible built-in/USB mic.
 */
export function DevicePicker({
  devices = [],
  value,
  onChange,
  level = 0,
  showMeter = true,
  autoLabel = "自动检测的麦克风（推荐）",
}: DevicePickerProps) {
  const [internal, setInternal] = useState(value ?? "auto");
  const val = value !== undefined ? value : internal;
  const setVal = (v: string) => {
    setInternal(v);
    onChange?.(v);
  };
  // Meter sensitivity. sqrt = perceptual curve (lifts quiet speech, whose raw
  // peak amplitude reads low); METER_GAIN scales it — raise for more bars, lower
  // if it pegs at full scale too easily.
  const METER_GAIN = 4;
  const clamped = Math.min(1, Math.sqrt(Math.max(0, level) * METER_GAIN));

  return (
    <div className="flex w-full items-center gap-3.5">
      <div className="min-w-0 flex-1">
        <Select
          value={val}
          onChange={(e) => {
            setVal(e.target.value);
          }}
        >
          <option value="auto">{autoLabel}</option>
          {devices.map((d) => (
            <option key={d.id} value={d.id}>
              {d.label}
            </option>
          ))}
        </Select>
      </div>

      {showMeter ? (
        <div className="flex h-8 shrink-0 items-center gap-0.5 rounded-full bg-gray-200 px-2.5">
          {Array.from({ length: 7 }).map((_, i) => {
            const h = 5 + (i / 6) * 13;
            const on = i < clamped * 7;
            return (
              <span
                key={i}
                className={[
                  "w-[3px] rounded-full transition-all duration-100",
                  on ? "bg-accent-fill opacity-100" : "bg-gray-300 opacity-70",
                ].join(" ")}
                style={{ height: `${h}px` }}
              />
            );
          })}
        </div>
      ) : null}
    </div>
  );
}
