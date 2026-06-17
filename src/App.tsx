import { useRecordingFlow } from "./hooks/useRecordingFlow";
import { useRecordingStore } from "./store/recording";
import { HotkeySettings } from "./components/Settings";

function App() {
  useRecordingFlow();
  const state = useRecordingStore((s) => s.state);

  return (
    <main className="min-h-screen flex items-center justify-center bg-base-100 text-base-content">
      <div className="text-center space-y-3">
        <h1 className="text-2xl font-semibold">Audie</h1>
        <p className="opacity-60 text-sm">Hold your hotkey to summon the capsule.</p>
        <div className="badge badge-outline mt-2">state: {state}</div>
        <div className="pt-4">
          <HotkeySettings />
        </div>
      </div>
    </main>
  );
}

export default App;
