export type RuntimePlatform = "macos" | "windows" | "other";

export function runtimePlatformFromUserAgent(userAgent: string): RuntimePlatform {
  if (userAgent.includes("Windows")) return "windows";
  if (userAgent.includes("Macintosh") || userAgent.includes("Mac OS X")) return "macos";
  return "other";
}

export function getRuntimePlatform(): RuntimePlatform {
  return runtimePlatformFromUserAgent(window.navigator.userAgent);
}
