import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { useRecordingFlow } from "./hooks/useRecordingFlow";
import { useSettings } from "./hooks/useSettings";
import { AppShell, AppSidebar, UpdateButton, type UpdateLabels, type UpdateState } from "./components/shell";
import { Button, Dialog } from "./components/ui";
import { HomeScreen } from "./components/screens/HomeScreen";
import { HistoryScreen } from "./components/screens/HistoryScreen";
import { SetupWizard } from "./components/screens/SetupWizard";
import { SettingsDialog } from "./components/Settings";

const UPDATE_LABELS: UpdateLabels = {
  check: "检查更新",
  checking: "检查中…",
  upToDate: "已是最新",
  update: "更新",
  downloading: "下载中…",
};
// mock: no real update channel yet — demo flow only (see plan). Starts on the
// design's "available" state so the titlebar pill matches the mockup.
const AVAILABLE_VERSION = "0.5.0";

// Sidebar dock card nudging first-run setup — progress + CTA into the wizard.
function SetupGuideCard({ done, total, onContinue }: { done: number; total: number; onContinue: () => void }) {
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
  // mock: no first-run persistence — the guide card shows until the wizard
  // finishes this session (see plan).
  const [setupDone, setSetupDone] = useState(false);
  const [updateState, setUpdateState] = useState<UpdateState>("available");
  const [updateOpen, setUpdateOpen] = useState(false);

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

  const handleUpdate = useCallback(() => {
    if (updateState === "available") {
      setUpdateOpen(true);
    } else if (updateState === "idle" || updateState === "up-to-date") {
      setUpdateState("checking");
      setTimeout(() => setUpdateState("available"), 1100);
    }
  }, [updateState]);

  const confirmUpdate = useCallback(() => {
    setUpdateOpen(false);
    setUpdateState("downloading");
    setTimeout(() => setUpdateState("up-to-date"), 1800);
  }, []);

  return (
    <div className="relative h-screen w-screen overflow-hidden bg-surface-app">
      {/* Overlay titlebar: drag the window by the top strip; macOS paints the
       * traffic lights over its top-left. Sidebar content clears it (pt-9). */}
      <div data-tauri-drag-region className="absolute inset-x-0 top-0 z-20 h-7" />
      {/* Update affordance sits just right of the native traffic lights. */}
      <div className="absolute left-[84px] top-[5px] z-30">
        <UpdateButton
          compact
          state={updateState}
          availableVersion={AVAILABLE_VERSION}
          labels={UPDATE_LABELS}
          onClick={handleUpdate}
        />
      </div>

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
              setupDone ? undefined : <SetupGuideCard done={3} total={4} onContinue={() => setSetupOpen(true)} />
            }
          />
        }
      >
        {nav === "home" ? <HomeScreen /> : <HistoryScreen />}
      </AppShell>

      <SettingsDialog open={settingsOpen} onClose={() => setSettingsOpen(false)} data={data} />

      <SetupWizard
        open={setupOpen}
        onClose={() => setSetupOpen(false)}
        onComplete={() => {
          setSetupDone(true);
          setSetupOpen(false);
        }}
        data={data}
      />

      <Dialog
        open={updateOpen}
        onClose={() => setUpdateOpen(false)}
        icon="download"
        title="发现新版本"
        actions={
          <>
            <Button variant="ghost" onClick={() => setUpdateOpen(false)}>
              稍后
            </Button>
            <Button variant="accent" onClick={confirmUpdate}>
              立即更新
            </Button>
          </>
        }
      >
        <div className="font-medium text-text-primary">Audie {AVAILABLE_VERSION} 已准备好安装。</div>
        <div className="mt-1.5">本次更新包含改进与修复。是否现在更新？</div>
      </Dialog>
    </div>
  );
}

export default App;
