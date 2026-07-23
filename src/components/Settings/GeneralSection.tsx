import { useEffect, useState } from "react";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";

import type { AudioDevice, Hotkey, Settings } from "../../types/settings";
import { Badge, DevicePicker, Select, Switch } from "../ui";
import { SettingSection, SettingRow } from "./SettingSection";
import { HotkeyRecorder } from "./HotkeyRecorder";
import { PermissionRow } from "./PermissionRow";
import { useMicMonitor } from "../../hooks/useMicMonitor";
import { LANGUAGES, useI18n, type Language } from "../../i18n";
import { getRuntimePlatform } from "../../lib/runtimePlatform";

interface GeneralSectionProps {
  settings: Settings;
  update: (patch: Partial<Settings>) => void;
  microphones: AudioDevice[];
  autoDevice: string | null;
}

export function GeneralSection({ settings, update, microphones, autoDevice }: GeneralSectionProps) {
  const { t } = useI18n();
  const platform = getRuntimePlatform();
  const [launchAtLogin, setLaunchAtLogin] = useState(false);
  const [launchAtLoginBusy, setLaunchAtLoginBusy] = useState(true);
  // Live preview of the selected mic — lets the user confirm it's picking up
  // sound (a silent meter on e.g. AirPods A2DP flags a dead mic before they rely
  // on it). Runs while this tab is open; recording stops it server-side.
  const micLevel = useMicMonitor(settings.input_device, true);

  // The "自动" row already names the device it resolves to, so hide that same mic
  // from the explicit list to avoid listing it twice — unless it happens to be
  // the current explicit pick (keep it so the selection stays visible).
  const explicitDevices = microphones.filter((d) => d.id !== autoDevice || d.id === settings.input_device);

  useEffect(() => {
    let alive = true;
    isEnabled()
      .then((enabled) => {
        if (alive) setLaunchAtLogin(enabled);
      })
      .catch((err) => {
        console.error("load launch-at-login failed:", err);
      })
      .finally(() => {
        if (alive) setLaunchAtLoginBusy(false);
      });
    return () => {
      alive = false;
    };
  }, []);

  const changeLaunchAtLogin = (next: boolean) => {
    const previous = launchAtLogin;
    setLaunchAtLogin(next);
    setLaunchAtLoginBusy(true);
    const task = next ? enable() : disable();
    task
      .then(() => isEnabled())
      .then((enabled) => {
        setLaunchAtLogin(enabled);
      })
      .catch((err) => {
        console.error("update launch-at-login failed:", err);
        setLaunchAtLogin(previous);
      })
      .finally(() => {
        setLaunchAtLoginBusy(false);
      });
  };

  return (
    <>
      <SettingSection icon="command" title={t("settings.general.hotkeys")}>
        <SettingRow
          label={t("settings.general.primaryHotkey")}
          description={t("settings.general.primaryHotkeyDesc")}
          divider={false}
          control={
            <HotkeyRecorder
              value={settings.hotkey}
              onChange={(h: Hotkey) => {
                update({ hotkey: h });
              }}
              conflictWith={settings.compose_hotkey}
            />
          }
        />
        <SettingRow
          label={t("settings.general.composeHotkey")}
          description={t("settings.general.composeHotkeyDesc")}
          control={
            <HotkeyRecorder
              value={settings.compose_hotkey}
              onChange={(h: Hotkey) => {
                update({ compose_hotkey: h });
              }}
              conflictWith={settings.hotkey}
            />
          }
        />
        {platform === "macos" && settings.hotkey === "Fn" ? (
          <div className="px-3.5 pb-3 text-xs text-warning-text">{t("settings.general.fnTip")}</div>
        ) : null}
      </SettingSection>

      <SettingSection icon="globe" title={t("settings.general.language")} cardStyle={{ overflow: "visible" }}>
        <SettingRow
          label={t("settings.general.uiLanguage")}
          divider={false}
          control={
            <div className="w-[200px]">
              <Select
                value={settings.ui_language}
                onChange={(e) => {
                  update({ ui_language: e.target.value as Language });
                }}
              >
                {LANGUAGES.map((language) => (
                  <option key={language} value={language}>
                    {language === "zh-Hans"
                      ? t("settings.general.language.zhHans")
                      : language === "zh-Hant"
                        ? t("settings.general.language.zhHant")
                        : t("settings.general.language.en")}
                  </option>
                ))}
              </Select>
            </div>
          }
        />
      </SettingSection>

      <SettingSection icon="mic" title={t("settings.general.devices")} cardStyle={{ overflow: "visible" }}>
        <div className="p-3.5">
          <DevicePicker
            autoLabel={
              autoDevice
                ? t("settings.general.autoDevice", { device: autoDevice })
                : t("settings.general.autoDeviceFallback")
            }
            devices={explicitDevices}
            value={settings.input_device || "auto"}
            onChange={(id) => {
              update({ input_device: id === "auto" ? "" : id });
            }}
            level={micLevel}
          />
        </div>
      </SettingSection>

      <SettingSection icon="settings" title={t("settings.general.system")}>
        <SettingRow
          label={t("settings.general.launchAtLogin")}
          divider={false}
          control={<Switch checked={launchAtLogin} disabled={launchAtLoginBusy} onChange={changeLaunchAtLogin} />}
        />
        <SettingRow
          label={
            <span className="inline-flex items-center gap-2">
              {t("settings.general.showInDock")}
              <Badge tone="neutral">macOS</Badge>
            </span>
          }
          control={
            <Switch
              checked={settings.show_in_dock}
              onChange={(showInDock) => {
                update({ show_in_dock: showInDock });
              }}
            />
          }
        />
      </SettingSection>

      <SettingSection icon="shield" title={t("settings.general.permissions")}>
        {/* mic / accessibility status still mock; input monitoring is real (P3.9) */}
        <PermissionRow icon="mic" name={t("settings.general.microphone")} status="granted" divider={false} />
        <PermissionRow icon="command" name={t("settings.general.accessibility")} status="granted" />
      </SettingSection>
    </>
  );
}
