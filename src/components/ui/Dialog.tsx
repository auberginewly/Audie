import type { ReactNode } from "react";
import { Icon, type IconName } from "./Icon";

type DialogProps = {
  open: boolean;
  onClose: () => void;
  icon?: IconName;
  title?: ReactNode;
  children?: ReactNode;
  actions?: ReactNode;
  width?: number;
};

/**
 * A centered modal dialog. Backdrop dims the window; click-outside dismisses.
 * Optional accent icon badge in the header. Fills its nearest positioned
 * ancestor (the app window) via `position: absolute`.
 */
export function Dialog({ open, onClose, icon, title, children, actions, width = 380 }: DialogProps) {
  if (!open) return null;
  return (
    <div
      onMouseDown={onClose}
      className="absolute inset-0 z-[60] flex items-center justify-center bg-black/50 p-6 backdrop-blur-[2px]"
    >
      <div
        role="dialog"
        aria-modal="true"
        onMouseDown={(e) => e.stopPropagation()}
        style={{ width }}
        className="max-w-full overflow-hidden rounded-md bg-surface-overlay shadow-modal"
      >
        {icon || title ? (
          <div className="flex items-center gap-[11px] px-[18px] pt-[18px]">
            {icon ? (
              <span className="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-sm bg-accent-bg text-accent-text">
                <Icon name={icon} size={16} />
              </span>
            ) : null}
            {title ? (
              <div className="text-base font-semibold leading-6 tracking-[-0.32px] text-text-primary">{title}</div>
            ) : null}
          </div>
        ) : null}

        {children ? (
          <div className="px-[18px] pb-[18px] pt-2.5 text-[13px] leading-[18px] text-text-secondary">{children}</div>
        ) : null}

        {actions ? <div className="flex justify-end gap-2 px-[18px] pb-[18px]">{actions}</div> : null}
      </div>
    </div>
  );
}
