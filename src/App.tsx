import { useRecordingFlow } from "./hooks/useRecordingFlow";
import { useRecordingStore } from "./store/recording";
import { HotkeySettings, ProviderSettings } from "./components/Settings";
import { DesignSystemPreview } from "./designsystem/DesignSystemPreview";

// Dev-only gallery for the design-system foundation. Reach it at `?preview`
// during `tauri dev`; the production main window is untouched.
const showPreview =
  import.meta.env.DEV && new URLSearchParams(window.location.search).has("preview");

function App() {
  if (showPreview) return <DesignSystemPreview />;
  return <MainWindow />;
}

function MainWindow() {
  useRecordingFlow();
  const state = useRecordingStore((s) => s.state);

  return (
    <main className="min-h-screen flex items-center justify-center bg-base-100 text-base-content">
      <div className="text-center space-y-4">
        <h1 className="text-2xl font-semibold">Audie</h1>
        <p className="opacity-60 text-sm">Hold your hotkey to summon the capsule.</p>
        <div className="badge badge-outline mt-2">state: {state}</div>
        <div className="pt-3">
          <HotkeySettings />
        </div>
        <ProviderSettings />
      </div>
    </main>
  );
}

export default App;
