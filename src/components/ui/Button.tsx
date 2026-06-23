import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Icon, type IconName } from "./Icon";

export type ButtonVariant = "primary" | "secondary" | "accent" | "danger" | "ghost";
export type ButtonSize = "sm" | "md" | "lg";

// Variant = emphasis: primary is the single most important action; secondary the
// bordered default; ghost low-emphasis; accent Audie aubergine; danger destructive.
const VARIANT: Record<ButtonVariant, string> = {
  primary: "bg-gray-1000 text-text-on-solid hover:bg-gray-900",
  secondary: "bg-gray-200 text-text-primary hover:bg-gray-300",
  accent: "bg-accent-fill text-text-on-accent hover:bg-accent-fill-hover",
  danger: "bg-danger-fill text-white hover:bg-red-900",
  ghost: "bg-transparent text-text-primary hover:bg-gray-alpha-100",
};

const SIZE: Record<ButtonSize, { box: string; text: string; gap: string; icon: number }> = {
  sm: { box: "h-7 px-2.5", text: "text-xs", gap: "gap-1.5", icon: 14 },
  md: { box: "h-8 px-3", text: "text-sm", gap: "gap-[7px]", icon: 16 },
  lg: { box: "h-10 px-4", text: "text-sm", gap: "gap-2", icon: 16 },
};

type ButtonProps = {
  variant?: ButtonVariant;
  size?: ButtonSize;
  icon?: IconName;
  iconRight?: IconName;
  block?: boolean;
  children?: ReactNode;
} & ButtonHTMLAttributes<HTMLButtonElement>;

export function Button({
  variant = "secondary",
  size = "md",
  icon,
  iconRight,
  block = false,
  disabled = false,
  className = "",
  children,
  ...rest
}: ButtonProps) {
  const s = SIZE[size];
  return (
    <button
      disabled={disabled}
      className={[
        block ? "flex w-full" : "inline-flex",
        "items-center justify-center whitespace-nowrap rounded-sm border border-transparent",
        "font-sans font-medium transition-colors duration-150 ease-[var(--ease-out)]",
        "disabled:cursor-not-allowed disabled:opacity-45",
        s.box,
        s.text,
        s.gap,
        VARIANT[variant],
        className,
      ].join(" ")}
      {...rest}
    >
      {icon ? <Icon name={icon} size={s.icon} /> : null}
      {children}
      {iconRight ? <Icon name={iconRight} size={s.icon} /> : null}
    </button>
  );
}
