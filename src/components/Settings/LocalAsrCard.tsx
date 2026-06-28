// One local-ASR model row, install-state driven (model system v2 · Phase 3).
// Mirrors Handy's ModelCard (recommended/active badges, progress bar, cancel,
// delete) in Audie's tokens. The three states come straight from the ModelInfo
// on-disk fields the backend ModelManager computes:
//  - downloading → progress bar + 取消
//  - downloaded  → 选用 / 删除   (使用中 replaces 选用 when active)
//  - not yet     → 下载 (+ size)
// Cancelled downloads keep their .partial, so a re-download resumes; the row just
// returns to the 下载 state until then.

import { Badge, Button, Icon } from "../ui";
import type { ModelInfo } from "../../types/settings";

// Whole MB → human size. Catalog sizes are coarse, so MB/GB with one decimal is plenty.
function formatSize(sizeMb: number): string {
  if (sizeMb <= 0) return "";
  if (sizeMb < 1024) return `${sizeMb} MB`;
  return `${(sizeMb / 1024).toFixed(1)} GB`;
}

export function LocalAsrCard({
  model,
  inUse,
  progress,
  onSelect,
  onDownload,
  onCancel,
  onDelete,
}: {
  model: ModelInfo;
  inUse: boolean;
  // Live download percentage [0,100] while downloading, else undefined.
  progress: number | undefined;
  onSelect: () => void;
  onDownload: () => void;
  onCancel: () => void;
  onDelete: () => void;
}) {
  const downloading = model.is_downloading || progress !== undefined;
  const size = formatSize(model.size_mb);

  return (
    <div className="flex flex-col gap-2 rounded-md bg-surface-card px-3.5 py-[13px]">
      <div className="flex items-center gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium text-text-primary">{model.name}</span>
            {inUse ? <Badge tone="accent">使用中</Badge> : null}
            {model.is_recommended ? <Badge tone="neutral">推荐</Badge> : null}
            {model.is_custom ? <Badge tone="neutral">自定义</Badge> : null}
          </div>
          {model.description ? (
            <div className="mt-[3px] text-[11px] text-text-tertiary">{model.description}</div>
          ) : null}
        </div>

        {/* Right-side action(s), keyed by install state. */}
        {downloading ? null : model.is_downloaded ? (
          <div className="flex items-center gap-2">
            {!inUse ? (
              <Button size="sm" variant="secondary" onClick={onSelect}>
                选用
              </Button>
            ) : null}
            <Button size="sm" variant="ghost" icon="trash" onClick={onDelete} aria-label="删除" />
          </div>
        ) : (
          <Button size="sm" variant="secondary" icon="download" onClick={onDownload}>
            下载{size ? ` · ${size}` : ""}
          </Button>
        )}
      </div>

      {downloading ? (
        <div className="flex flex-col gap-1.5">
          <div className="h-1.5 w-full overflow-hidden rounded-full bg-gray-alpha-200">
            <div
              className="h-full rounded-full bg-accent-fill transition-[width] duration-300"
              style={{ width: `${Math.round(progress ?? 0)}%` }}
            />
          </div>
          <div className="flex items-center justify-between">
            <span className="flex items-center gap-1.5 text-[11px] text-text-tertiary">
              <Icon name="download" size={12} />
              下载中 {Math.round(progress ?? 0)}%
            </span>
            <Button size="sm" variant="ghost" onClick={onCancel}>
              取消
            </Button>
          </div>
        </div>
      ) : null}
    </div>
  );
}
