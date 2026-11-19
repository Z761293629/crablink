// English and Simplified Chinese: the browser's language by default, or the one
// picked with the switcher, remembered in this browser.

import i18n, { type TFunction } from 'i18next';
import { initReactI18next } from 'react-i18next';
import { ApiError } from './api';
import en from './locales/en.json';
import zhCN from './locales/zh-CN.json';

export const languages = { en: 'English', 'zh-CN': '简体中文' };
type Language = keyof typeof languages;

const STORAGE_KEY = 'crablink.language';
// Fails to typecheck when zh-CN lacks a key en has.
const zh: typeof en = zhCN;

function stored(): string | null {
  try {
    return localStorage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
}

function initial(): Language {
  const lang = stored();
  if (lang && lang in languages) return lang as Language;
  return navigator.language.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en';
}

i18n.on('languageChanged', (lng) => {
  document.documentElement.lang = lng;
});

void i18n.use(initReactI18next).init({
  resources: { en: { translation: en }, 'zh-CN': { translation: zh } },
  lng: initial(),
  fallbackLng: 'en',
  interpolation: { escapeValue: false },
});

export function setLanguage(lng: string) {
  try {
    localStorage.setItem(STORAGE_KEY, lng);
  } catch {
    // Not remembered, but still switched.
  }
  void i18n.changeLanguage(lng);
}

/** An error in the UI's language: an API error by its `kind`, or its English message for a kind this UI doesn't know. */
export function errorMessage(t: TFunction, err: unknown): string {
  if (err instanceof ApiError) return t(`errors.${err.kind}`, { defaultValue: err.message });
  return t('errors.network');
}
