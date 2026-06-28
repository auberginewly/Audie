// Recommended local Ollama models by RAM tier — shown in the LLM config dialog for
// local cards (Ollama / LM Studio). Verified-real tags only (the web is full of
// hallucinated Qwen3.5/3.6 / Gemma 4 — excluded). Tiers are sized so the Q4_K_M
// weights fit the RAM with headroom. Default-thinking hybrids (qwen3:8b / :14b) are
// safe here because the enhance request appends /no_think (Qwen3 skips <think>) and
// the output sanitizer strips any leaked reasoning — see src-tauri/src/llm/mod.rs.
// Quantization is Ollama's default for each tag (Q4_K_M today). No system-RAM
// auto-detection (out of scope) — the user picks the tier matching their machine.

export type LocalModelRecommendation = {
  ram: string; // RAM tier label, e.g. "8GB"
  tag: string; // Ollama tag to fill into the model field
  note: string; // why this tag fits the tier
};

export const LOCAL_MODEL_RECOMMENDATIONS: LocalModelRecommendation[] = [
  { ram: "8GB", tag: "qwen3:4b-instruct-2507", note: "原生非思考" },
  { ram: "16GB", tag: "qwen3:8b", note: "已自动 /no_think 关思考" },
  { ram: "24GB", tag: "qwen3:14b", note: "已自动 /no_think 关思考" },
  { ram: "32GB+", tag: "qwen3:30b-a3b-instruct-2507", note: "MoE，原生非思考" },
];
