import { useEffect, useRef, useState, type ReactNode } from "react";
import { Icon, type IconName } from "./Icon";

export type MenuItem =
  { type: "divider" } | { icon?: IconName; label: string; tone?: "default" | "danger"; onClick?: () => void };

interface MenuProps {
  trigger: ReactNode;
  items: MenuItem[];
  align?: "left" | "right";
  width?: number;
}

/**
 * A click-triggered popover menu. Wrap any trigger; opens below it and closes on
 * outside-click, Escape, or item select. Scoped to its own relative wrapper.
 */
export function Menu({ trigger, items, align = "right", width = 208 }: MenuProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLSpanElement>(null);

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

  return (
    <span ref={ref} className="relative inline-flex">
      <span
        className="inline-flex"
        onClick={(e) => {
          e.stopPropagation();
          setOpen((o) => !o);
        }}
      >
        {trigger}
      </span>
      {open ? (
        <div
          role="menu"
          style={{ width }}
          className={[
            "absolute top-[calc(100%+6px)] z-50 p-1",
            "rounded-md bg-surface-overlay shadow-popover",
            align === "right" ? "right-0" : "left-0",
          ].join(" ")}
        >
          {items.map((it, i) => {
            if ("type" in it) {
              return <div key={i} className="my-1 h-px bg-border-subtle" />;
            }
            const danger = it.tone === "danger";
            return (
              <button
                key={i}
                role="menuitem"
                onClick={(e) => {
                  e.stopPropagation();
                  setOpen(false);
                  it.onClick?.();
                }}
                className={[
                  "flex h-8 w-full items-center gap-2.5 rounded-sm border-0 px-2.5 text-left",
                  "font-sans text-[13px] cursor-pointer",
                  danger
                    ? "bg-transparent text-danger-text hover:bg-red-100"
                    : "bg-transparent text-text-secondary hover:bg-gray-alpha-200 hover:text-text-primary",
                ].join(" ")}
              >
                {it.icon ? <Icon name={it.icon} size={15} /> : null}
                <span className="flex-1">{it.label}</span>
              </button>
            );
          })}
        </div>
      ) : null}
    </span>
  );
}
