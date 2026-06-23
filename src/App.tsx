import { useCallback, useState } from "react";

import { useRecordingFlow } from "./hooks/useRecordingFlow";
import { useSettings } from "./hooks/useSettings";
import { AppShell, AppSidebar, UpdateButton, type UpdateLabels, type UpdateState } from "./components/shell";
import { Button, Dialog } from "./components/ui";
import { HomeScreen } from "./components/screens/HomeScreen";
import { HistoryScreen } from "./components/screens/HistoryScreen";
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

function App() {
  useRecordingFlow();
  const data = useSettings();
  const [nav, setNav] = useState("home");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [updateState, setUpdateState] = useState<UpdateState>("available");
  const [updateOpen, setUpdateOpen] = useState(false);

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
        sidebar={
          <AppSidebar
            active={nav}
            onNavigate={setNav}
            version="0.0.0"
            settingsLabel="设置"
            settingsActive={settingsOpen}
            onSettings={() => setSettingsOpen(true)}
          />
        }
      >
        {nav === "home" ? <HomeScreen /> : <HistoryScreen />}
      </AppShell>

      <SettingsDialog open={settingsOpen} onClose={() => setSettingsOpen(false)} data={data} />

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
