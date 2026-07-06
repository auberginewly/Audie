import { Icon } from "../../ui";
import { useI18n } from "../../../i18n";
import { NUMBERED, STEP_LABEL, type StepId } from "./types";

function StepItem({
  index,
  label,
  sub,
  current,
  done,
  onClick,
}: {
  index: number;
  label: string;
  sub: string;
  current: boolean;
  done: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={[
        "flex w-full items-center gap-2.5 rounded-sm border-0 px-2.5 py-2.5 text-left cursor-pointer",
        "transition-colors duration-150 ease-[var(--ease-out)]",
        current ? "bg-gray-alpha-200" : "bg-transparent hover:bg-gray-alpha-100",
      ].join(" ")}
    >
      <span
        className={[
          "inline-flex h-[22px] w-[22px] shrink-0 items-center justify-center rounded-full text-[11px] font-semibold",
          current ? "bg-accent-fill text-surface-card" : "bg-gray-200 text-text-tertiary",
        ].join(" ")}
      >
        {index + 1}
      </span>
      <span className="min-w-0">
        <span
          className={[
            "block text-[13px]",
            current ? "font-medium text-text-primary" : done ? "text-text-primary" : "text-text-secondary",
          ].join(" ")}
        >
          {label}
        </span>
        <span className="mt-px block text-[10px] text-text-tertiary">{sub}</span>
      </span>
      {done ? <Icon name="check" size={15} className="ml-auto shrink-0 text-success-text" /> : null}
    </button>
  );
}

interface StepNavProps {
  current: StepId;
  ids: StepId[];
  doneMap: Record<string, boolean>;
  subMap: Record<string, string>;
  onSelect: (index: number) => void;
}

export function StepNav({ current, ids, doneMap, subMap, onSelect }: StepNavProps) {
  const { t } = useI18n();

  return (
    <nav className="flex w-[212px] shrink-0 flex-col bg-surface-sidebar px-2.5 pb-2.5 pt-4">
      <div className="px-2.5 pb-3.5">
        <div className="text-sm font-semibold text-text-primary">{t("setup.nav.title")}</div>
        <div className="mt-[3px] text-xs text-text-tertiary">{t("setup.nav.desc")}</div>
      </div>
      <div className="flex flex-col gap-0.5">
        {NUMBERED.map((sid, i) => (
          <StepItem
            key={sid}
            index={i}
            label={t(STEP_LABEL[sid])}
            sub={subMap[sid]}
            current={sid === current}
            done={doneMap[sid]}
            onClick={() => {
              onSelect(ids.indexOf(sid));
            }}
          />
        ))}
      </div>
    </nav>
  );
}
