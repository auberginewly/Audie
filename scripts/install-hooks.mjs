// 只在显式请求时修改当前 clone 的 Git 配置，已有用户 hook/config 不覆盖。
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, lstatSync } from "node:fs";
import { resolve } from "node:path";

function git(...args) {
  const result = spawnSync("git", args, { encoding: "utf8" });
  if (result.error || (result.status !== 0 && result.status !== 1)) {
    throw new Error(result.error?.message ?? result.stderr.trim());
  }
  return result;
}

try {
  const rootResult = git("rev-parse", "--show-toplevel");
  if (rootResult.status !== 0) throw new Error("Run hooks:install inside the Audie clone.");
  const root = rootResult.stdout.trim();
  const configured = git("config", "--get", "core.hooksPath");
  const current = configured.stdout.trim();
  if (configured.status === 0 && current !== ".githooks")
    throw new Error(`Refusing to replace existing core.hooksPath: ${current}`);
  const commonDirectory = resolve(git("rev-parse", "--git-common-dir").stdout.trim());
  if (!current && lstatSync(resolve(commonDirectory, "hooks/pre-commit"), { throwIfNoEntry: false })) {
    throw new Error("Refusing to bypass existing default pre-commit hook.");
  }
  const hook = resolve(root, ".githooks/pre-commit");
  if (!existsSync(hook)) throw new Error("Tracked .githooks/pre-commit is missing.");
  chmodSync(hook, 0o755);
  const result = git("config", "--local", "core.hooksPath", ".githooks");
  if (result.status !== 0) throw new Error(result.stderr.trim() || "Could not configure local hooksPath.");
  console.log("Installed .githooks for this clone; pnpm install does not install hooks.");
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
