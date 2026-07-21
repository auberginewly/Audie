import assert from "node:assert/strict";
import test from "node:test";

import { SettingsSchema } from "../src/types/settings.ts";

const baseSettings = {
  hotkey: "Fn",
  asr_provider: "glm",
  asr_model: "glm-asr-1",
  llm_provider: "openai_compatible",
  enhance_enabled: true,
  enhance_prompt: "clean up",
  openai_compatible_base_url: "https://api.openai.com/v1",
  openai_compatible_model: "gpt-4o-mini",
  llm_api_key_id: "openai_compatible_api_key",
  doubao_endpoint: "wss://doubao.example.test",
  doubao_resource_id: "resource",
  glm_endpoint: "https://glm.example.test/audio/transcriptions",
  aliyun_endpoint: "wss://aliyun.example.test/inference",
  stepfun_endpoint: "https://stepfun.example.test/audio/transcriptions",
  input_device: "",
  onboarding_completed: true,
  onboarding_test_completed: true,
  primary_language: "zh-Hans",
  history_retention: "forever",
  ui_language: "zh-Hans",
  show_in_dock: true,
  compose_hotkey: "F13",
  compose_prompt: "compose",
  rewrite_prompt: "rewrite",
  llm_models: {},
};

test("parses all persisted ASR endpoints", () => {
  const parsed = SettingsSchema.parse(baseSettings);

  assert.equal(parsed.glm_endpoint, baseSettings.glm_endpoint);
  assert.equal(parsed.aliyun_endpoint, baseSettings.aliyun_endpoint);
  assert.equal(parsed.stepfun_endpoint, baseSettings.stepfun_endpoint);
});

test("rejects an empty provider endpoint", () => {
  const parsed = SettingsSchema.safeParse({ ...baseSettings, glm_endpoint: "" });

  assert.equal(parsed.success, false);
});
