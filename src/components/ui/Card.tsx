import type { HTMLAttributes, ReactNode } from "react";

type CardProps = {
  inset?: boolean;
  interactive?: boolean;
  children: ReactNode;
} & HTMLAttributes<HTMLDivElement>;

/** A surface container. Default = filled gray-100 card (no border; tonal hierarchy). */
export function Card({ inset = false, interactive = false, children, className = "", ...rest }: CardProps) {
  return (
    <div
      className={[
        "rounded-md p-4",
        inset ? "bg-surface-inset" : "bg-surface-card",
        interactive
          ? "cursor-pointer transition-colors duration-150 ease-[var(--ease-out)] hover:bg-surface-card-hover"
          : "",
        className,
      ].join(" ")}
      {...rest}
    >
      {children}
    </div>
  );
}
