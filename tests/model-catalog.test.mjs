import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { readFile } from "node:fs/promises";
import { promisify } from "node:util";
import test from "node:test";

const modelsPath = new URL("../src/components/Settings/models.ts", import.meta.url);
const wizardModelStepPath = new URL(
  "../src/components/screens/setup-wizard/ModelStep.tsx",
  import.meta.url,
);
const modelSectionPath = new URL("../src/components/Settings/ModelSection.tsx", import.meta.url);
const execFileAsync = promisify(execFile);

test("ASR catalog does not offer Groq", async () => {
  const catalog = await readFile(modelsPath, "utf8");

  assert.doesNotMatch(catalog, /id: "groq",\s*name: "Groq",\s*type: "asr"/);
});

test("application source contains no Groq ASR implementation", async () => {
  try {
    const { stdout } = await execFileAsync("git", ["grep", "-ni", "groq", "--", "src", "src-tauri/src"]);
    assert.equal(stdout, "");
  } catch (error) {
    if (error && typeof error === "object" && "code" in error && error.code === 1) return;
    throw error;
  }
});

test("active model badges require the model to be configured", async () => {
  const [wizardModelStep, modelSection] = await Promise.all([
    readFile(wizardModelStepPath, "utf8"),
    readFile(modelSectionPath, "utf8"),
  ]);

  assert.match(wizardModelStep, /\{inUse && configured \? \(/);
  assert.match(modelSection, /const statusBadge =\s*inUse && usable \? \(/);
});
