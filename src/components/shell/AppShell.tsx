import type { ReactNode } from "react";

interface AppShellProps {
  sidebar?: ReactNode;
  header?: ReactNode;
  children: ReactNode;
  maxContentWidth?: string;
  bleed?: boolean;
  panel?: boolean;
}

/**
 * The desktop window shell: a left rail (`sidebar`) beside the content area.
 * By default the content sits in a rounded, inset "panel" floating on the window
 * frame (set `panel={false}` for a flush, edge-to-edge content area). Optional
 * sticky `header` sits above the content. Sized to fill its container.
 */
export function AppShell({
  sidebar,
  header,
  children,
  maxContentWidth = "640px",
  bleed = false,
  panel = true,
}: AppShellProps) {
  return (
    <div className="flex h-full w-full overflow-hidden bg-surface-sidebar text-text-primary font-sans">
      {sidebar}
      <main
        className={[
          "flex flex-1 min-w-0 flex-col overflow-hidden",
          // Top margin matches the sidebar's pt-9 so the panel's top edge lines
          // up with the "Audie" brand row; other sides keep the 8px float.
          panel ? "bg-surface-app rounded-md mt-9 mr-2 mb-2 ml-2" : "bg-transparent",
        ].join(" ")}
      >
        {header ? <div className="flex h-13 shrink-0 items-center px-6">{header}</div> : null}
        <div className="flex flex-1 min-h-0 flex-col">
          {bleed ? (
            children
          ) : (
            <div className="flex-1 overflow-y-auto">
              <div className="mx-auto px-6 pt-7 pb-12" style={{ maxWidth: maxContentWidth }}>
                {children}
              </div>
            </div>
          )}
        </div>
      </main>
    </div>
  );
}

interface ShellHeaderProps {
  title: ReactNode;
  subtitle?: ReactNode;
  actions?: ReactNode;
}

/** Standard content header: a title with optional subtitle and trailing actions. */
export function ShellHeader({ title, subtitle, actions }: ShellHeaderProps) {
  return (
    <div className="flex w-full items-center justify-between gap-4">
      <div className="min-w-0">
        <div className="text-base font-semibold leading-6 tracking-[-0.32px] text-text-primary">{title}</div>
        {subtitle ? <div className="mt-px text-xs text-text-tertiary">{subtitle}</div> : null}
      </div>
      {actions ? <div className="flex shrink-0 items-center gap-2">{actions}</div> : null}
    </div>
  );
}
