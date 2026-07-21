export type PermissionKey = "microphone" | "accessibility" | "inputMonitoring";
export type PermissionPhase = "idle" | "requesting" | "needsSettings" | "needsRestart";

export interface PermissionSnapshot {
  readonly granted: boolean;
  readonly phase: PermissionPhase;
}

export interface PermissionGrantStatus {
  readonly microphone: boolean | null;
  readonly accessibility: boolean | null;
  readonly inputMonitoring: boolean | null;
}

export function permissionsAreReady(permissions: PermissionGrantStatus): boolean {
  return permissions.microphone === true && permissions.accessibility === true && permissions.inputMonitoring === true;
}

export function permissionAfterStatus(
  key: PermissionKey,
  current: PermissionSnapshot,
  granted: boolean,
): PermissionSnapshot {
  if (!granted) return { ...current, granted: false };
  if (current.phase === "needsRestart") return current;
  if (key === "inputMonitoring" && (current.phase === "requesting" || current.phase === "needsSettings")) {
    return { granted: true, phase: "needsRestart" };
  }
  return { granted: true, phase: "idle" };
}

export function permissionAfterTimeout(current: PermissionSnapshot): PermissionSnapshot {
  if (current.granted) return current;
  return { granted: false, phase: "needsSettings" };
}

export function effectivePermissionGranted(snapshot: PermissionSnapshot): boolean {
  return snapshot.granted && snapshot.phase !== "needsRestart";
}
