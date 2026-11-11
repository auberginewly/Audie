// Hook 只运行静态检查；提交失败保留原退出码，不修改工作区或暂存区。
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

export function runChecks(root, run = spawnSync) {
  const commands = [
    ["pnpm", ["check:engineering"]],
    ["pnpm", ["typecheck"]],
    ["pnpm", ["lint"]],
    ["pnpm", ["format:check"]],
    ["cargo", ["fmt", "--manifest-path", "src-tauri/Cargo.toml", "--", "--check"]],
    ["cargo", ["clippy", "--manifest-path", "src-tauri/Cargo.toml", "--all-targets", "--", "-D", "warnings"]],
  ];
  for (const [command, args] of commands) {
    const windowsPnpm = process.platform === "win32" && command === "pnpm";
    const result = run(windowsPnpm ? "pnpm.cmd" : command, args, {
      cwd: root,
      stdio: "inherit",
      // Windows 的 pnpm 是批处理入口；参数全部是上方固定值。
      shell: windowsPnpm,
    });
    if (result.error) console.error(result.error.message);
    if (result.status !== 0) return result.status ?? 1;
  }
  return 0;
}

if (resolve(process.argv[1] ?? "") === fileURLToPath(import.meta.url)) {
  process.exitCode = runChecks(fileURLToPath(new URL("../", import.meta.url)));
}
