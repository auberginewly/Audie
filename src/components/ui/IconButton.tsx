import type { ButtonHTMLAttributes } from "react";
import { Icon, type IconName } from "./Icon";

export type IconButtonSize = "sm" | "md" | "lg";

const BOX: Record<IconButtonSize, string> = { sm: "w-7 h-7", md: "w-8 h-8", lg: "w-10 h-10" };
const ICON: Record<IconButtonSize, number> = { sm: 15, md: 16, lg: 18 };

type IconButtonProps = {
  name: IconName;
  label: string;
  size?: IconButtonSize;
  variant?: "ghost" | "outline";
} & Omit<ButtonHTMLAttributes<HTMLButtonElement>, "name">;

/** Square icon-only button. Ghost by default; calm hover tint. */
export function IconButton({
  name,
  label,
  size = "md",
  variant = "ghost",
  disabled = false,
  className = "",
  ...rest
}: IconButtonProps) {
  return (
    <button
      aria-label={label}
      title={label}
      disabled={disabled}
      className={[
        "inline-flex items-center justify-center rounded-sm border",
        "text-text-secondary hover:text-text-primary hover:bg-gray-alpha-200",
        "transition-colors duration-150 ease-[var(--ease-out)]",
        "disabled:cursor-not-allowed disabled:opacity-45",
        variant === "outline" ? "border-border-default" : "border-transparent",
        BOX[size],
        className,
      ].join(" ")}
      {...rest}
    >
      <Icon name={name} size={ICON[size]} />
    </button>
  );
}
