// 通用 — keyboard · language · device · system · permissions · input. Only the
// hotkey is backed (maps to the preset enum); language/device/system toggles/
// permissions/input are mock for visual fidelity (see plan).

import { useState } from "react";

import type { AudioDevice, Hotkey, Settings } from "../../types/settings";
import { Badge, DevicePicker, Select, Switch } from "../ui";
import { SettingSection, SettingRow } from "./SettingSection";
import { HotkeyRecorder } from "./HotkeyRecorder";
import { PermissionRow } from "./PermissionRow";
import { useMicMonitor } from "../../hooks/useMicMonitor";
import { useInputMonitoring } from "../../hooks/useInputMonitoring";

// mock: a Switch holding its own demo state, for unbacked rows.
function MockSwitch({ defaultOn }: { defaultOn?: boolean }) {
  const [on, setOn] = useState(!!defaultOn);
  return <Switch checked={on} onChange={setOn} />;
}

type GeneralSectionProps = {
  settings: Settings;
  update: (patch: Partial<Settings>) => void;
  microphones: AudioDevice[];
  autoDevice: string | null;
};

export function GeneralSection({ settings, update, microphones, autoDevice }: GeneralSectionProps) {
  // Live preview of the selected mic — lets the user confirm it's picking up
  // sound (a silent meter on e.g. AirPods A2DP flags a dead mic before they rely
  // on it). Runs while this tab is open; recording stops it server-side.
  const micLevel = useMicMonitor(settings.input_device, true);
  const inputMonitoring = useInputMonitoring();

  // The "自动" row already names the device it resolves to, so hide that same mic
  // from the explicit list to avoid listing it twice — unless it happens to be
  // the current explicit pick (keep it so the selection stays visible).
  const explicitDevices = microphones.filter(
    (d) => d.id !== autoDevice || d.id === settings.input_device,
  );

  return (
    <>
      <SettingSection icon="command" title="触发键">
        <SettingRow
          label="触发键"
          description="按一下开始录音，再按一下结束。点右侧框可改键"
          divider={false}
          control={<HotkeyRecorder value={settings.hotkey} onChange={(h: Hotkey) => update({ hotkey: h })} />}
        />
        {settings.hotkey === "Fn" ? (
          <div className="px-3.5 pb-3 text-xs text-warning-text">
            提示：macOS 默认按 fn 会弹表情面板。到「系统设置 → 键盘 → 按下 🌐 键用来」改为「无操作」，fn 才会纯归 Audie。
          </div>
        ) : null}
        <SettingRow
          label="写作触发键"
          description="留空 = 不启用写作。按它说出要点，生成的文本插入光标处"
          control={
            <HotkeyRecorder
              value={settings.compose_hotkey}
              onChange={(h: Hotkey) => update({ compose_hotkey: h })}
            />
          }
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
        <div className="p-3.5">
          <DevicePicker
            autoLabel={autoDevice ? `自动检测 · ${autoDevice}` : "自动检测的麦克风（推荐）"}
            devices={explicitDevices}
            value={settings.input_device || "auto"}
            onChange={(id) => update({ input_device: id === "auto" ? "" : id })}
            level={micLevel}
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
        {/* mic / accessibility status still mock; input monitoring is real (P3.9) */}
        <PermissionRow icon="mic" name="麦克风" status="granted" divider={false} />
        <PermissionRow icon="command" name="辅助功能" status="granted" />
        <PermissionRow
          icon="monitor"
          name="输入监控"
          description="触发键（默认 fn）需要；授权后需重启 Audie 生效"
          status={inputMonitoring.granted ? "granted" : "pending"}
          onGrant={inputMonitoring.request}
          grantLabel="授权"
        />
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
