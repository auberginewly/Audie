import { spawnSync } from "node:child_process";
import { readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const requested = process.argv.slice(2);
const directories = requested.length > 0 ? requested : ["tests", "src"];

function discover(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return discover(path);
    return entry.isFile() && entry.name.endsWith(".test.mjs") ? [path] : [];
  });
}

const files = directories.flatMap((directory) => discover(resolve(root, directory))).sort();
console.log(`Discovered ${files.length} test files.`);
if (files.length === 0) {
  console.error("No test files found.");
  process.exitCode = 1;
} else {
  const result = spawnSync(process.execPath, ["--experimental-strip-types", "--test", ...files], {
    cwd: root,
    stdio: "inherit",
  });
  if (result.error) console.error(result.error);
  process.exitCode = result.status ?? 1;
}
