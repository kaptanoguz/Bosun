// Arayüz metinleri. Varsayılan İngilizce; Ayarlar'dan Türkçe seçilebilir.
// Dil seçimi servisin yapılandırmasında tutulur, burada yalnızca son seçim önbelleğe alınır
// ki servis kapalıyken de arayüz doğru dilde açılsın.

import { createContext, useContext } from "react";
import { setLocale } from "./format";
import { en, type Key } from "./locales/en";
import { tr } from "./locales/tr";

export type Lang = "en" | "tr";

export type { Key };

const dicts: Record<Lang, Record<Key, string>> = { en, tr };

export function translate(lang: Lang, key: Key, vars?: Record<string, string | number>): string {
  let s: string = dicts[lang][key] ?? en[key];
  if (!vars) return s;
  // Tekil/çoğul: "{n|item|items}" → n 1 ise "item", değilse "items".
  s = s.replace(/\{(\w+)\|([^|}]*)\|([^}]*)\}/g, (_m, k: string, one: string, many: string) => (Number(vars[k]) === 1 ? one : many));
  for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v));
  return s;
}

export type T = (key: Key, vars?: Record<string, string | number>) => string;

export const I18n = createContext<{ lang: Lang; t: T }>({ lang: "en", t: (k, v) => translate("en", k, v) });

export function useT() {
  return useContext(I18n);
}

const STORE_KEY = "bosun.lang";

export function cachedLang(): Lang {
  try {
    const v = localStorage.getItem(STORE_KEY);
    return v === "tr" ? "tr" : "en";
  } catch {
    return "en";
  }
}

export function applyLang(lang: Lang) {
  setLocale(lang);
  document.documentElement.lang = lang;
  try {
    localStorage.setItem(STORE_KEY, lang);
  } catch {
    /* tarayıcı depolaması yoksa önemli değil */
  }
}
