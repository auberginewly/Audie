// 本地 LLM 供应商卡（Ollama / LM Studio）。把「检测到正在运行」融进同一张卡，而不是
// 单列一行「运行中」（语义重复）：服务在跑时，它的实时模型内联列出，点一下即选用——
// 选中模型本身就是「配置好了」。服务没跑时退化成配置入口（+ 若有存过的模型可重新选用）。

import { Badge, Button } from "../ui";
import type { DiscoveredLocalLlm } from "../../types/settings";

export function LocalLlmCard({
  name,
  isActive,
  activeModel,
  storedModel,
  usable,
  server,
  onPickStored,
  onPickModel,
  onConfigure,
}: {
  name: string;
  isActive: boolean; // this provider is the active LLM
  activeModel: string; // settings.openai_compatible_model (when active)
  storedModel: string; // llm_models[id] — last model picked for this provider
  usable: boolean; // has a stored model → can be re-activated without the server
  server: DiscoveredLocalLlm | null; // matched running server (live models), else null
  onPickStored: () => void; // re-activate the stored model (server off)
  onPickModel: (model: string) => void; // pick a specific live model
  onConfigure: () => void;
}) {
  const subtitle = isActive ? activeModel : storedModel;
  // When the server is running, 已运行 + the live list carry the state — don't also
  // show 未配置/已配置 (that's what felt redundant). 使用中 always wins.
  const statusBadge = isActive ? (
    <Badge tone="accent">使用中</Badge>
  ) : server ? null : usable ? (
    <Badge tone="success">已配置</Badge>
  ) : (
    <Badge tone="neutral">未配置</Badge>
  );
  return (
    <div className="flex flex-col gap-2 rounded-md bg-surface-card px-3.5 py-[13px]">
      <div className="flex items-center gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium text-text-primary">{name}</span>
            {statusBadge}
            <Badge tone="neutral">本地</Badge>
            {server ? <Badge tone="success">已运行</Badge> : null}
          </div>
          {subtitle ? (
            <div className="mt-[3px] font-mono text-[11px] text-text-tertiary">{subtitle}</div>
          ) : null}
        </div>
        {/* Re-activate the stored model only when the server isn't detected running;
            when it is, pick from the live list below. */}
        {!isActive && usable && !server ? (
          <Button size="sm" variant="secondary" onClick={onPickStored}>
            选用
          </Button>
        ) : null}
        <Button size="sm" variant="secondary" onClick={onConfigure}>
          配置
        </Button>
      </div>
      {server && server.models.length ? (
        <div className="flex flex-col gap-1.5 border-t border-border-subtle pt-2">
          {server.models.map((model) => {
            const inUse = isActive && model === activeModel;
            return (
              <div key={model} className="flex items-center gap-3">
                <span className="min-w-0 flex-1 truncate font-mono text-[12px] text-text-secondary">
                  {model}
                </span>
                {inUse ? (
                  <Badge tone="accent">使用中</Badge>
                ) : (
                  <Button size="sm" variant="ghost" onClick={() => onPickModel(model)}>
                    选用
                  </Button>
                )}
              </div>
            );
          })}
        </div>
      ) : null}
    </div>
  );
}
