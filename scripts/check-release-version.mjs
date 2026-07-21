#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = process.cwd();
const args = process.argv.slice(2);
const requireUnreleased = args.length === 1 && args[0] === "--require-unreleased";

if (args.length > 0 && !requireUnreleased) {
  fail("Usage: node scripts/check-release-version.mjs [--require-unreleased]");
}

const packageVersion = readJson("package.json").version;
const tauriVersion = readJson("src-tauri/tauri.conf.json").version;
const cargoToml = readText("src-tauri/Cargo.toml");
const cargoVersion = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1];

if (typeof packageVersion !== "string" || typeof tauriVersion !== "string" || cargoVersion === undefined) {
  fail("Unable to read all three release version sources");
}

if (packageVersion !== cargoVersion || packageVersion !== tauriVersion) {
  fail(
    `Version sources must match: package.json=${packageVersion}, Cargo.toml=${cargoVersion}, tauri.conf.json=${tauriVersion}`,
  );
}

if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(packageVersion)) {
  fail(`Release version must be a stable semantic version, got: ${packageVersion}`);
}

const releaseNotes = readText("RELEASE_NOTES.md");
const expectedTitle = `# Audie ${packageVersion} Preview`;
if (releaseNotes.split("\n", 1)[0] !== expectedTitle) {
  fail(`Release notes title must be: ${expectedTitle}`);
}

const englishHeading = releaseNotes.indexOf("## English");
const chineseHeading = releaseNotes.indexOf("## 中文");
if (englishHeading === -1 || chineseHeading === -1 || englishHeading >= chineseHeading) {
  fail("Release notes must place English before 中文");
}

if (requireUnreleased) {
  const tag = `v${packageVersion}`;
  const tagCheck = spawnSync("git", ["rev-parse", "--verify", "--quiet", `refs/tags/${tag}`], {
    cwd: root,
    stdio: "ignore",
  });
  if (tagCheck.status === 0) fail(`Tag ${tag} already exists`);
  if (tagCheck.status !== 1) fail(`Unable to check whether tag ${tag} exists`);
}

process.stdout.write(`${packageVersion}\n`);

function readJson(path) {
  return JSON.parse(readText(path));
}

function readText(path) {
  return readFileSync(resolve(root, path), "utf8");
}

function fail(message) {
  process.stderr.write(`${message}\n`);
  process.exit(1);
}
