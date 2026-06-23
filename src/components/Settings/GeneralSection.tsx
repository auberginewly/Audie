// 通用 — keyboard · language · device · system · permissions · input. Only the
// hotkey is backed (maps to the preset enum); language/device/system toggles/
// permissions/input are mock for visual fidelity (see plan).

import { useState } from "react";

import type { Hotkey, Settings } from "../../types/settings";
import { Badge, DevicePicker, Select, Switch } from "../ui";
import { SettingSection, SettingRow } from "./SettingSection";
import { HotkeyRecorder } from "./HotkeyRecorder";
import { PermissionRow } from "./PermissionRow";

// mock: a Switch holding its own demo state, for unbacked rows.
function MockSwitch({ defaultOn }: { defaultOn?: boolean }) {
  const [on, setOn] = useState(!!defaultOn);
  return <Switch checked={on} onChange={setOn} />;
}

type GeneralSectionProps = {
  settings: Settings;
  update: (patch: Partial<Settings>) => void;
};

export function GeneralSection({ settings, update }: GeneralSectionProps) {
  return (
    <>
      <SettingSection icon="command" title="快捷键">
        <SettingRow
          label="快捷键"
          divider={false}
          control={<HotkeyRecorder value={settings.hotkey} onChange={(h: Hotkey) => update({ hotkey: h })} />}
        />
      </SettingSection>

      <SettingSection icon="globe" title="语言" cardStyle={{ overflow: "visible" }}>
        {/* mock: interface language switch isn't backed */}
        <SettingRow
          label="界面语言"
          divider={false}
          control={
            <div className="w-[200px]">
              <Select defaultValue="zh">
                <option value="zh">简体中文</option>
                <option value="en">English</option>
              </Select>
            </div>
          }
        />
      </SettingSection>

      <SettingSection icon="mic" title="设备" cardStyle={{ overflow: "visible" }}>
        {/* mock: device enumeration isn't implemented (P3) */}
        <div className="p-3.5">
          <DevicePicker
            autoLabel="自动检测的麦克风（推荐）"
            devices={[
              { id: "b", label: "内置麦克风" },
              { id: "u", label: "USB 音频设备" },
            ]}
          />
        </div>
      </SettingSection>

      <SettingSection icon="settings" title="系统">
        {/* mock: launch-at-login / dock visibility aren't backed */}
        <SettingRow label="开机自动启动" divider={false} control={<MockSwitch defaultOn />} />
        <SettingRow
          label={
            <span className="inline-flex items-center gap-2">
              在 Dock 中显示
              <Badge tone="neutral">macOS</Badge>
            </span>
          }
          control={<MockSwitch defaultOn />}
        />
      </SettingSection>

      <SettingSection icon="shield" title="权限">
        {/* mock: real TCC status check is P3 */}
        <PermissionRow icon="mic" name="麦克风" status="granted" divider={false} />
        <PermissionRow icon="command" name="辅助功能" status="granted" />
      </SettingSection>

      <SettingSection icon="copy" title="输入">
        <SettingRow
          label="剪贴板粘贴"
          divider={false}
          control={
            <Badge tone="accent" dot>
              默认
            </Badge>
          }
        />
      </SettingSection>
    </>
  );
}
