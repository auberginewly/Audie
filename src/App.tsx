import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { listen } from "@tauri-apps/api/event";

import { useRecordingFlow } from "./features/dictation/useRecordingFlow";
import { useSettings } from "./features/settings/useSettings";
import { usePermissions } from "./features/permissions/usePermissions";
import { useConfiguredModels } from "./features/providers/useConfiguredModels";
import { useRecordingStore } from "./features/dictation/recording";
import { AppShell, AppSidebar } from "./components/shell";
import { SetupGuideCard } from "./features/onboarding/SetupGuideCard";
import { HomeScreen } from "./features/home/HomeScreen";
import { HistoryScreen } from "./features/history/HistoryScreen";
import { SetupWizard } from "./features/onboarding/SetupWizard";
import { useOnboarding } from "./features/onboarding/useOnboarding";
import { SettingsDialog } from "./features/settings";
import { I18nProvider, isLanguage } from "./i18n";
import { getRuntimePlatform } from "./lib/runtimePlatform";

function App() {
  const platform = getRuntimePlatform();
  const data = useSettings();
  const permissions = usePermissions();
  const configuredModels = useConfiguredModels();
  const recordingState = useRecordingStore((s) => s.state);
  useRecordingFlow();
  const [nav, setNav] = useState("home");
  const [appVersion, setAppVersion] = useState("—");
  const [settingsOpen, setSettingsOpen] = useState(false);

  useEffect(() => {
    let mounted = true;

    void getVersion()
      .then((version) => {
        if (mounted) setAppVersion(version);
      })
      .catch(() => {
        if (mounted) setAppVersion("—");
      });

    return () => {
      mounted = false;
    };
  }, []);

  const { setupOpen, setSetupOpen, onboardingCompleted, onboardingProgress, rerunSetup } = useOnboarding(
    data,
    permissions,
    configuredModels,
    platform,
    recordingState,
    () => {
      setSettingsOpen(false);
    },
  );
  const language = data.settings && isLanguage(data.settings.ui_language) ? data.settings.ui_language : "zh-Hans";

  // The overlay's 去设置 (polish-unavailable toast) shows + focuses this window
  // via the backend, then fires `open-settings` so we surface the Settings dialog.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listen("open-settings", () => {
      setSettingsOpen(true);
    })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch(() => {
        console.error("failed to subscribe open-settings:");
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return (
    <I18nProvider language={language}>
      <div className="relative h-screen w-screen overflow-hidden bg-surface-app">
        {/* Overlay titlebar: drag the window by the top strip; macOS paints the
         * traffic lights over its top-left. Sidebar content clears it (pt-9). */}
        <div data-tauri-drag-region className="absolute inset-x-0 top-0 z-20 h-7" />

        <AppShell
          bleed
          sidebar={
            <AppSidebar
              active={nav}
              onNavigate={setNav}
              version={appVersion}
              settingsActive={settingsOpen}
              onSettings={() => {
                setSettingsOpen(true);
              }}
              aboveDock={
                onboardingCompleted === false ? (
                  <SetupGuideCard
                    progress={onboardingProgress}
                    onContinue={() => {
                      setSetupOpen(true);
                    }}
                  />
                ) : undefined
              }
            />
          }
        >
          {nav === "home" ? (
            <HomeScreen hotkey={data.settings?.hotkey} platform={platform} />
          ) : (
            <HistoryScreen data={data} />
          )}
        </AppShell>

        <SettingsDialog
          open={settingsOpen}
          onClose={() => {
            setSettingsOpen(false);
          }}
          data={data}
          onRerunSetup={rerunSetup}
        />

        <SetupWizard
          open={setupOpen}
          onClose={() => {
            setSetupOpen(false);
          }}
          onComplete={() => {
            void data.update({ onboarding_completed: true });
            setSetupOpen(false);
          }}
          data={data}
          permissions={permissions}
          progress={onboardingProgress}
          platform={platform}
          configured={configuredModels.configured}
          onRefreshModels={configuredModels.refresh}
        />
      </div>
    </I18nProvider>
  );
}

export default App;
