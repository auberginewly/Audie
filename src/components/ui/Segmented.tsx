export type SegmentedOption<T extends string> = { id: T; label: string };

type SegmentedProps<T extends string> = {
  value: T;
  options: SegmentedOption<T>[];
  onChange: (id: T) => void;
};

/** A pill-track segmented control. Active segment lifts a tonal step. */
export function Segmented<T extends string>({ value, options, onChange }: SegmentedProps<T>) {
  return (
    <div className="inline-flex gap-0.5 rounded-full bg-gray-200 p-[3px]">
      {options.map((o) => {
        const on = o.id === value;
        return (
          <button
            key={o.id}
            onClick={() => onChange(o.id)}
            className={[
              "h-[26px] rounded-full border-0 px-3.5 font-sans text-[13px] cursor-pointer",
              "transition-colors duration-150 ease-[var(--ease-out)]",
              on ? "bg-gray-300 text-text-primary font-medium" : "bg-transparent text-text-tertiary font-normal",
            ].join(" ")}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}
