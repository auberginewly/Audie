import type { HTMLAttributes, ReactNode } from "react";
import { Icon, type IconName } from "./Icon";

export type BadgeTone = "neutral" | "accent" | "success" | "warning" | "danger";

const TONE: Record<BadgeTone, { chip: string; dot: string }> = {
  neutral: { chip: "bg-gray-alpha-200 text-text-secondary", dot: "bg-gray-600" },
  accent: { chip: "bg-aubergine-100 text-aubergine-900", dot: "bg-aubergine-700" },
  success: { chip: "bg-green-100 text-green-900", dot: "bg-green-700" },
  warning: { chip: "bg-amber-100 text-amber-700", dot: "bg-amber-700" },
  danger: { chip: "bg-red-100 text-red-900", dot: "bg-red-800" },
};

type BadgeProps = {
  tone?: BadgeTone;
  dot?: boolean;
  icon?: IconName;
  children: ReactNode;
} & HTMLAttributes<HTMLSpanElement>;

/** Small status pill. Optional leading dot or icon. Use a tone to signal state. */
export function Badge({ tone = "neutral", dot = false, icon, children, className = "", ...rest }: BadgeProps) {
  const t = TONE[tone];
  return (
    <span
      className={[
        "inline-flex items-center gap-[5px] h-5 rounded-full whitespace-nowrap",
        "font-sans text-xs font-medium leading-4",
        icon || dot ? "pl-[7px] pr-2" : "px-2",
        t.chip,
        className,
      ].join(" ")}
      {...rest}
    >
      {dot && !icon ? <span className={["w-1.5 h-1.5 rounded-full shrink-0", t.dot].join(" ")} /> : null}
      {icon ? <Icon name={icon} size={12} strokeWidth={2} /> : null}
      {children}
    </span>
  );
}
