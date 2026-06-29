import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { useRecordingFlow } from "./hooks/useRecordingFlow";
import { useSettings } from "./hooks/useSettings";
import { usePermissions } from "./hooks/usePermissions";
import { useConfiguredModels } from "./hooks/useConfiguredModels";
import { useRecordingStore } from "./store/recording";
import { modelIdForAsrProvider } from "./components/Settings/models";
import type { Settings } from "./types/settings";
import { AppShell, AppSidebar } from "./components/shell";
import { Button } from "./components/ui";
import { HomeScreen } from "./components/screens/HomeScreen";
import { HistoryScreen } from "./components/screens/HistoryScreen";
import { SetupWizard } from "./components/screens/SetupWizard";
import { SettingsDialog } from "./components/Settings";

// Sidebar dock card nudging first-run setup — real progress + CTA into the wizard.
// Rendered only while onboarding is incomplete, so the permission/secret polls run
// only then (completed users pay nothing). x/n mirrors the wizard's per-step
// checkmarks (one unit per step): 权限 / 快捷键 / 听写 / 润色 / 试一下.
function SetupGuideCard({ settings, onContinue }: { settings: Settings | null; onContinue: () => void }) {
  const perms = usePermissions();
  const { configured } = useConfiguredModels();
  const everSucceeded = useRecordingStore((s) => s.everSucceeded);
  const steps = [
    perms.microphone.granted === true &&
      perms.accessibility.granted === true &&
      perms.inputMonitoring.granted === true,
    !!settings?.hotkey,
    !!settings && configured(modelIdForAsrProvider(settings.asr_provider)),
    configured("deepseek"), // 润色: the openai_compatible LLM slot's key
    everSucceeded, // 试一下: a dictation has succeeded this session
  ];
  const total = steps.length;
  const done = steps.filter(Boolean).length;
  return (
    <div className="rounded-md bg-gray-100 p-3">
      <div className="mb-2 flex items-center justify-between">
        <span className="text-[13px] font-medium text-text-primary">完成配置</span>
        <span className="font-mono text-[11px] text-text-tertiary">
          {done}/{total}
        </span>
      </div>
      <div className="mb-2.5 h-1 overflow-hidden rounded-full bg-gray-300">
        <div className="h-full rounded-full bg-accent-fill" style={{ width: `${(done / total) * 100}%` }} />
      </div>
      <Button size="sm" variant="accent" block iconRight="chevron-right" onClick={onContinue}>
        继续配置
      </Button>
    </div>
  );
}

function App() {
  useRecordingFlow();
  const data = useSettings();
  const [nav, setNav] = useState("home");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [setupOpen, setSetupOpen] = useState(false);

  // First-run onboarding is persisted in settings (P3.12). Auto-open the wizard once
  // when a fresh install reports it incomplete; the ref stops it reopening if the
  // user closes it without finishing (it auto-opens again next launch, like Voxt).
  const onboardingCompleted = data.settings?.onboarding_completed;
  const autoOpened = useRef(false);
  useEffect(() => {
    if (onboardingCompleted === false && !autoOpened.current) {
      autoOpened.current = true;
      setSetupOpen(true);
    }
  }, [onboardingCompleted]);

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
    listen("open-settings", () => setSettingsOpen(true))
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch((err) => console.error("failed to subscribe open-settings:", err));
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return (
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
            version="0.0.0"
            settingsLabel="设置"
            settingsActive={settingsOpen}
            onSettings={() => setSettingsOpen(true)}
            aboveDock={
              onboardingCompleted === false ? (
                <SetupGuideCard settings={data.settings} onContinue={() => setSetupOpen(true)} />
              ) : undefined
            }
          />
        }
      >
        {nav === "home" ? <HomeScreen /> : <HistoryScreen data={data} />}
      </AppShell>

      <SettingsDialog open={settingsOpen} onClose={() => setSettingsOpen(false)} data={data} onRerunSetup={rerunSetup} />

      <SetupWizard
        open={setupOpen}
        onClose={() => setSetupOpen(false)}
        onComplete={() => {
          void data.update({ onboarding_completed: true });
          setSetupOpen(false);
        }}
        data={data}
      />
    </div>
  );
}

export default App;
