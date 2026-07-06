// Recommended local models by RAM tier — shown in the LLM config dialog for local
// cards (Ollama / LM Studio). Distilled from the project's local-LLM research report
// (the verifiable one); ONLY real tags (no hallucinated Qwen3.5/3.6 / Gemma 4).
//
// Qwen3 is the per-tier 主推 for Chinese polish; Gemma 3 / Granite 4.0 are honest
// alternatives — faster or leaner, but weaker on Chinese (Audie's primary use).
// Thinking hybrids (qwen3:8b / :14b / :32b) are safe here: the enhance request
// appends /no_think and the output sanitizer strips leaked reasoning
// (src-tauri/src/llm/mod.rs). Quant = Ollama default (Q4_K_M). Acquire via
// `ollama pull <tag>`, or search the name in LM Studio. No system-RAM auto-detection
// (out of scope) — the user picks the tier matching their machine.

export interface LocalModelRecommendation {
  ram: string; // RAM tier label, e.g. "8GB"
  name: string; // display name (also the LM Studio search term)
  tag: string; // Ollama pull tag; filled into the model field on click
  note: string; // honest one-liner: strengths / caveats
  noteKey?: I18nKey;
  primary: boolean; // the tier's 主推 (Qwen3, best for Chinese)
}

export const LOCAL_MODEL_RECOMMENDATIONS: LocalModelRecommendation[] = [
  // 8GB (3–4B)
  {
    ram: "8GB",
    name: "Qwen3 4B Instruct 2507",
    tag: "qwen3:4b-instruct-2507",
    note: "中文最佳、原生非思考",
    noteKey: "settings.config.reco.qwen3_4b_instruct",
    primary: true,
  },
  {
    ram: "8GB",
    name: "Gemma 3 4B",
    tag: "gemma3:4b",
    note: "快、标点好；中文细节弱、可能加 emoji",
    noteKey: "settings.config.reco.gemma3_4b",
    primary: false,
  },
  {
    ram: "8GB",
    name: "Granite 4.0 Micro",
    tag: "granite4:micro",
    note: "最省内存、输出老实；中文一般",
    noteKey: "settings.config.reco.granite4_micro",
    primary: false,
  },
  // 16GB (7–8B)
  {
    ram: "16GB",
    name: "Qwen3 8B",
    tag: "qwen3:8b",
    note: "中文强、平衡（已自动 /no_think）",
    noteKey: "settings.config.reco.qwen3_8b",
    primary: true,
  },
  {
    ram: "16GB",
    name: "Gemma 3 12B",
    tag: "gemma3:12b",
    note: "标点/多语好；无 system role、比 Qwen 慢",
    noteKey: "settings.config.reco.gemma3_12b",
    primary: false,
  },
  {
    ram: "16GB",
    name: "Granite 4.0 H-Tiny",
    tag: "granite4:tiny-h",
    note: "省内存、指令老实；中文一般",
    noteKey: "settings.config.reco.granite4_tiny_h",
    primary: false,
  },
  // 24GB (14B)
  {
    ram: "24GB",
    name: "Qwen3 14B",
    tag: "qwen3:14b",
    note: "中文润色质量高（已自动 /no_think）",
    noteKey: "settings.config.reco.qwen3_14b",
    primary: true,
  },
  {
    ram: "24GB",
    name: "Gemma 3 27B",
    tag: "gemma3:27b",
    note: "质量高、标点好；占用大、无 system role",
    noteKey: "settings.config.reco.gemma3_27b",
    primary: false,
  },
  {
    ram: "24GB",
    name: "Qwen3 30B-A3B Instruct 2507",
    tag: "qwen3:30b-a3b-instruct-2507",
    note: "MoE 快、原生非思考；24GB 余量偏紧",
    noteKey: "settings.config.reco.qwen3_30b_a3b_24",
    primary: false,
  },
  // 32GB+ (14–32B / 快速 MoE)
  {
    ram: "32GB+",
    name: "Qwen3 30B-A3B Instruct 2507",
    tag: "qwen3:30b-a3b-instruct-2507",
    note: "MoE、原生非思考、快、中文强",
    noteKey: "settings.config.reco.qwen3_30b_a3b_32",
    primary: true,
  },
  {
    ram: "32GB+",
    name: "Qwen3 32B",
    tag: "qwen3:32b",
    note: "质量最高，但慢（已自动 /no_think）",
    noteKey: "settings.config.reco.qwen3_32b",
    primary: false,
  },
];
import type { I18nKey } from "../../i18n";
