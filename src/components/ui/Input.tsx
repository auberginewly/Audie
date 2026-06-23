import type { InputHTMLAttributes, TextareaHTMLAttributes } from "react";

export type InputSize = "sm" | "md" | "lg";

const HEIGHT: Record<InputSize, string> = { sm: "h-7", md: "h-8", lg: "h-10" };

const FIELD_BASE =
  "w-full rounded-sm bg-gray-200 text-text-primary text-[13px] outline-none " +
  "transition-colors duration-150 ease-[var(--ease-out)] " +
  "placeholder:text-text-tertiary";

type InputProps = {
  size?: InputSize;
  mono?: boolean;
  invalid?: boolean;
} & Omit<InputHTMLAttributes<HTMLInputElement>, "size">;

/** Filled text input. Mono variant for keys/URLs. `invalid` shows the danger border. */
export function Input({ size = "md", mono = false, invalid = false, className = "", ...rest }: InputProps) {
  return (
    <input
      className={[
        FIELD_BASE,
        "border px-2.5",
        HEIGHT[size],
        invalid ? "border-danger-border" : "border-transparent",
        mono ? "font-mono" : "font-sans",
        className,
      ].join(" ")}
      {...rest}
    />
  );
}

type TextareaProps = {
  mono?: boolean;
} & TextareaHTMLAttributes<HTMLTextAreaElement>;

/** Multi-line text input — prompts, notes. */
export function Textarea({ mono = false, className = "", ...rest }: TextareaProps) {
  return (
    <textarea
      className={[
        FIELD_BASE,
        "border border-transparent px-2.5 py-2 min-h-[76px] leading-[18px] resize-y",
        mono ? "font-mono" : "font-sans",
        className,
      ].join(" ")}
      {...rest}
    />
  );
}
