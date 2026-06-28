// Recommended local Ollama models by RAM tier — shown in the LLM config dialog
// for local cards (Ollama / LM Studio). Verified-real tags only, and ONLY native
// non-thinking models (the *-instruct-2507 line) — polish must not emit
// <think>…</think> into the cursor, and we don't post-process reasoning out, so
// default-thinking hybrids (qwen3:8b / :14b) are deliberately excluded rather
// than relying on a /no_think suffix we don't actually send. Quantization is
// Ollama's default for each tag (Q4_K_M today). No system-RAM auto-detection
// (out of scope) — the user picks the tier that matches their machine.

export type LocalModelRecommendation = {
  ram: string; // RAM tier label, e.g. "8GB"
  tag: string; // Ollama tag to fill into the model field
  note: string; // why this tag fits the tier
};

export const LOCAL_MODEL_RECOMMENDATIONS: LocalModelRecommendation[] = [
  { ram: "8GB", tag: "qwen3:4b-instruct-2507", note: "原生非思考" },
  { ram: "16GB+", tag: "qwen3:30b-a3b-instruct-2507", note: "MoE，原生非思考" },
];
