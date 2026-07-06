import type { CSSProperties, ReactNode } from "react";
import { Icon, type IconName } from "../ui";

interface SettingSectionProps {
  icon?: IconName;
  title: ReactNode;
  description?: ReactNode;
  action?: ReactNode;
  children: ReactNode;
  cardStyle?: CSSProperties;
}

/** A titled settings section: icon + title above a filled card holding rows. */
export function SettingSection({ icon, title, description, action, children, cardStyle }: SettingSectionProps) {
  return (
    <section className="mb-7">
      <div className="mb-2.5 flex items-start justify-between gap-3 pl-1">
        <div className="flex min-w-0 items-center gap-2.5">
          {icon ? <Icon name={icon} size={16} className="mt-px text-text-tertiary" /> : null}
          <div className="min-w-0">
            <h2 className="text-sm font-semibold leading-5 tracking-[-0.28px] text-text-primary">{title}</h2>
            {description ? <p className="mt-0.5 text-xs text-text-tertiary">{description}</p> : null}
          </div>
        </div>
        {action ? <div className="shrink-0">{action}</div> : null}
      </div>
      <div className="overflow-hidden rounded-md bg-surface-card" style={cardStyle}>
        {children}
      </div>
    </section>
  );
}

interface SettingRowProps {
  label: ReactNode;
  description?: ReactNode;
  control?: ReactNode;
  icon?: IconName;
  divider?: boolean;
}

/** A single row inside a SettingSection. Rows stack with inset hairline dividers. */
export function SettingRow({ label, description, control, icon, divider = true }: SettingRowProps) {
  return (
    <div className="relative flex items-center justify-between gap-4 px-3.5 py-3">
      {divider ? <div className="absolute inset-x-3.5 top-0 h-px bg-border-subtle" /> : null}
      <div className="flex min-w-0 items-center gap-2.5">
        {icon ? <Icon name={icon} size={16} className="text-text-tertiary" /> : null}
        <div className="min-w-0">
          <div className="text-sm text-text-primary">{label}</div>
          {description ? <div className="mt-px text-xs text-text-tertiary">{description}</div> : null}
        </div>
      </div>
      {control ? <div className="shrink-0">{control}</div> : null}
    </div>
  );
}
