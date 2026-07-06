import { Badge } from "../../ui";

interface StepHeaderProps {
  title: string;
  desc: string;
  tag?: string;
}

export function StepHeader({ title, desc, tag }: StepHeaderProps) {
  return (
    <div className="mb-[18px]">
      <div className="flex items-center gap-2.5">
        <h2 className="text-lg font-semibold leading-[1.3] text-text-primary">{title}</h2>
        {tag ? <Badge tone="neutral">{tag}</Badge> : null}
      </div>
      <p className="mt-[7px] max-w-[44ch] text-[13px] leading-[18px] text-text-secondary">{desc}</p>
    </div>
  );
}
