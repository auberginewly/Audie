import { useEffect, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { listen } from "@tauri-apps/api/event";

import { useRecordingFlow } from "./hooks/useRecordingFlow";
import { useSettings } from "./hooks/useSettings";
import { usePermissions } from "./hooks/usePermissions";
import { useConfiguredModels } from "./hooks/useConfiguredModels";
import { useRecordingStore } from "./store/recording";
import { AppShell, AppSidebar } from "./components/shell";
import { Button } from "./components/ui";
import { HomeScreen } from "./components/screens/HomeScreen";
import { HistoryScreen } from "./components/screens/HistoryScreen";
import { SetupWizard } from "./components/screens/SetupWizard";
import { deriveOnboardingProgress, type OnboardingProgress } from "./components/screens/setup-wizard/progress";
import { SettingsDialog } from "./components/Settings";
import { I18nProvider, isLanguage, useI18n } from "./i18n";
import { getRuntimePlatform } from "./lib/runtimePlatform";

// Sidebar and wizard consume the same derived snapshot, so their checkmarks and
// x/5 value cannot diverge when a saved provider or keychain secret changes.
function SetupGuideCard({ progress, onContinue }: { progress: OnboardingProgress; onContinue: () => void }) {
  const { t } = useI18n();
  return (
    <div className="rounded-md bg-gray-100 p-3">
      <div className="mb-2 flex items-center justify-between">
        <span className="text-[13px] font-medium text-text-primary">{t("app.setup.progressTitle")}</span>
        <span className="font-mono text-[11px] text-text-tertiary">
          {progress.done}/{progress.total}
        </span>
      </div>
      <div className="mb-2.5 h-1 overflow-hidden rounded-full bg-gray-300">
        <div
          className="h-full rounded-full bg-accent-fill"
          style={{ width: `${(progress.done / progress.total) * 100}%` }}
        />
      </div>
      <Button size="sm" variant="accent" block iconRight="chevron-right" onClick={onContinue}>
        {t("app.setup.continue")}
      </Button>
    </div>
  );
}

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
  const [setupOpen, setSetupOpen] = useState(false);

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

  // First-run onboarding is persisted in settings (P3.12). Auto-open the wizard once
  // when a fresh install reports it incomplete; the ref stops it reopening if the
  // user closes it without finishing (it auto-opens again next launch, like Voxt).
  const onboardingCompleted = data.settings?.onboarding_completed;
  const language = data.settings && isLanguage(data.settings.ui_language) ? data.settings.ui_language : "zh-Hans";
  const onboardingProgress = deriveOnboardingProgress(
    data.settings,
    {
      microphone: permissions.microphone.granted,
      accessibility: permissions.accessibility.granted,
      inputMonitoring: permissions.inputMonitoring.granted,
    },
    platform,
    configuredModels.configured,
  );
  const autoOpened = useRef(false);
  useEffect(() => {
    if (onboardingCompleted === false && !autoOpened.current) {
      autoOpened.current = true;
      setSetupOpen(true);
    }
  }, [onboardingCompleted]);

  // SUCCESS is emitted only after the real dictation path completes. Persist it
  // through the existing Settings store so reopening/restarting keeps "Try it" done.
  useEffect(() => {
    if (recordingState === "SUCCESS" && data.settings && !data.settings.onboarding_test_completed) {
      void data.update({ onboarding_test_completed: true });
    }
  }, [data, recordingState]);

  // "重新运行配置向导" (About page): mark onboarding incomplete and reopen the
  // wizard, closing Settings so it's visible — saves the user editing the store.
  const rerunSetup = () => {
    void data.update({ onboarding_completed: false });
    setSettingsOpen(false);
    setSetupOpen(true);
  };

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
      .catch((err) => {
        console.error("failed to subscribe open-settings:", err);
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
          {nav === "home" ? <HomeScreen /> : <HistoryScreen data={data} />}
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
