import { useState } from 'react'
import { useLocale } from '../hooks/useLocale'

interface LoginProps {
  onLogin: (token: string, username: string) => void
}

// Default credentials for dev mode
const DEV_USERS: Record<string, string> = {
  admin: 'admin123',
  operator: 'operator123',
  viewer: 'viewer123',
}

export function LoginPage({ onLogin }: LoginProps) {
  const { t, locale, switchLocale } = useLocale()
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState('')
  const [remember, setRemember] = useState(false)

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    setError('')

    // Dev mode: check against static users
    if (DEV_USERS[username] === password) {
      const token = btoa(`${username}:${Date.now()}`)
      if (remember) {
        localStorage.setItem('prx-token', token)
        localStorage.setItem('prx-user', username)
      }
      onLogin(token, username)
    } else {
      setError(t('login.error'))
    }
  }

  return (
    <div style={{
      minHeight: '100vh', display: 'flex', alignItems: 'center', justifyContent: 'center',
      background: 'linear-gradient(135deg, #0a0a1a 0%, #1a1a2e 50%, #16213e 100%)',
    }}>
      <div style={{ position: 'absolute', top: 16, right: 16 }}>
        <button
          onClick={() => switchLocale(locale === 'zh' ? 'en' : 'zh')}
          style={{
            padding: '6px 14px', background: '#27272a', color: '#a1a1aa',
            border: 'none', borderRadius: 4, cursor: 'pointer', fontSize: 13,
          }}
        >
          {locale === 'zh' ? 'English' : '中文'}
        </button>
      </div>

      <div style={{
        width: 380, background: '#111118', borderRadius: 12,
        border: '1px solid #27272a', padding: 40, boxShadow: '0 20px 60px rgba(0,0,0,0.5)',
      }}>
        <div style={{ textAlign: 'center', marginBottom: 32 }}>
          <div style={{ fontSize: 28, fontWeight: 800, color: '#a78bfa', marginBottom: 8 }}>
            {t('login.title')}
          </div>
          <div style={{ fontSize: 13, color: '#71717a' }}>
            {t('login.subtitle')}
          </div>
        </div>

        <form onSubmit={handleSubmit}>
          <div style={{ marginBottom: 16 }}>
            <label style={{ display: 'block', fontSize: 12, color: '#71717a', marginBottom: 6 }}>
              {t('login.username')}
            </label>
            <input
              type="text" value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder={locale === 'zh' ? '请输入用户名' : 'Enter username'}
              style={{
                width: '100%', padding: '10px 12px', background: '#0a0a0f',
                border: '1px solid #27272a', borderRadius: 6, color: '#e4e4e7',
                fontSize: 14, outline: 'none', boxSizing: 'border-box',
              }}
            />
          </div>

          <div style={{ marginBottom: 16 }}>
            <label style={{ display: 'block', fontSize: 12, color: '#71717a', marginBottom: 6 }}>
              {t('login.password')}
            </label>
            <input
              type="password" value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder={locale === 'zh' ? '请输入密码' : 'Enter password'}
              style={{
                width: '100%', padding: '10px 12px', background: '#0a0a0f',
                border: '1px solid #27272a', borderRadius: 6, color: '#e4e4e7',
                fontSize: 14, outline: 'none', boxSizing: 'border-box',
              }}
            />
          </div>

          <div style={{ marginBottom: 20, display: 'flex', alignItems: 'center', gap: 8 }}>
            <input
              type="checkbox" checked={remember}
              onChange={(e) => setRemember(e.target.checked)}
              style={{ accentColor: '#7c3aed' }}
            />
            <span style={{ fontSize: 12, color: '#71717a' }}>{t('login.remember')}</span>
          </div>

          {error && (
            <div style={{
              padding: '8px 12px', background: '#ef444422', color: '#ef4444',
              borderRadius: 6, fontSize: 13, marginBottom: 16, textAlign: 'center',
            }}>
              {error}
            </div>
          )}

          <button type="submit" style={{
            width: '100%', padding: '12px', background: '#7c3aed', color: '#fff',
            border: 'none', borderRadius: 6, fontSize: 15, fontWeight: 600,
            cursor: 'pointer',
          }}>
            {t('login.submit')}
          </button>
        </form>

        <div style={{ marginTop: 24, fontSize: 11, color: '#3f3f46', textAlign: 'center' }}>
          {locale === 'zh'
            ? '开发模式：admin/admin123, operator/operator123, viewer/viewer123'
            : 'Dev mode: admin/admin123, operator/operator123, viewer/viewer123'}
        </div>
      </div>
    </div>
  )
}
