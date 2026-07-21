import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

test("sidebar version comes from packaged application metadata", () => {
  const appSource = readFileSync("src/App.tsx", "utf8");
  const sidebarSource = readFileSync("src/components/shell/Sidebar.tsx", "utf8");

  assert.match(appSource, /getVersion\(\)/);
  assert.doesNotMatch(`${appSource}\n${sidebarSource}`, /0\.0\.0/);
});
