import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

const installer = resolve("scripts/install-hooks.mjs");

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "audie-hooks-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  execFileSync("git", ["init", "--quiet"], { cwd: root });
  mkdirSync(join(root, ".githooks"));
  writeFileSync(join(root, ".githooks/pre-commit"), "#!/bin/sh\nexit 0\n");
  chmodSync(join(root, ".githooks/pre-commit"), 0o644);
  return root;
}

function install(root, env = process.env) {
  return spawnSync(process.execPath, [installer], { cwd: root, encoding: "utf8", env });
}

function hooksPath(root) {
  return spawnSync("git", ["config", "--local", "--get", "core.hooksPath"], { cwd: root, encoding: "utf8" });
}

test("installs per clone, remains idempotent and makes tracked hook executable", (t) => {
  const root = fixture(t);
  assert.equal(install(root).status, 0);
  assert.equal(hooksPath(root).stdout.trim(), ".githooks");
  assert.equal(install(root).status, 0);
  if (process.platform !== "win32") {
    assert.equal(spawnSync(join(root, ".githooks/pre-commit")).status, 0);
  }
});

test("refuses an existing custom hooksPath without modifying it", (t) => {
  const root = fixture(t);
  execFileSync("git", ["config", "--local", "core.hooksPath", "custom-hooks"], { cwd: root });
  const result = install(root);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /core.hooksPath/);
  assert.equal(hooksPath(root).stdout.trim(), "custom-hooks");
});

test("refuses an existing default pre-commit and preserves its content", (t) => {
  const root = fixture(t);
  const hook = join(root, ".git/hooks/pre-commit");
  writeFileSync(hook, "custom user hook\n");
  const result = install(root);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /pre-commit/);
  assert.equal(readFileSync(hook, "utf8"), "custom user hook\n");
  assert.notEqual(hooksPath(root).status, 0);
});

test("pre-commit preserves a failing child exit code and stops subsequent checks", async () => {
  const { runChecks } = await import("../scripts/pre-commit.mjs");
  const calls = [];
  const status = runChecks(process.cwd(), (command, args, options) => {
    calls.push([command, args]);
    return spawnSync(process.execPath, ["-e", `process.exit(${calls.length === 2 ? 37 : 0})`], {
      ...options,
      shell: false,
    });
  });
  assert.equal(status, 37);
  assert.equal(calls.length, 2);
  assert.deepEqual(
    calls.map(([, args]) => args),
    [["check:engineering"], ["typecheck"]],
  );
});

test("pre-commit runs frontend static checks and Rust fmt/clippy on success", async () => {
  const { runChecks } = await import("../scripts/pre-commit.mjs");
  const calls = [];
  assert.equal(
    runChecks(process.cwd(), (command, args) => {
      calls.push([command, ...args]);
      return { status: 0 };
    }),
    0,
  );
  assert.deepEqual(
    calls.map(([command]) => command),
    [...Array(4).fill(process.platform === "win32" ? "pnpm.cmd" : "pnpm"), "cargo", "cargo"],
  );
  assert.ok(calls.some((args) => args.includes("format:check")));
  assert.ok(calls.some((args) => args.includes("--all-targets")));
});

test("tracked shell hook propagates Node runner status", (t) => {
  if (process.platform === "win32")
    return t.skip("Git for Windows executes the shell hook; Node runner is tested above");
  const root = fixture(t);
  mkdirSync(join(root, "scripts"));
  writeFileSync(join(root, "scripts/pre-commit.mjs"), "process.exit(29);\n");
  copyFileSync(resolve(".githooks/pre-commit"), join(root, ".githooks/pre-commit"));
  const result = spawnSync("sh", [join(root, ".githooks/pre-commit")], { cwd: root });
  assert.equal(result.status, 29);
});

test("refuses inherited global hooksPath without writing local or global config", (t) => {
  const root = fixture(t);
  const globalConfig = join(root, "global.gitconfig");
  const content = "[core]\n\thooksPath = custom-global-hooks\n";
  writeFileSync(globalConfig, content);
  const result = install(root, { ...process.env, GIT_CONFIG_GLOBAL: globalConfig });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /custom-global-hooks/);
  assert.notEqual(hooksPath(root).status, 0);
  assert.equal(readFileSync(globalConfig, "utf8"), content);
});

test("pre-commit fails if a check cannot launch", async () => {
  const { runChecks } = await import("../scripts/pre-commit.mjs");
  assert.equal(
    runChecks(process.cwd(), () => ({ status: null })),
    1,
  );
});

test("refuses an explicitly empty hooksPath", (t) => {
  const root = fixture(t);
  execFileSync("git", ["config", "--local", "core.hooksPath", ""], { cwd: root });
  const result = install(root);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /core.hooksPath/);
  assert.equal(hooksPath(root).stdout, "\n");
});

test("refuses a dangling default pre-commit symlink", (t) => {
  if (process.platform === "win32") return t.skip("Creating symlinks requires Windows privileges");
  const root = fixture(t);
  symlinkSync("missing-user-hook", join(root, ".git/hooks/pre-commit"));
  const result = install(root);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /pre-commit/);
  assert.notEqual(hooksPath(root).status, 0);
});
