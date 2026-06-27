import {
  Children,
  isValidElement,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { Icon } from "./Icon";

export type SelectSize = "sm" | "md" | "lg";

const HEIGHT: Record<SelectSize, string> = { sm: "h-7", md: "h-8", lg: "h-10" };

type Opt = { value: string; label: string };

function readOptions(children: ReactNode): Opt[] {
  const out: Opt[] = [];
  Children.forEach(children, (child) => {
    if (!isValidElement(child) || child.type !== "option") return;
    const props = child.props as { value?: string; children?: ReactNode };
    out.push({ value: String(props.value ?? ""), label: Children.toArray(props.children).join("") });
  });
  return out;
}

type SelectProps = {
  size?: SelectSize;
  value?: string;
  defaultValue?: string;
  onChange?: (e: { target: { value: string } }) => void;
  children: ReactNode;
  className?: string;
};

/**
 * Audie-styled dropdown. Reads `<option>` children but renders a custom trigger +
 * popover matching the app surfaces — never a native system select. API-compatible:
 * `value`/`defaultValue`/`onChange` (onChange receives an event-like `{ target: { value } }`).
 *
 * The popover is portaled to <body> with fixed positioning so it escapes the
 * settings panel's `overflow` clipping (an inline absolute popover got cut off), and
 * is height-capped with internal scroll so a long list never runs off-screen.
 */
export function Select({ size = "md", value, defaultValue, onChange, children, className = "" }: SelectProps) {
  const options = readOptions(children);
  const controlled = value !== undefined;
  const [internal, setInternal] = useState<string>(defaultValue ?? options[0]?.value ?? "");
  const current = controlled ? value : internal;

  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const [rect, setRect] = useState<{ top: number; left: number; width: number } | null>(null);
  const selected = options.find((o) => o.value === current) ?? options[0];

  // Anchor the portaled popover under the trigger (fixed coords from its rect).
  useLayoutEffect(() => {
    if (!open || !triggerRef.current) return;
    const r = triggerRef.current.getBoundingClientRect();
    setRect({ top: r.bottom + 6, left: r.left, width: r.width });
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const t = e.target as Node;
      if (triggerRef.current?.contains(t) || popoverRef.current?.contains(t)) return;
      setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    // Fixed coords go stale on scroll/resize — just close instead of tracking.
    const onReflow = () => setOpen(false);
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    window.addEventListener("resize", onReflow);
    window.addEventListener("scroll", onReflow, true);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
      window.removeEventListener("resize", onReflow);
      window.removeEventListener("scroll", onReflow, true);
    };
  }, [open]);

  const pick = (v: string) => {
    setOpen(false);
    if (!controlled) setInternal(v);
    onChange?.({ target: { value: v } });
  };

  return (
    <div className={["relative w-full", className].join(" ")}>
      <button
        ref={triggerRef}
        type="button"
        onClick={() => setOpen((o) => !o)}
        className={[
          "flex w-full items-center gap-2.5 px-2.5 text-left outline-none",
          "rounded-sm border border-transparent bg-gray-200 text-text-primary",
          "font-sans text-[13px] cursor-pointer",
          "transition-colors duration-150 ease-[var(--ease-out)]",
          HEIGHT[size],
        ].join(" ")}
      >
        <span className="flex-1 min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{selected?.label}</span>
        <span
          className={[
            "text-text-tertiary transition-transform duration-150 ease-[var(--ease-out)]",
            open ? "rotate-180" : "",
          ].join(" ")}
        >
          <Icon name="chevron-down" size={14} />
        </span>
      </button>

      {open && rect
        ? createPortal(
            <div
              ref={popoverRef}
              role="listbox"
              style={{ position: "fixed", top: rect.top, left: rect.left, width: rect.width }}
              className={[
                "z-[100] max-h-60 overflow-y-auto p-[5px]",
                "flex flex-col gap-[3px] rounded-md bg-surface-overlay shadow-popover",
              ].join(" ")}
            >
              {options.map((o) => {
                const active = o.value === current;
                return (
                  <button
                    key={o.value}
                    role="option"
                    aria-selected={active}
                    onClick={() => pick(o.value)}
                    className={[
                      "flex h-9 w-full items-center gap-[9px] px-2.5 text-left",
                      "rounded-sm border-0 font-sans text-[13px] cursor-pointer",
                      active
                        ? "bg-gray-alpha-200 text-text-primary font-medium"
                        : "bg-transparent text-text-secondary hover:bg-gray-alpha-100",
                    ].join(" ")}
                  >
                    <span className="flex-1 min-w-0">{o.label}</span>
                    {active ? <Icon name="check" size={15} className="text-accent-text" /> : null}
                  </button>
                );
              })}
            </div>,
            document.body,
          )
        : null}
    </div>
  );
}
