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
import { useI18n, type I18nContextValue } from "../../i18n";

const RETENTION_OPTIONS: { id: Settings["history_retention"]; labelKey: Parameters<I18nContextValue["t"]>[0] }[] = [
  { id: "never", labelKey: "history.retention.never" },
  { id: "day", labelKey: "history.retention.day" },
  { id: "week", labelKey: "history.retention.week" },
  { id: "month", labelKey: "history.retention.month" },
  { id: "forever", labelKey: "history.retention.forever" },
];

// 按处理模式筛选历史。"all" = 全部；其余匹配 entry.mode（kind=empty 的 mode=polish，归润色/全部）。
type Filter = "all" | HistoryEntry["mode"];
const FILTERS: { id: Filter; labelKey: Parameters<I18nContextValue["t"]>[0] }[] = [
  { id: "all", labelKey: "history.filter.all" },
  { id: "polish", labelKey: "history.filter.polish" },
  { id: "rewrite", labelKey: "history.filter.rewrite" },
  { id: "compose", labelKey: "history.filter.compose" },
];

// Local-day bucketing for the section headers (今天 / 昨天 / MM月DD日).
function dayLabel(unixSeconds: number, t: I18nContextValue["t"]): string {
  const d = new Date(unixSeconds * 1000);
  const startOfDay = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const diffDays = Math.round((startOfDay(new Date()) - startOfDay(d)) / 86_400_000);
  if (diffDays <= 0) return t("history.day.today");
  if (diffDays === 1) return t("history.day.yesterday");
  return t("history.day.date", { month: d.getMonth() + 1, day: d.getDate() });
}

