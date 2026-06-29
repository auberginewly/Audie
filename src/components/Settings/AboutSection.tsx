// 关于 — version · project · author.

import { Button, Icon } from "../ui";
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
  return (
    <>
      <SettingSection icon="info" title="版本">
        <SettingRow
          label="Audie"
          divider={false}
          control={<span className="font-mono text-[13px] text-text-tertiary">0.0.0</span>}
        />
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
