// 关于 — version · project · author.

import { Button, Icon } from "../ui";
import { openExternal } from "../../lib/open";
import { SettingSection, SettingRow } from "./SettingSection";
import { useI18n } from "../../i18n";

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
  const { t } = useI18n();
  return (
    <>
      <SettingSection icon="info" title={t("settings.about.version")}>
        <SettingRow
          label="Audie"
          divider={false}
          control={<span className="font-mono text-[13px] text-text-tertiary">0.0.0</span>}
        />
      </SettingSection>

      <SettingSection icon="github" title={t("settings.about.project")}>
        <SettingRow
          label={t("settings.about.source")}
          divider={false}
          control={
            <ExtLink mono href={REPO_URL}>
              auberginewly/Audie
            </ExtLink>
          }
        />
        <SettingRow
          label={t("settings.about.issues")}
          control={<ExtLink href={`${REPO_URL}/issues`}>GitHub Issues</ExtLink>}
        />
        <SettingRow
          label={t("settings.about.author")}
          control={<ExtLink href="https://github.com/auberginewly">auberginewly</ExtLink>}
        />
      </SettingSection>

      <SettingSection icon="flag" title={t("settings.about.wizard")}>
        <SettingRow
          label={t("settings.about.rerunWizard")}
          description={t("settings.about.rerunWizardDesc")}
          divider={false}
          control={
            <Button size="sm" variant="secondary" onClick={onRerunSetup}>
              {t("settings.about.run")}
            </Button>
          }
        />
      </SettingSection>
    </>
  );
}