function timeLabel(unixSeconds: number): string {
  const d = new Date(unixSeconds * 1000);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

// Collapse the (already newest-first) list into contiguous day groups.
function groupByDay(entries: HistoryEntry[], t: I18nContextValue["t"]): { label: string; items: HistoryEntry[] }[] {
  const groups: { label: string; items: HistoryEntry[] }[] = [];
  for (const item of entries) {
    const label = dayLabel(item.created_at, t);
    const n = groups.length;
    if (n > 0 && groups[n - 1].label === label) {
      groups[n - 1].items.push(item);
    } else {
      groups.push({ label, items: [item] });
    }
  }
  return groups;
}

// 处理模式的中文名 —— 决定历史条目第二个框的标签（润色 / 改写 / 写作）。
function modeLabel(mode: HistoryEntry["mode"], t: I18nContextValue["t"]): string {
  return mode === "compose"
    ? t("history.mode.compose")
    : mode === "rewrite"
      ? t("history.mode.rewrite")
      : t("history.mode.polish");
}

// One labeled version of an entry (原文 / 润色 / 写作 / 改写) with hover actions. Body is
// `text` by default; pass `children` to render a structured body instead (改写 原文 does
// this to split 指令 / 引用 —— see RewriteRawBody).
function VersionBox({
  label,
  text,
  actions,
  children,
}: {
  label: string;
  text?: string;
  actions: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className="group/box rounded-md bg-surface-card px-3 py-2">
      <div className="mb-0.5 flex items-center justify-between gap-2">
        <span className="font-mono text-[10px] uppercase tracking-[0.04em] text-text-tertiary">{label}</span>
        <div className="flex items-center gap-0.5 opacity-0 transition-opacity duration-150 group-hover/box:opacity-100">
          {actions}
        </div>
      </div>
      {children ?? (
        <div className="whitespace-pre-wrap text-sm leading-5 text-text-primary [overflow-wrap:anywhere]">{text}</div>
      )}
    </div>
  );
}

// 改写的「原文」由后端 rewrite_history_raw 拼成 "指令：…\n\n引用：\n…"。拆成两段渲染出层级：
// 指令是你说的命令（正文），引用是被改写的选中内容（缩进引用块、置灰、左竖线）。格式对不上
// （老数据 / 后端改了格式）就退回平铺，不会显示坏。
function RewriteRawBody({ raw }: { raw: string }) {
  const PREFIX = "指令：";
  const MARKER = "\n\n引用：\n";
  const idx = raw.indexOf(MARKER);
  if (!raw.startsWith(PREFIX) || idx === -1) {
    return (
      <div className="whitespace-pre-wrap text-sm leading-5 text-text-primary [overflow-wrap:anywhere]">{raw}</div>
    );
  }
  const instruction = raw.slice(PREFIX.length, idx);
  const source = raw.slice(idx + MARKER.length);
  return (
    <div className="space-y-2">
      <div className="whitespace-pre-wrap text-sm leading-5 text-text-primary [overflow-wrap:anywhere]">
        {instruction}
      </div>
      <div className="flex gap-2.5">
        <div className="w-0.5 shrink-0 self-stretch rounded-full bg-border-strong" />
        <div className="min-w-0 whitespace-pre-wrap text-[13px] leading-5 text-text-secondary [overflow-wrap:anywhere]">
          {source}
        </div>
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
  const { t } = useI18n();
  const translatedMode = modeLabel(item.mode, t);
  return (
    <div className="group/row relative flex items-start gap-3 rounded-sm px-3 py-2.5 transition-colors duration-150 hover:bg-gray-alpha-100">
      {divider ? <div className="absolute inset-x-3 top-0 h-px bg-border-subtle" /> : null}
      <span className="mt-2 w-11 shrink-0 font-mono text-xs text-text-tertiary">{timeLabel(item.created_at)}</span>

      <div className="min-w-0 flex-1 space-y-1.5">
        {isEmpty ? (
          <div className="flex items-center gap-1.5 py-1 text-sm italic text-text-tertiary">
            <Icon name="alert" size={13} className="text-text-tertiary" />
            {t("history.emptyEntry")}
          </div>
        ) : (
          <>
            <VersionBox
              label={t("history.raw")}
              text={item.raw_text}
              actions={
                <>
                  <IconButton
                    name="copy"
                    label={t("history.copyRaw")}
                    size="sm"
                    onClick={() => {
                      onCopy(item.raw_text);
                    }}
                  />
                  {llmConfigured && item.mode === "polish" ? (
                    <IconButton
                      name="sparkles"
                      label={t("history.retryPolish")}
                      size="sm"
                      disabled={reenhancing}
                      onClick={() => {
                        onRetry(item.id);
                      }}
                    />
                  ) : null}
                </>
              }
            >
              {item.mode === "rewrite" ? <RewriteRawBody raw={item.raw_text} /> : null}
            </VersionBox>
            {item.enhanced_text ? (
              <VersionBox
                label={translatedMode}
                text={item.enhanced_text}
                actions={
                  <IconButton
                    name="copy"
                    label={t("history.copyMode", { mode: translatedMode })}
                    size="sm"
                    onClick={() => {
                      onCopy(item.enhanced_text ?? "");
                    }}
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
          trigger={<IconButton name="more" label={t("history.more")} size="sm" />}
          items={[
            {
              icon: "trash",
              label: t("history.delete"),
              tone: "danger",
              onClick: () => {
                onDelete(item.id);
              },
            },
          ]}
        />
      </div>
    </div>
  );
}

export function HistoryScreen({ data }: { data: UseSettings }) {
  const { t } = useI18n();
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
      .then((raw) => {
        setLlmConfigured(raw === true);
      })
      .catch(() => {
        setLlmConfigured(false);
      });
  }, []);

  const retention = data.settings?.history_retention ?? "forever";

  const flash = (msg: string) => {
    setToast(msg);
    window.setTimeout(() => {
      setToast(null);
    }, 1400);
  };
  const onCopy = (text: string) => {
    void navigator.clipboard.writeText(text).catch(() => {});
    flash(t("history.copied"));
  };
  const onRetry = (id: number) => {
    if (reenhancingId !== null) return;
    setReenhancingId(id);
    reenhance(id)
      .then(() => {
        flash(t("history.reenhanced"));
      })
      .catch((err) => {
        console.error("reenhance failed:", err);
        flash(t("history.reenhanceFailed"));
      })
      .finally(() => {
        setReenhancingId(null);
      });
  };
  const onDelete = (id: number) => void remove(id);

  const visible = filter === "all" ? entries : entries.filter((e) => e.mode === filter);
  const groups = groupByDay(visible, t);
  const filterOptions = FILTERS.map((item) => ({ id: item.id, label: t(item.labelKey) }));

  return (
    <div className="relative flex h-full min-h-0 flex-col">
      {/* Fixed top */}
      <div className="shrink-0 px-7 pt-6">
        <div className="mb-[18px] flex items-center justify-between pl-1">
          <h1 className="text-xl font-semibold tracking-[-0.4px] text-text-primary">{t("history.title")}</h1>
          <Menu
            align="right"
            width={208}
            trigger={<IconButton name="more" label={t("history.deleteAll")} />}
            items={[
              {
                icon: "trash",
                label: t("history.deleteAll"),
                tone: "danger",
                onClick: () => {
                  setClearOpen(true);
                },
              },
            ]}
          />
        </div>

        {/* Keep-history + privacy card */}
        <div className="mb-4 rounded-md bg-surface-card">
          <div className="flex items-center justify-between gap-4 p-3.5">
            <div className="flex min-w-0 items-start gap-2.5">
              <Icon name="history" size={16} className="mt-0.5 text-text-tertiary" />
              <div>
                <div className="text-sm font-medium text-text-primary">{t("history.keepTitle")}</div>
                <div className="mt-px text-xs text-text-tertiary">{t("history.keepDescription")}</div>
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
                    {t(o.labelKey)}
                  </option>
                ))}
              </Select>
            </div>
          </div>
          <div className="relative flex items-start gap-2.5 p-3.5">
            <div className="absolute inset-x-3.5 top-0 h-px bg-border-subtle" />
            <Icon name="shield" size={16} className="mt-px shrink-0 text-text-tertiary" />
            <div>
              <div className="text-[13px] font-medium text-text-primary">{t("history.privacyTitle")}</div>
              <div className="mt-0.5 text-xs leading-4 text-text-tertiary">{t("history.privacyDescription")}</div>
            </div>
          </div>
        </div>

        <div className="mb-2 pl-1">
          <Segmented value={filter} onChange={setFilter} options={filterOptions} />
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
            <div className="text-sm text-text-secondary">{t("history.emptyTitle")}</div>
            <div className="mt-1 text-xs">{t("history.emptyDescription")}</div>
          </div>
        )}
      </div>

      {/* Clear-all confirm */}
      <Dialog
        open={clearOpen}
        onClose={() => {
          setClearOpen(false);
        }}
        title={t("history.clearTitle")}
        actions={
          <>
            <Button
              variant="ghost"
              onClick={() => {
                setClearOpen(false);
              }}
            >
              {t("history.cancel")}
            </Button>
            <Button
              variant="danger"
              onClick={() => {
                void clearAll();
                setClearOpen(false);
              }}
            >
              {t("history.clearConfirm")}
            </Button>
          </>
        }
      >
        {t("history.clearBody")}
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
