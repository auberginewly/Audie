import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

const scriptPath = resolve("scripts/check-release-version.mjs");

function createFixture({
  packageVersion = "0.1.0",
  cargoVersion = "0.1.0",
  tauriVersion = "0.1.0",
  notesVersion = "0.1.0",
  notesLanguages = "## English\n\nNotes.\n\n## 中文\n\n说明。",
} = {}) {
  const root = mkdtempSync(join(tmpdir(), "audie-release-version-"));
  mkdirSync(join(root, "src-tauri"));
  writeFileSync(join(root, "package.json"), JSON.stringify({ version: packageVersion }));
  writeFileSync(join(root, "src-tauri", "Cargo.toml"), `[package]\nname = "audie"\nversion = "${cargoVersion}"\n`);
  writeFileSync(
    join(root, "src-tauri", "tauri.conf.json"),
    JSON.stringify({ productName: "Audie", version: tauriVersion }),
  );
  writeFileSync(join(root, "RELEASE_NOTES.md"), `# Audie ${notesVersion} Preview\n\n${notesLanguages}\n`);
  execFileSync("git", ["init", "--quiet"], { cwd: root });
  return root;
}

function runCheck(root, ...args) {
  return spawnSync(process.execPath, [scriptPath, ...args], {
    cwd: root,
    encoding: "utf8",
  });
}

test("prints the shared stable release version", () => {
  const result = runCheck(createFixture());

  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout, "0.1.0\n");
});

test("rejects mismatched version sources", () => {
  const result = runCheck(createFixture({ cargoVersion: "0.1.1" }));

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /version sources must match/i);
});

test("rejects a non-stable semantic version", () => {
  const result = runCheck(
    createFixture({
      packageVersion: "0.1.0-beta.1",
      cargoVersion: "0.1.0-beta.1",
      tauriVersion: "0.1.0-beta.1",
      notesVersion: "0.1.0-beta.1",
    }),
  );

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /stable semantic version/i);
});

test("rejects release notes for another version", () => {
  const result = runCheck(createFixture({ notesVersion: "0.2.0" }));

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /release notes title/i);
});

test("rejects release notes with Chinese before English", () => {
  const result = runCheck(createFixture({ notesLanguages: "## 中文\n\n说明。\n\n## English\n\nNotes." }));

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /English.*before.*中文/i);
});

test("rejects an existing tag when an unreleased version is required", () => {
  const root = createFixture();
  execFileSync("git", ["config", "user.name", "Audie Tests"], { cwd: root });
  execFileSync("git", ["config", "user.email", "tests@audie.local"], { cwd: root });
  execFileSync("git", ["add", "."], { cwd: root });
  execFileSync("git", ["commit", "--quiet", "-m", "fixture"], { cwd: root });
  execFileSync("git", ["tag", "v0.1.0"], { cwd: root });

  const result = runCheck(root, "--require-unreleased");

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /tag v0\.1\.0 already exists/i);
});
