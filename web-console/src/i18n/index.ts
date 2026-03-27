import { zh } from './zh'
import { en } from './en'

export type Locale = 'zh' | 'en'

const messages: Record<Locale, Record<string, string>> = { zh, en }

let currentLocale: Locale = (localStorage.getItem('prx-locale') as Locale) || 'zh'

export function setLocale(locale: Locale) {
  currentLocale = locale
  localStorage.setItem('prx-locale', locale)
}

export function getLocale(): Locale {
  return currentLocale
}

export function t(key: string): string {
  return messages[currentLocale]?.[key] || messages['en']?.[key] || key
}
