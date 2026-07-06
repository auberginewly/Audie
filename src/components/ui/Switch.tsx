export type SwitchSize = "sm" | "md";

const TRACK: Record<SwitchSize, string> = { sm: "w-8 h-[18px]", md: "w-[38px] h-[22px]" };
const KNOB: Record<SwitchSize, string> = { sm: "w-3.5 h-3.5", md: "w-[18px] h-[18px]" };
// knob inset = (track height - knob) / 2 → 2px both sizes; off=left, on=right.
const KNOB_POS: Record<SwitchSize, { off: string; on: string }> = {
  sm: { off: "left-0.5", on: "left-[14px]" },
  md: { off: "left-0.5", on: "left-[18px]" },
};

interface SwitchProps {
  checked?: boolean;
  onChange?: (next: boolean) => void;
  disabled?: boolean;
  size?: SwitchSize;
  className?: string;
}

/** Boolean toggle. Accent fill when on. Controlled via `checked` + `onChange`. */
export function Switch({ checked = false, onChange, disabled = false, size = "md", className = "" }: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => !disabled && onChange?.(!checked)}
      className={[
        "relative shrink-0 rounded-full border-0 p-0 transition-colors duration-150 ease-[var(--ease-out)]",
        disabled ? "cursor-not-allowed opacity-45" : "cursor-pointer",
        checked ? "bg-accent-fill" : "bg-gray-400",
        TRACK[size],
        className,
      ].join(" ")}
    >
      <span
        className={[
          "absolute top-0.5 rounded-full bg-white shadow-[0_1px_2px_rgba(0,0,0,0.35)]",
          "transition-[left] duration-150 ease-[var(--ease-out)]",
          KNOB[size],
          checked ? KNOB_POS[size].on : KNOB_POS[size].off,
        ].join(" ")}
      />
    </button>
  );
}
