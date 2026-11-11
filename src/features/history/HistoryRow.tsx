import type { ReactNode } from "react";
import { Icon, IconButton, Menu } from "../../components/ui";
import { useI18n, type I18nContextValue } from "../../i18n";
import type { HistoryEntry } from "./schema";

function timeLabel(unixSeconds: number): string {
  const d = new Date(unixSeconds * 1000);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
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

export function HistoryRow({
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
