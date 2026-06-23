// History — honest empty state. Persistence is not implemented yet (PROJECT_SPEC
// non-goals), so this never implies stored data; it points to the first action.

import { Icon } from "../ui";

export function HistoryScreen() {
  return (
    <div className="px-1">
      <h1 className="mb-7 text-xl font-semibold leading-[26px] tracking-[-0.4px] text-text-primary">历史</h1>
      <div className="flex flex-col items-center gap-2 rounded-md bg-surface-card px-3.5 py-14 text-center">
        <span className="mb-1 inline-flex h-12 w-12 items-center justify-center rounded-full bg-gray-200 text-text-tertiary">
          <Icon name="history" size={22} />
        </span>
        <span className="text-sm text-text-secondary">还没有听写记录</span>
        <span className="max-w-[36ch] text-xs leading-[18px] text-text-tertiary">
          按住快捷键说话，即可插入第一段文字。历史持久化尚未实现。
        </span>
      </div>
    </div>
  );
}
