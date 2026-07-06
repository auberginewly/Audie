import { useEffect, useState } from "react";

import type { SecretKeyId } from "../../../types/settings";
import { useI18n } from "../../../i18n";
import { IconButton, Input } from "../../ui";
import { getSecretForSettings } from "./modelConfigActions";

interface KeyInputProps {
  keyId: SecretKeyId;
  placeholder: string;
}

// Keychain-backed password field. Loading the existing secret is user-initiated by
// opening the config dialog; stable app signing keeps that read silent.
export function KeyInput({ keyId, placeholder }: KeyInputProps) {
  const { t } = useI18n();
  const [value, setValue] = useState("");
  const [revealed, setRevealed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    getSecretForSettings(keyId)
      .then((secret) => {
        if (!cancelled && secret) setValue(secret);
      })
      .catch(() => {
        // Missing or unreadable secrets should leave the field blank.
      });
    return () => {
      cancelled = true;
    };
  }, [keyId]);

  return (
    <div className="relative">
      <Input
        mono
        type={revealed ? "text" : "password"}
        value={value}
        onChange={(e) => {
          setValue(e.target.value);
        }}
        placeholder={placeholder}
        data-key-id={keyId}
        className="pr-9"
      />
      <div className="absolute inset-y-0 right-0 flex items-center pr-0.5">
        <IconButton
          name={revealed ? "eye" : "eye-off"}
          label={revealed ? t("settings.config.hide") : t("settings.config.reveal")}
          size="sm"
          onClick={() => {
            setRevealed((r) => !r);
          }}
        />
      </div>
    </div>
  );
}
