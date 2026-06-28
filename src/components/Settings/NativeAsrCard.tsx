// macOS 本机听写 (SFSpeechRecognizer) picker card for the 本地 ASR group. Keyless +
// OS-managed model → nothing to download, no config dialog; just 选用 + a
// Speech-recognition authorization row (the provider's transcribe() hard-gates on
// it, so without the grant dictation always fails). macOS-only provider.

import { Badge, Button } from "../ui";
import type { PermissionState } from "../../hooks/usePermissions";

export function NativeAsrCard({
  inUse,
  speech,
  onPick,
}: {
  inUse: boolean;
  speech: PermissionState;
  onPick: () => void;
}) {
  const granted = speech.granted === true;
  return (
    <div className="flex flex-col gap-2 rounded-md bg-surface-card px-3.5 py-[13px]">
      <div className="flex items-center gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium text-text-primary">macOS 本机听写</span>
            {inUse ? <Badge tone="accent">使用中</Badge> : null}
            <Badge tone="neutral">本地</Badge>
            <Badge tone="neutral">内置</Badge>
          </div>
          <div className="mt-[3px] font-mono text-[11px] text-text-tertiary">系统内置（离线）</div>
        </div>
        {!inUse ? (
          <Button size="sm" variant="secondary" onClick={onPick}>
            选用
          </Button>
        ) : null}
      </div>
      {/* Speech-recognition authorization: without it transcribe() returns Permission.
          macOS won't re-prompt after a denial, so keep the 去设置 fallback. */}
      {!granted ? (
        <div className="flex items-center gap-2 border-t border-border-subtle pt-2">
          <span className="flex-1 text-[11px] text-text-tertiary">需要「语音识别」权限才能离线转写</span>
          <Button size="sm" variant="secondary" onClick={speech.request}>
            授权
          </Button>
          <Button size="sm" variant="ghost" onClick={speech.openSettings}>
            去设置
          </Button>
        </div>
      ) : null}
    </div>
  );
}
