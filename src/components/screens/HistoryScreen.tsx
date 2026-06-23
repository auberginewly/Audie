// History — full restoration of the design's Typeless-style layout: a fixed top
// (keep-history + privacy card, then filter pills) above a scrolling list. Each
// row emphasizes on hover and carries a ⋯ menu; the page ⋯ clears all.
//
// mock: persistence isn't implemented (PROJECT_SPEC non-goals). The list is the
// design's demo data; delete/filter/clear act on local state only.

import { useState } from "react";

import { Button, Dialog, Icon, IconButton, Menu, Segmented, Select, StatusMessage } from "../ui";

type HistKind = "polish" | "rewrite" | "compose" | "error";
type HistItem = { id: number; time: string; kind: HistKind; text: string };
type Filter = "all" | "polish" | "rewrite" | "compose";

// mock: the design's sample dictations (zh copy from strings.js · hist.samples).
const SAMPLES: HistItem[] = [
  { id: 1, time: "02:41", kind: "polish", text: "先把暗色主题发出去，下周再回头处理浅色 token。" },
  { id: 2, time: "02:37", kind: "rewrite", text: "提醒我开站会前把设计评审纪要发邮件。" },
  { id: 3, time: "02:18", kind: "error", text: "转录已被取消。" },
  { id: 4, time: "01:52", kind: "compose", text: "帮我写一段 v0.5 更新的简短发布说明，覆盖新的配置流程。" },
  { id: 5, time: "01:30", kind: "polish", text: "胶囊在处理时应当呼吸式脉动，成功时闪过一个对勾。" },
  { id: 6, time: "00:58", kind: "rewrite", text: "短编辑场景按住说话比切换模式更顺手。" },
];

const FILTERS: { id: Filter; label: string }[] = [
  { id: "all", label: "全部" },
  { id: "polish", label: "润色" },
  { id: "rewrite", label: "改写" },
  { id: "compose", label: "写作" },
];

function HistoryRow({
  item,
  divider,
  onCopy,
  onDelete,
  onRetry,
}: {
  item: HistItem;
  divider: boolean;
  onCopy: (i: HistItem) => void;
  onDelete: (i: HistItem) => void;
  onRetry: (i: HistItem) => void;
}) {
  const isError = item.kind === "error";
  return (
    <div className="group relative flex items-center gap-3.5 rounded-sm px-3 py-3 transition-colors duration-150 hover:bg-gray-alpha-100">
      {divider ? <div className="absolute inset-x-3 top-0 h-px bg-border-subtle" /> : null}
      <span className="w-11 shrink-0 font-mono text-xs text-text-tertiary">{item.time}</span>
      <div className="min-w-0 flex-1">
        <div
          className={[
            "overflow-hidden text-ellipsis whitespace-nowrap text-sm leading-5",
            isError ? "italic text-text-tertiary" : "text-text-primary",
          ].join(" ")}
        >
          {isError ? (
            <Icon name="alert" size={13} className="mr-1.5 inline align-[-2px] text-danger-text" />
          ) : null}
          {item.text}
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-1 opacity-0 transition-opacity duration-150 group-hover:opacity-100">
        {isError ? <IconButton name="refresh" label="重试" size="sm" onClick={() => onRetry(item)} /> : null}
        <Menu
          align="right"
          width={184}
          trigger={<IconButton name="more" label="更多" size="sm" />}
          items={[
            { icon: "copy", label: "复制文本", onClick: () => onCopy(item) },
            ...(isError ? [{ icon: "refresh" as const, label: "重试", onClick: () => onRetry(item) }] : []),
            { type: "divider" },
            { icon: "trash", label: "删除", tone: "danger", onClick: () => onDelete(item) },
          ]}
        />
      </div>
    </div>
  );
}

export function HistoryScreen() {
  const [items, setItems] = useState<HistItem[]>(SAMPLES);
  const [filter, setFilter] = useState<Filter>("all");
  const [retention, setRetention] = useState("forever");
  const [clearOpen, setClearOpen] = useState(false);
  const [toast, setToast] = useState<string | null>(null);

  const flash = (msg: string) => {
    setToast(msg);
    window.setTimeout(() => setToast(null), 1400);
  };
  const onCopy = (item: HistItem) => {
    void navigator.clipboard.writeText(item.text).catch(() => {});
    flash("已复制");
  };
  const onDelete = (item: HistItem) => setItems((xs) => xs.filter((x) => x.id !== item.id));
  const onRetry = () => {}; // mock: no real retry pipeline

  const filtered = items.filter((it) => filter === "all" || it.kind === filter);

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
              <Select value={retention} onChange={(e) => setRetention(e.target.value)}>
                <option value="never">从不</option>
                <option value="h24">24 小时</option>
                <option value="w1">1 周</option>
                <option value="m1">1 个月</option>
                <option value="forever">永远</option>
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

        {/* Filter pills */}
        <Segmented value={filter} options={FILTERS} onChange={setFilter} />
      </div>

      {/* Scrolling list */}
      <div className="min-h-0 flex-1 overflow-y-auto px-7 pb-7 pt-3.5 [overscroll-behavior:contain]">
        <div className="mb-1.5 pl-3 font-mono text-xs uppercase tracking-[0.04em] text-text-tertiary">今天</div>
        {filtered.length ? (
          <div>
            {filtered.map((it, i) => (
              <HistoryRow
                key={it.id}
                item={it}
                divider={i > 0}
                onCopy={onCopy}
                onDelete={onDelete}
                onRetry={onRetry}
              />
            ))}
          </div>
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
                setItems([]);
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

      {/* Copy toast */}
      {toast ? (
        <div className="absolute bottom-5 left-1/2 z-40 -translate-x-1/2 rounded-full bg-surface-overlay px-3.5 py-2 shadow-popover">
          <StatusMessage tone="success">{toast}</StatusMessage>
        </div>
      ) : null}
    </div>
  );
}
