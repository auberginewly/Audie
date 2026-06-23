import { useState } from "react";

import { useRecordingFlow } from "./hooks/useRecordingFlow";
import { useSettings } from "./hooks/useSettings";
import { AppShell, AppSidebar } from "./components/shell";
import { HomeScreen } from "./components/screens/HomeScreen";
import { HistoryScreen } from "./components/screens/HistoryScreen";
import { SettingsDialog } from "./components/Settings";

function App() {
  useRecordingFlow();
  const data = useSettings();
  const [nav, setNav] = useState("home");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const hotkey = data.settings?.hotkey ?? "Ctrl+Shift+Space";

  return (
    <div className="relative h-screen w-screen overflow-hidden bg-surface-app">
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
        {nav === "home" ? <HomeScreen hotkey={hotkey} /> : <HistoryScreen />}
      </AppShell>
      <SettingsDialog open={settingsOpen} onClose={() => setSettingsOpen(false)} data={data} />
    </div>
  );
}

export default App;
