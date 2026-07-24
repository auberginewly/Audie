import assert from "node:assert/strict";
import test from "node:test";

import { captureWindowsHotkey } from "../src/lib/windowsHotkeyCapture.ts";

function keyInput(key, overrides = {}) {
  return {
    key,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    ...overrides,
  };
}

test("captures the Windows default shortcut in canonical order", () => {
  assert.deepEqual(captureWindowsHotkey(keyInput(" ", { ctrlKey: true, shiftKey: true })), {
    kind: "captured",
    hotkey: "Ctrl+Shift+Space",
  });
});

test("captures supported letters, digits, function keys, and navigation keys", () => {
  assert.deepEqual(captureWindowsHotkey(keyInput("k", { ctrlKey: true, altKey: true })), {
    kind: "captured",
    hotkey: "Ctrl+Alt+K",
  });
  assert.deepEqual(captureWindowsHotkey(keyInput("F13")), { kind: "captured", hotkey: "F13" });
  assert.deepEqual(captureWindowsHotkey(keyInput("ArrowUp", { shiftKey: true })), {
    kind: "captured",
    hotkey: "Shift+Up",
  });
});

test("waits for a main key and ignores repeated keydown events", () => {
  assert.deepEqual(captureWindowsHotkey(keyInput("Control", { ctrlKey: true })), { kind: "ignore" });
  assert.deepEqual(captureWindowsHotkey(keyInput("K", { ctrlKey: true, repeat: true })), { kind: "ignore" });
});

test("rejects Windows-key, Caps Lock, unsupported, and reserved combinations", () => {
  assert.deepEqual(captureWindowsHotkey(keyInput("k", { metaKey: true })), { kind: "rejected" });
  assert.deepEqual(captureWindowsHotkey(keyInput("CapsLock")), { kind: "rejected" });
  assert.deepEqual(captureWindowsHotkey(keyInput("AudioVolumeUp")), { kind: "rejected" });
  assert.deepEqual(captureWindowsHotkey(keyInput("F4", { altKey: true })), { kind: "rejected" });
});
