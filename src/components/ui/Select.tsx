import {
  Children,
  isValidElement,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
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
 */
export function Select({ size = "md", value, defaultValue, onChange, children, className = "" }: SelectProps) {
  const options = readOptions(children);
  const controlled = value !== undefined;
  const [internal, setInternal] = useState<string>(defaultValue ?? options[0]?.value ?? "");
  const current = controlled ? value : internal;

  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const selected = options.find((o) => o.value === current) ?? options[0];

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const pick = (v: string) => {
    setOpen(false);
    if (!controlled) setInternal(v);
    onChange?.({ target: { value: v } });
  };

  return (
    <div ref={ref} className={["relative w-full", className].join(" ")}>
      <button
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

      {open ? (
        <div
          role="listbox"
          className={[
            "absolute left-0 right-0 top-[calc(100%+6px)] z-[60] p-[5px]",
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
        </div>
      ) : null}
    </div>
  );
}
