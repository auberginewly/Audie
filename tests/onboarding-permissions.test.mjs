import assert from "node:assert/strict";
import test from "node:test";

import {
  effectivePermissionGranted,
  permissionAfterStatus,
  permissionAfterTimeout,
  permissionsAreReady,
} from "../src/hooks/permissionState.ts";

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

test("onboarding permissions require all three effective grants", () => {
  assert.equal(permissionsAreReady({ microphone: true, accessibility: true, inputMonitoring: true }), true);
  assert.equal(permissionsAreReady({ microphone: false, accessibility: true, inputMonitoring: true }), false);
  assert.equal(permissionsAreReady({ microphone: true, accessibility: false, inputMonitoring: true }), false);
  assert.equal(permissionsAreReady({ microphone: true, accessibility: true, inputMonitoring: false }), false);
});
