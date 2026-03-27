import { useState, useCallback } from 'react'
import { getLocale, setLocale as setI18nLocale, t as translate, type Locale } from '../i18n'

export function useLocale() {
  const [locale, setLocaleState] = useState<Locale>(getLocale())

  const switchLocale = useCallback((newLocale: Locale) => {
    setI18nLocale(newLocale)
    setLocaleState(newLocale)
  }, [])

  const t = useCallback((key: string) => translate(key), [locale])

  return { locale, switchLocale, t }
}
