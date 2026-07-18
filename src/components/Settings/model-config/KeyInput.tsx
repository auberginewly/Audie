import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import type { SecretKeyId } from "../../../types/settings";
import { useI18n } from "../../../i18n";
import { IconButton, Input } from "../../ui";

interface KeyInputProps {
  keyId: SecretKeyId;
  placeholder: string;
}

// Opening Settings only checks presence. The saved value enters the WebView only
// after the user explicitly clicks Reveal; a blank field still preserves it on save.
export function KeyInput({ keyId, placeholder }: KeyInputProps) {
  const { t } = useI18n();
  const [value, setValue] = useState("");
  const [revealed, setRevealed] = useState(false);
  const [configured, setConfigured] = useState(false);
  const [revealing, setRevealing] = useState(false);

  useEffect(() => {
    let cancelled = false;
    invoke("has_secret", { keyId })
      .then((present) => {
        if (!cancelled) setConfigured(present === true);
      })
      .catch(() => {
        if (!cancelled) setConfigured(false);
      });
    return () => {
      cancelled = true;
    };
  }, [keyId]);

  const toggleReveal = () => {
    if (revealed) {
      setRevealed(false);
      return;
    }
    if (value) {
      setRevealed(true);
      return;
    }
    if (!configured || revealing) return;

    setRevealing(true);
    invoke("get_secret_for_settings", { keyId })
      .then((secret) => {
        if (typeof secret === "string" && secret) {
          setValue(secret);
          setRevealed(true);
        }
      })
      .catch(() => {
        setRevealed(false);
      })
      .finally(() => {
        setRevealing(false);
      });
  };

  return (
    <div className="relative">
      <Input
        mono
        type={revealed ? "text" : "password"}
        value={value}
        onChange={(e) => {
          setValue(e.target.value);
        }}
        placeholder={configured ? t("settings.config.keyConfiguredPlaceholder") : placeholder}
        data-key-id={keyId}
        className="pr-9"
      />
      <div className="absolute inset-y-0 right-0 flex items-center pr-0.5">
        <IconButton
          name={revealed ? "eye" : "eye-off"}
          label={revealed ? t("settings.config.hide") : t("settings.config.reveal")}
          size="sm"
          disabled={revealing || (!configured && !value)}
          onClick={toggleReveal}
        />
      </div>
    </div>
  );
}
