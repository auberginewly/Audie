// Home — the landing. Real content only: slogan, the configured hotkey, and the
// live recording state from the Rust event stream. No fabricated stats (the
// backend tracks none yet); the stat area is an honest empty state.

import { useRecordingStore } from "../../store/recording";
import type { AppState } from "../../types/events";
import { Badge, KeyCombo, type BadgeTone } from "../ui";

const STATE_LABEL: Record<AppState, { text: string; tone: BadgeTone }> = {
  IDLE: { text: "待命", tone: "neutral" },
  RECORDING: { text: "录音中…", tone: "accent" },
  PROCESSING: { text: "处理中…", tone: "accent" },
  SUCCESS: { text: "已插入", tone: "success" },
  ERROR: { text: "出错了", tone: "danger" },
  CANCEL: { text: "已取消", tone: "neutral" },
};

export function HomeScreen({ hotkey }: { hotkey: string }) {
  const state = useRecordingStore((s) => s.state);
  const label = STATE_LABEL[state] ?? STATE_LABEL.IDLE;
  const keys = hotkey.split("+").map((k) => k.trim().toLowerCase());

  return (
    <div className="px-1">
      <div className="mb-7">
        <h1 className="max-w-[34ch] text-balance text-xl font-semibold leading-[26px] tracking-[-0.4px] text-text-primary">
          按住快捷键说话，松手后干净的文字落在光标处。
        </h1>
        <div className="mt-3 flex items-center gap-2 text-sm text-text-tertiary">
          <span>按住</span>
          <KeyCombo keys={keys} />
          <span>开始说话</span>
        </div>
      </div>

      <div className="mb-3 flex items-center gap-2">
        <span className="font-mono text-xs uppercase tracking-[0.04em] text-text-tertiary">当前状态</span>
        <Badge tone={label.tone} dot>
          {label.text}
        </Badge>
      </div>

      <div className="flex flex-col items-center gap-1.5 rounded-md bg-surface-card px-3.5 py-9 text-center">
        <span className="text-[13px] text-text-secondary">还没有听写记录</span>
        <span className="text-xs text-text-tertiary">按住快捷键说话，即可插入第一段文字。</span>
      </div>
    </div>
  );
}
