import type { HTMLAttributes, ReactNode } from "react";

// Keycap labels — show the symbol the user sees printed on the keyboard.
const GLYPHS: Record<string, string> = {
  cmd: "⌘",
  command: "⌘",
  meta: "⌘",
  ctrl: "⌃",
  control: "⌃",
  opt: "⌥",
  option: "⌥",
  alt: "⌥",
  shift: "⇧",
  enter: "↵",
  return: "↵",
  esc: "esc",
  escape: "esc",
  tab: "⇥",
  space: "Space",
  fn: "fn",
  up: "↑",
  down: "↓",
  left: "←",
  right: "→",
};

export type KeycapSize = "sm" | "md";

const METRICS: Record<KeycapSize, string> = {
  sm: "h-5 min-w-5 px-[5px] text-[11px]",
  md: "h-6 min-w-6 px-[7px] text-xs",
};

type KeycapProps = {
  children: ReactNode;
  size?: KeycapSize;
  literal?: boolean;
} & HTMLAttributes<HTMLElement>;

/** A single physical key rendered as a keycap. Pass a known key name or any label. */
export function Keycap({ children, size = "md", literal = false, className = "", ...rest }: KeycapProps) {
  const raw = typeof children === "string" ? children : "";
  const label = literal ? children : (GLYPHS[raw.toLowerCase()] ?? children);
  return (
    <kbd
      className={[
        "inline-flex items-center justify-center rounded-sm leading-none",
        "border border-b-2 border-border-default bg-gray-200 text-text-secondary",
        "font-mono font-medium",
        METRICS[size],
        className,
      ].join(" ")}
      {...rest}
    >
      {label}
    </kbd>
  );
}

interface KeyComboProps {
  keys: string[];
  size?: KeycapSize;
  className?: string;
  literal?: boolean;
}

/** A hotkey combo: an array of keys joined by a thin gap. */
export function KeyCombo({ keys, size = "md", className = "", literal = false }: KeyComboProps) {
  return (
    <span className={["inline-flex items-center gap-1", className].join(" ")}>
      {keys.map((k, i) => (
        <Keycap key={i} size={size} literal={literal}>
          {k}
        </Keycap>
      ))}
    </span>
  );
}
