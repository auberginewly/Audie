import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import test from "node:test";

const script = resolve("scripts/check-engineering.mjs");
const guides = ["CONTRIBUTING.md", "docs/ENGINEERING.md", "docs/ARCHITECTURE.md", "README.md", "README.zh-CN.md"];

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "audie-engineering-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const write = (file, content) => {
    mkdirSync(dirname(join(root, file)), { recursive: true });
    writeFileSync(join(root, file), content);
  };
  for (const guide of guides) write(guide, "# Guide\n\n## 测试与验收\n");
  write("src/example.ts", "export {};\n");
  write(".gitignore", "docs/\n");
  return { root, write };
}

function check(root, ...args) {
  return spawnSync(process.execPath, [script, ...args], { cwd: root, encoding: "utf8" });
}

test("accepts exactly 400 physical lines including comments and blanks", (t) => {
  const { root, write } = fixture(t);
  write("src/example.ts", "// comment\n\n".repeat(200));
  const result = check(root);
  assert.equal(result.status, 0, result.stderr);
});

test("rejects 401 lines in handwritten source, tests and scripts", (t) => {
  const { root, write } = fixture(t);
  for (const file of ["src/example.ts", "src-tauri/src/example.rs", "tests/example.mjs", "scripts/example.mjs"]) {
    write(file, "\n".repeat(401));
  }
  const result = check(root);
  assert.equal(result.status, 1);
  for (const file of ["src/example.ts", "src-tauri/src/example.rs", "tests/example.mjs", "scripts/example.mjs"]) {
    assert.ok(result.stderr.includes(`${file}: 401 physical lines`), result.stderr);
  }
});

test("does not count build, generated or dependency output", (t) => {
  const { root, write } = fixture(t);
  for (const file of [
    "src-tauri/target/output.rs",
    "src-tauri/gen/output.rs",
    "src/node_modules/vendor.ts",
    "dist/out.ts",
  ]) {
    write(file, "\n".repeat(401));
  }
  const result = check(root);
  assert.equal(result.status, 0, result.stderr);
});

test("checks ignored public docs and rejects missing paths and anchors", (t) => {
  const { root, write } = fixture(t);
  write("docs/ENGINEERING.md", "# Guide\n\n[missing](missing.md)\n\n[anchor](../README.md#missing)\n");
  const result = check(root);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /docs\/ENGINEERING.md.*missing.md/);
  assert.match(result.stderr, /README.md#missing/);
});

test("accepts encoded Unicode anchors, reference links, duplicate headings and files", (t) => {
  const { root, write } = fixture(t);
  write("README.md", "# Guide\n\n## Same\n\n## Same\n");
  write(
    "docs/ENGINEERING.md",
    "# Guide\n\n[验收](#%E6%B5%8B%E8%AF%95%E4%B8%8E%E9%AA%8C%E6%94%B6)\n\n## 测试与验收\n\n[second][ref]\n\n[ref]: ../README.md#same-1\n\n[code](../src/example.ts)\n\n[web](https://example.com/missing)\n\n`[code sample](missing.md)`\n",
  );
  const result = check(root);
  assert.equal(result.status, 0, result.stderr);
});

test("rejects formatting mismatches even in ignored docs", (t) => {
  const { root, write } = fixture(t);
  write("docs/ENGINEERING.md", "# Guide\n\n-   inconsistent list\n");
  const result = check(root);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /docs\/ENGINEERING.md.*format/);
});

test("explicit Markdown arguments check ignored files outside the public list", (t) => {
  const { root, write } = fixture(t);
  write("docs/change.md", "# Change\n\n[missing](absent.md)\n");
  assert.equal(check(root).status, 0);
  const result = check(root, "docs/change.md");
  assert.equal(result.status, 1);
  assert.match(result.stderr, /docs\/change.md.*absent.md/);
});

test("fails on zero discovered source files and missing public guides", (t) => {
  const { root } = fixture(t);
  rmSync(join(root, "src"), { recursive: true });
  rmSync(join(root, "CONTRIBUTING.md"));
  const result = check(root);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /No handwritten source files/);
  assert.match(result.stderr, /CONTRIBUTING.md/);
});
