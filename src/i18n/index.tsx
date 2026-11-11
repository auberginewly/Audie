import { createContext, useContext, type ReactNode } from "react";
import type { Language } from "./languages";
import { zhHans, type Messages, type I18nKey } from "./locales/zh-Hans";
import { zhHant } from "./locales/zh-Hant";
import { en } from "./locales/en";

export { LANGUAGES, isLanguage, type Language } from "./languages";
export type { I18nKey } from "./locales/zh-Hans";

type MessageParams = Partial<Record<string, string | number>>;

export const messages: Record<Language, Messages> = {
  "zh-Hans": zhHans,
  "zh-Hant": zhHant,
  en,
};

function interpolate(template: string, params?: MessageParams): string {
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (match: string, key: string) => {
    const value = params[key];
    return value === undefined ? match : String(value);
  });
}

export interface I18nContextValue {
  language: Language;
  t: (key: I18nKey, params?: MessageParams) => string;
}

const I18nContext = createContext<I18nContextValue | null>(null);

export function I18nProvider({ language, children }: { language: Language; children: ReactNode }) {
  const value: I18nContextValue = {
    language,
    t: (key, params) => interpolate(messages[language][key], params),
  };
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18nContextValue {
  const ctx = useContext(I18nContext);
  if (!ctx) {
    return {
      language: "zh-Hans",
      t: (key, params) => interpolate(messages["zh-Hans"][key], params),
    };
  }
  return ctx;
}
