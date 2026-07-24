export type WindowsHotkeyCaptureResult =
  { kind: "ignore" } | { kind: "captured"; hotkey: string } | { kind: "rejected" };

export interface WindowsHotkeyKeyboardInput {
  key: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
  repeat?: boolean;
}

const MODIFIER_KEYS = new Set(["Control", "Alt", "Shift", "Meta"]);
const NAMED_KEYS: Record<string, string> = {
  " ": "Space",
  Spacebar: "Space",
  Enter: "Enter",
  Tab: "Tab",
  Escape: "Escape",
  Esc: "Escape",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  ArrowUp: "Up",
  ArrowDown: "Down",
};

function mainKeyLabel(key: string): string | null {
  const named = NAMED_KEYS[key];
  if (named) return named;
  if (/^F(?:[1-9]|1\d|20)$/i.test(key)) return key.toUpperCase();
  if (/^[a-z0-9]$/i.test(key)) return key.toUpperCase();
  return null;
}

export function captureWindowsHotkey(input: WindowsHotkeyKeyboardInput): WindowsHotkeyCaptureResult {
  if (input.repeat || MODIFIER_KEYS.has(input.key)) return { kind: "ignore" };
  if (input.metaKey || input.key === "CapsLock") return { kind: "rejected" };

  const main = mainKeyLabel(input.key);
  if (!main) return { kind: "rejected" };

  // Keep Windows-reserved navigation/close combinations out of the saved config.
  if (input.altKey && (main === "F4" || main === "Tab")) return { kind: "rejected" };

  const keys: string[] = [];
  if (input.ctrlKey) keys.push("Ctrl");
  if (input.altKey) keys.push("Alt");
  if (input.shiftKey) keys.push("Shift");
  keys.push(main);
  return { kind: "captured", hotkey: keys.join("+") };
}
