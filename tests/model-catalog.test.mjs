import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const modelsPath = new URL("../src/components/Settings/models.ts", import.meta.url);

test("ASR catalog does not offer Groq", async () => {
  const catalog = await readFile(modelsPath, "utf8");

  assert.doesNotMatch(catalog, /id: "groq",\s*name: "Groq",\s*type: "asr"/);
});
