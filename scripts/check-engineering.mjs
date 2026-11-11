// 检查手写源码规模与维护中的公共指南；直接读文件，不受 Git ignore 影响。
import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, extname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import * as prettier from "prettier";
import { parsers } from "prettier/plugins/markdown";

const publicGuides = ["CONTRIBUTING.md", "docs/ENGINEERING.md", "docs/ARCHITECTURE.md", "README.md", "README.zh-CN.md"];
const sourceRoots = new Set(["src", "src-tauri", "scripts", "tests"]);
const sourceExtensions = new Set([".rs", ".ts", ".tsx", ".mjs"]);
// 只排除实际构建/依赖目录；测试与 scripts 不享有规模豁免。
const outputDirectories = new Set(["node_modules", "target", "dist", "dist-ssr"]);

function sourceFiles(root) {
  function walk(directory) {
    return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) {
        if (outputDirectories.has(entry.name) || path === join(root, "src-tauri", "gen")) return [];
        return walk(path);
      }
      return entry.isFile() && sourceExtensions.has(extname(path)) ? [path] : [];
    });
  }
  return readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    if (entry.isDirectory() && sourceRoots.has(entry.name)) return walk(join(root, entry.name));
    return entry.isFile() && sourceExtensions.has(extname(entry.name)) ? [join(root, entry.name)] : [];
  });
}

function nodes(node) {
  return [node, ...(node.children ?? []).flatMap(nodes)];
}

function headingText(node) {
  return node.value ?? node.alt ?? (node.children ?? []).map(headingText).join("");
}

function anchors(ast) {
  const found = new Set();
  for (const node of nodes(ast)) {
    if (node.type === "heading") {
      const slug = headingText(node)
        .toLowerCase()
        .replace(/[^\p{L}\p{M}\p{N}_\-\s]/gu, "")
        .replace(/\s/g, "-");
      let unique = slug;
      for (let suffix = 1; found.has(unique); suffix += 1) unique = `${slug}-${suffix}`;
      found.add(unique);
    }
    if (node.type === "html") {
      for (const match of node.value.matchAll(/\b(?:id|name)=["']([^"']+)["']/g)) found.add(match[1]);
    }
  }
  return found;
}

function destinations(ast) {
  return nodes(ast).flatMap((node) => {
    if (["link", "image", "definition"].includes(node.type)) return [node.url];
    if (node.type === "html") {
      return [...node.value.matchAll(/\b(?:href|src)=["']([^"']+)["']/g)].map((match) => match[1]);
    }
    return [];
  });
}

export async function checkEngineering(root, extraGuides = []) {
  const problems = [];
  const sources = sourceFiles(root);
  if (sources.length === 0) problems.push("No handwritten source files discovered.");
  for (const file of sources) {
    const content = readFileSync(file, "utf8");
    const lines = content === "" ? 0 : content.split(/\r\n|\n|\r/).length - Number(/[\r\n]$/.test(content));
    if (lines > 400) problems.push(`${relative(root, file)}: ${lines} physical lines (maximum 400).`);
  }

  const parsed = new Map();
  async function markdown(file) {
    if (!parsed.has(file)) parsed.set(file, await parsers.markdown.parse(readFileSync(file, "utf8")));
    return parsed.get(file);
  }
  for (const guide of new Set([...publicGuides, ...extraGuides])) {
    const file = resolve(root, guide);
    try {
      if (extname(file) !== ".md") throw new Error("Expected a Markdown file.");
      const content = readFileSync(file, "utf8");
      const config = await prettier.resolveConfig(file);
      if (!(await prettier.check(content, { ...config, filepath: file }))) {
        problems.push(`${guide}: Markdown format differs from Prettier.`);
      }
      for (const destination of destinations(await markdown(file))) {
        if (/^(?:[a-z][a-z\d+.-]*:|\/\/)/i.test(destination)) continue;
        try {
          const [pathAndQuery, fragment] = destination.split("#");
          const path = decodeURIComponent(pathAndQuery.split("?")[0]);
          const target = path ? resolve(dirname(file), path) : file;
          const info = statSync(target);
          if (fragment && info.isFile() && extname(target) === ".md") {
            if (!anchors(await markdown(target)).has(decodeURIComponent(fragment))) throw new Error("Missing anchor.");
          }
        } catch {
          problems.push(`${guide}: broken relative link ${destination}`);
        }
      }
    } catch (error) {
      problems.push(`${guide}: ${error.message}`);
    }
  }
  return { problems, sourceCount: sources.length };
}

if (resolve(process.argv[1] ?? "") === fileURLToPath(import.meta.url)) {
  try {
    const result = await checkEngineering(process.cwd(), process.argv.slice(2));
    if (result.problems.length > 0) {
      console.error(result.problems.join("\n"));
      process.exitCode = 1;
    } else {
      console.log(
        `Engineering checks passed (${result.sourceCount} source files; public guides and requested Markdown).`,
      );
    }
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
