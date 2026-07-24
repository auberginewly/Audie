import assert from "node:assert/strict";
import test from "node:test";

import {
  effectivePermissionGranted,
  permissionAfterStatus,
  permissionAfterTimeout,
  permissionsAreReady,
} from "../src/hooks/permissionState.ts";
import { runtimePlatformFromUserAgent } from "../src/lib/runtimePlatform.ts";

test("a permission request stays incomplete until macOS reports it granted", () => {
  const requesting = { granted: false, phase: "requesting" };

  assert.equal(effectivePermissionGranted(requesting), false);
  assert.deepEqual(permissionAfterStatus("microphone", requesting, true), {
    granted: true,
    phase: "idle",
  });
});

test("a newly granted Input Monitoring permission requires a restart", () => {
  const requesting = { granted: false, phase: "requesting" };
  const grantedByMacOS = permissionAfterStatus("inputMonitoring", requesting, true);

  assert.deepEqual(grantedByMacOS, { granted: true, phase: "needsRestart" });
  assert.equal(effectivePermissionGranted(grantedByMacOS), false);
  assert.deepEqual(permissionAfterStatus("inputMonitoring", { granted: false, phase: "needsSettings" }, true), {
    granted: true,
    phase: "needsRestart",
  });
});

test("an unanswered or denied request falls back to System Settings", () => {
  assert.deepEqual(permissionAfterTimeout({ granted: false, phase: "requesting" }), {
    granted: false,
    phase: "needsSettings",
  });
});

test("macOS onboarding permissions require all three effective grants", () => {
  assert.equal(permissionsAreReady({ microphone: true, accessibility: true, inputMonitoring: true }, "macos"), true);
  assert.equal(permissionsAreReady({ microphone: false, accessibility: true, inputMonitoring: true }, "macos"), false);
  assert.equal(permissionsAreReady({ microphone: true, accessibility: false, inputMonitoring: true }, "macos"), false);
  assert.equal(permissionsAreReady({ microphone: true, accessibility: true, inputMonitoring: false }, "macos"), false);
});

test("Windows onboarding only requires the microphone permission", () => {
  assert.equal(
    permissionsAreReady({ microphone: true, accessibility: false, inputMonitoring: false }, "windows"),
    true,
  );
  assert.equal(
    permissionsAreReady({ microphone: false, accessibility: true, inputMonitoring: true }, "windows"),
    false,
  );
});

test("runtime platform detection distinguishes Windows and macOS webviews", () => {
  assert.equal(runtimePlatformFromUserAgent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)"), "windows");
  assert.equal(runtimePlatformFromUserAgent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)"), "macos");
});
