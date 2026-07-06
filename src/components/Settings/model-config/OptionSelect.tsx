import { useState } from "react";

import { Input, Select } from "../../ui";

const CUSTOM_OPTION = "__custom__";

interface OptionSelectProps {
  options: { id: string; title: string }[];
  value: string;
  placeholder: string;
  onChange: (next: string) => void;
  customLabel: string;
}

// Curated dropdown with a free-text escape hatch for provider/model ids that are
// newer than Audie's bundled list.
export function OptionSelect({ options, value, placeholder, onChange, customLabel }: OptionSelectProps) {
  const inList = options.some((o) => o.id === value);
  const [custom, setCustom] = useState(value !== "" && !inList);

  if (custom) {
    return (
      <div className="flex flex-col gap-[7px]">
        <Select
          value={CUSTOM_OPTION}
          onChange={(e) => {
            if (e.target.value !== CUSTOM_OPTION) {
              setCustom(false);
              onChange(e.target.value);
            }
          }}
        >
          {options.map((o) => (
            <option key={o.id} value={o.id}>
              {o.title}
            </option>
          ))}
          <option value={CUSTOM_OPTION}>{customLabel}</option>
        </Select>
        <Input
          mono
          value={value}
          onChange={(e) => {
            onChange(e.target.value);
          }}
          placeholder={placeholder}
        />
      </div>
    );
  }

  return (
    <Select
      value={inList ? value : (options[0]?.id ?? "")}
      onChange={(e) => {
        if (e.target.value === CUSTOM_OPTION) {
          setCustom(true);
          return;
        }
        onChange(e.target.value);
      }}
    >
      {options.map((o) => (
        <option key={o.id} value={o.id}>
          {o.title}
        </option>
      ))}
      <option value={CUSTOM_OPTION}>{customLabel}</option>
    </Select>
  );
}
