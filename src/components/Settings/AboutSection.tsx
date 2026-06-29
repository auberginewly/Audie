// 关于 — updates · project · author. Version is real; the check-update button and
// beta toggle are mock (App owns the demo update flow).

import { useState } from "react";

import { Button, Icon, Switch } from "../ui";
import { openExternal } from "../../lib/open";
import { SettingSection, SettingRow } from "./SettingSection";

const REPO_URL = "https://github.com/auberginewly/Audie";

function ExtLink({ href, children, mono }: { href: string; children: string; mono?: boolean }) {
  return (
    <a
      href={href}
      onClick={(e) => {
        e.preventDefault();
        openExternal(href);
      }}
      className={[
        "inline-flex items-center gap-1.5 text-sm text-text-secondary no-underline transition-colors hover:text-text-primary",
        mono ? "font-mono" : "",
      ].join(" ")}
    >
      <span>{children}</span>
      <Icon name="arrow-up-right" size={14} className="text-text-tertiary" />
    </a>
  );
}

export function AboutSection({ onRerunSetup }: { onRerunSetup: () => void }) {
  const [beta, setBeta] = useState(false);
  return (
    <>
      <SettingSection icon="info" title="更新检查">
        <SettingRow
          label="版本"
          divider={false}
          control={
            <div className="flex items-center gap-3">
              <span className="font-mono text-[13px] text-text-tertiary">0.0.0</span>
              {/* mock: no real update channel — App owns the demo flow */}
              <Button size="sm" variant="secondary">
                检查更新
              </Button>
            </div>
          }
        />
        {/* mock: beta channel isn't backed */}
        <SettingRow label="Beta 更新" control={<Switch checked={beta} onChange={setBeta} />} />
      </SettingSection>

      <SettingSection icon="github" title="关于项目">
        <SettingRow label="源代码" divider={false} control={<ExtLink mono href={REPO_URL}>auberginewly/Audie</ExtLink>} />
        <SettingRow label="问题反馈" control={<ExtLink href={`${REPO_URL}/issues`}>GitHub Issues</ExtLink>} />
        <SettingRow label="作者" control={<ExtLink href="https://github.com/auberginewly">auberginewly</ExtLink>} />
      </SettingSection>

      <SettingSection icon="flag" title="配置向导">
        <SettingRow
          label="重新运行配置向导"
          description="重新过一遍权限、模型与测试录音。"
          divider={false}
          control={
            <Button size="sm" variant="secondary" onClick={onRerunSetup}>
              运行
            </Button>
          }
        />
      </SettingSection>
    </>
  );
}
