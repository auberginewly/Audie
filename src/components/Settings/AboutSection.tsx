// 关于 — updates · project · author, plus import/export (the design's About omits
// it, but we keep the functionality here so nothing is lost). Version is real;
// the check-update button and beta toggle are mock (App owns the demo update flow).

import { useState } from "react";

import type { UseSettings } from "../../hooks/useSettings";
import { Button, Icon, Switch } from "../ui";
import { openExternal } from "../../lib/open";
import { SettingSection, SettingRow } from "./SettingSection";
import { ConfigSection } from "./ConfigSection";

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

export function AboutSection({ data }: { data: UseSettings }) {
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
        <SettingRow label="作者" control={<ExtLink href="https://auberginewly.vercel.app">auberginewly</ExtLink>} />
      </SettingSection>

      <ConfigSection onImported={data.applyImported} />
    </>
  );
}
