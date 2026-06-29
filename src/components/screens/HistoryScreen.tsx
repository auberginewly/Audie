// History — Typeless-style layout: a fixed top (keep-history + privacy card) above
// a single chronological list grouped by day. Each entry shows the transcript (原文)
// and, when polished, the enhanced version (润色) as separate copyable boxes. When an
// LLM is configured, 原文 carries a 重试 button to (re)generate the 润色 version — no
// audio needed (re-enhance, not re-transcribe). Entries with no recognized speech
// render as 没有识别到内容. Future 改写/写作 versions slot in as more boxes.

import { type ReactNode, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { Button, Dialog, Icon, IconButton, Menu, Segmented, Select, StatusMessage } from "../ui";
import { useHistory } from "../../hooks/useHistory";
import type { UseSettings } from "../../hooks/useSettings";
import type { Settings } from "../../types/settings";
import type { HistoryEntry } from "../../types/history";

const RETENTION_OPTIONS: { id: Settings["history_retention"]; label: string }[] = [
  { id: "never", label: "从不" },
  { id: "day", label: "24 小时" },
  { id: "week", label: "7 天" },
  { id: "month", label: "30 天" },
  { id: "forever", label: "永远" },
];

// 按处理模式筛选历史。"all" = 全部；其余匹配 entry.mode（kind=empty 的 mode=polish，归润色/全部）。
type Filter = "all" | HistoryEntry["mode"];
const FILTERS: { id: Filter; label: string }[] = [
  { id: "all", label: "全部" },
  { id: "polish", label: "润色" },
  { id: "rewrite", label: "改写" },
  { id: "compose", label: "写作" },
];

// Local-day bucketing for the section headers (今天 / 昨天 / MM月DD日).
function dayLabel(unixSeconds: number): string {
  const d = new Date(unixSeconds * 1000);
  const startOfDay = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const diffDays = Math.round((startOfDay(new Date()) - startOfDay(d)) / 86_400_000);
  if (diffDays <= 0) return "今天";
  if (diffDays === 1) return "昨天";
  return `${d.getMonth() + 1}月${d.getDate()}日`;
}

function timeLabel(unixSeconds: number): string {
  const d = new Date(unixSeconds * 1000);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

// Collapse the (already newest-first) list into contiguous day groups.
function groupByDay(entries: HistoryEntry[]): { label: string; items: HistoryEntry[] }[] {
  const groups: { label: string; items: HistoryEntry[] }[] = [];
  for (const item of entries) {
    const label = dayLabel(item.created_at);
    const last = groups[groups.length - 1];
    if (last && last.label === label) last.items.push(item);
    else groups.push({ label, items: [item] });
  }
  return groups;
}

// 处理模式的中文名 —— 决定历史条目第二个框的标签（润色 / 改写 / 写作）。
function modeLabel(mode: HistoryEntry["mode"]): string {
  return mode === "compose" ? "写作" : mode === "rewrite" ? "改写" : "润色";
}

// One labeled version of an entry (原文 / 润色 / 写作 / 改写) with hover actions.
function VersionBox({ label, text, actions }: { label: string; text: string; actions: ReactNode }) {
  return (
    <div className="group/box rounded-md bg-surface-card px-3 py-2">
      <div className="mb-0.5 flex items-center justify-between gap-2">
        <span className="font-mono text-[10px] uppercase tracking-[0.04em] text-text-tertiary">{label}</span>
        <div className="flex items-center gap-0.5 opacity-0 transition-opacity duration-150 group-hover/box:opacity-100">
          {actions}
        </div>
      </div>
      <div className="whitespace-pre-wrap text-sm leading-5 text-text-primary [overflow-wrap:anywhere]">
        {text}
      </div>
    </div>
  );
}

function HistoryRow({
  item,
  divider,
  llmConfigured,
  reenhancing,
  onCopy,
  onRetry,
  onDelete,
}: {
  item: HistoryEntry;
  divider: boolean;
  llmConfigured: boolean;
  reenhancing: boolean;
  onCopy: (text: string) => void;
  onRetry: (id: number) => void;
  onDelete: (id: number) => void;
}) {
  const isEmpty = item.kind === "empty";
  return (
    <div className="group/row relative flex items-start gap-3 rounded-sm px-3 py-2.5 transition-colors duration-150 hover:bg-gray-alpha-100">
      {divider ? <div className="absolute inset-x-3 top-0 h-px bg-border-subtle" /> : null}
      <span className="mt-2 w-11 shrink-0 font-mono text-xs text-text-tertiary">{timeLabel(item.created_at)}</span>

      <div className="min-w-0 flex-1 space-y-1.5">
        {isEmpty ? (
          <div className="flex items-center gap-1.5 py-1 text-sm italic text-text-tertiary">
            <Icon name="alert" size={13} className="text-text-tertiary" />
            没有识别到内容
          </div>
        ) : (
          <>
            <VersionBox
              label="原文"
              text={item.raw_text}
              actions={
                <>
                  <IconButton name="copy" label="复制原文" size="sm" onClick={() => onCopy(item.raw_text)} />
                  {llmConfigured && item.mode === "polish" ? (
                    <IconButton
                      name="sparkles"
                      label="重试润色"
                      size="sm"
                      disabled={reenhancing}
                      onClick={() => onRetry(item.id)}
                    />
                  ) : null}
                </>
              }
            />
            {item.enhanced_text ? (
              <VersionBox
                label={modeLabel(item.mode)}
                text={item.enhanced_text}
                actions={
                  <IconButton
                    name="copy"
                    label={`复制${modeLabel(item.mode)}`}
                    size="sm"
                    onClick={() => onCopy(item.enhanced_text ?? "")}
                  />
                }
              />
            ) : null}
          </>
        )}
      </div>

      <div className="mt-1 shrink-0 opacity-0 transition-opacity duration-150 group-hover/row:opacity-100">
        <Menu
          align="right"
          width={160}
          trigger={<IconButton name="more" label="更多" size="sm" />}
          items={[{ icon: "trash", label: "删除", tone: "danger", onClick: () => onDelete(item.id) }]}
        />
      </div>
    </div>
  );
}

export function HistoryScreen({ data }: { data: UseSettings }) {
  const { entries, remove, clearAll, reenhance } = useHistory();
  const [clearOpen, setClearOpen] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [reenhancingId, setReenhancingId] = useState<number | null>(null);
  const [llmConfigured, setLlmConfigured] = useState(false);
  const [filter, setFilter] = useState<Filter>("all");

  // Show the 重试 (re-enhance) button only when an LLM key is set — re-checked each
  // time the History screen mounts (switching tabs remounts it).
  useEffect(() => {
    invoke("has_secret", { keyId: "openai_compatible_api_key" })
      .then((raw) => setLlmConfigured(raw === true))
      .catch(() => setLlmConfigured(false));
  }, []);

  const retention = data.settings?.history_retention ?? "forever";

  const flash = (msg: string) => {
    setToast(msg);
    window.setTimeout(() => setToast(null), 1400);
  };
  const onCopy = (text: string) => {
    void navigator.clipboard.writeText(text).catch(() => {});
    flash("已复制");
  };
  const onRetry = (id: number) => {
    if (reenhancingId !== null) return;
    setReenhancingId(id);
    reenhance(id)
      .then(() => flash("已重新润色"))
      .catch((err) => {
        console.error("reenhance failed:", err);
        flash("润色失败");
      })
      .finally(() => setReenhancingId(null));
  };
  const onDelete = (id: number) => void remove(id);

  const visible = filter === "all" ? entries : entries.filter((e) => e.mode === filter);
  const groups = groupByDay(visible);

  return (
    <div className="relative flex h-full min-h-0 flex-col">
      {/* Fixed top */}
      <div className="shrink-0 px-7 pt-6">
        <div className="mb-[18px] flex items-center justify-between pl-1">
          <h1 className="text-xl font-semibold tracking-[-0.4px] text-text-primary">历史记录</h1>
          <Menu
            align="right"
            width={208}
            trigger={<IconButton name="more" label="删除所有历史记录" />}
            items={[
              { icon: "trash", label: "删除所有历史记录", tone: "danger", onClick: () => setClearOpen(true) },
            ]}
          />
        </div>

        {/* Keep-history + privacy card */}
        <div className="mb-4 rounded-md bg-surface-card">
          <div className="flex items-center justify-between gap-4 p-3.5">
            <div className="flex min-w-0 items-start gap-2.5">
              <Icon name="history" size={16} className="mt-0.5 text-text-tertiary" />
              <div>
                <div className="text-sm font-medium text-text-primary">保存历史</div>
                <div className="mt-px text-xs text-text-tertiary">你希望在设备上保存口述历史多久？</div>
              </div>
            </div>
            <div className="w-[150px] shrink-0">
              <Select
                value={retention}
                onChange={(e) =>
                  void data.update({ history_retention: e.target.value as Settings["history_retention"] })
                }
              >
                {RETENTION_OPTIONS.map((o) => (
                  <option key={o.id} value={o.id}>
                    {o.label}
                  </option>
                ))}
              </Select>
            </div>
          </div>
          <div className="relative flex items-start gap-2.5 p-3.5">
            <div className="absolute inset-x-3.5 top-0 h-px bg-border-subtle" />
            <Icon name="shield" size={16} className="mt-px shrink-0 text-text-tertiary" />
            <div>
              <div className="text-[13px] font-medium text-text-primary">你的数据保持私密</div>
              <div className="mt-0.5 text-xs leading-4 text-text-tertiary">
                你的语音口述不会离开本机 —— 仅存储在你的设备上，服务端零数据保留，无法从其他地方访问。
              </div>
            </div>
          </div>
        </div>

        <div className="mb-2 pl-1">
          <Segmented value={filter} onChange={setFilter} options={FILTERS} />
        </div>
      </div>

      {/* Scrolling list */}
      <div className="min-h-0 flex-1 overflow-y-auto px-7 pb-7 pt-3.5 [overscroll-behavior:contain]">
        {groups.length ? (
          groups.map((group) => (
            <div key={group.label} className="mb-2">
              <div className="mb-1.5 pl-3 font-mono text-xs uppercase tracking-[0.04em] text-text-tertiary">
                {group.label}
              </div>
              {group.items.map((it, i) => (
                <HistoryRow
                  key={it.id}
                  item={it}
                  divider={i > 0}
                  llmConfigured={llmConfigured}
                  reenhancing={reenhancingId === it.id}
                  onCopy={onCopy}
                  onRetry={onRetry}
                  onDelete={onDelete}
                />
              ))}
            </div>
          ))
        ) : (
          <div className="py-12 text-center text-text-tertiary">
            <Icon name="history" size={26} className="mx-auto mb-3 opacity-60" />
            <div className="text-sm text-text-secondary">还没有历史记录</div>
            <div className="mt-1 text-xs">按住快捷键说话，即可插入第一段文字。</div>
          </div>
        )}
      </div>

      {/* Clear-all confirm */}
      <Dialog
        open={clearOpen}
        onClose={() => setClearOpen(false)}
        title="删除所有历史记录？"
        actions={
          <>
            <Button variant="ghost" onClick={() => setClearOpen(false)}>
              取消
            </Button>
            <Button
              variant="danger"
              onClick={() => {
                void clearAll();
                setClearOpen(false);
              }}
            >
              全部删除
            </Button>
          </>
        }
      >
        这将永久移除本机上保存的全部口述记录，无法撤销。
      </Dialog>

      {/* Toast (copy / re-enhance) */}
      {toast ? (
        <div className="absolute bottom-5 left-1/2 z-40 -translate-x-1/2 rounded-full bg-surface-overlay px-3.5 py-2 shadow-popover">
          <StatusMessage tone="success">{toast}</StatusMessage>
        </div>
      ) : null}
    </div>
  );
}
