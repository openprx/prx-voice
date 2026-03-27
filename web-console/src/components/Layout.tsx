import { Link, Outlet, useLocation } from 'react-router-dom'
import { useLocale } from '../hooks/useLocale'

interface LayoutProps {
  username: string
  onLogout: () => void
}

export function Layout({ username, onLogout }: LayoutProps) {
  const location = useLocation()
  const { t, locale, switchLocale } = useLocale()

  const navItems = [
    { path: '/', label: t('nav.sessions') },
    { path: '/test', label: t('nav.test') },
    { path: '/voice-clone', label: 'Voice Clone' },
    { path: '/audio', label: t('nav.audio') },
    { path: '/transcript', label: t('nav.transcript') },
    { path: '/agent', label: t('nav.agent') },
    { path: '/tts', label: t('nav.tts') },
    { path: '/events', label: t('nav.events') },
    { path: '/replay', label: t('nav.replay') },
    { path: '/metrics', label: t('nav.metrics') },
    { path: '/admin', label: t('nav.admin') },
  ]

  return (
    <div style={{ display: 'flex', minHeight: '100vh' }}>
      <nav style={{
        width: 220, background: '#111118', borderRight: '1px solid #27272a',
        padding: '20px 0', display: 'flex', flexDirection: 'column',
      }}>
        <div style={{ padding: '0 16px 24px', fontSize: 18, fontWeight: 700, color: '#a78bfa' }}>
          PRX Voice
        </div>

        <div style={{ flex: 1 }}>
          {navItems.map((item) => (
            <Link
              key={item.path} to={item.path}
              style={{
                display: 'block', padding: '10px 16px',
                color: location.pathname === item.path ? '#a78bfa' : '#a1a1aa',
                textDecoration: 'none',
                background: location.pathname === item.path ? '#1e1e2e' : 'transparent',
                borderLeft: location.pathname === item.path ? '3px solid #a78bfa' : '3px solid transparent',
                fontSize: 13,
              }}
            >
              {item.label}
            </Link>
          ))}
        </div>

        {/* Bottom: user info, language switch, logout */}
        <div style={{ borderTop: '1px solid #27272a', padding: '12px 16px' }}>
          <div style={{ fontSize: 12, color: '#71717a', marginBottom: 8 }}>
            {username}
          </div>
          <div style={{ display: 'flex', gap: 8 }}>
            <button
              onClick={() => switchLocale(locale === 'zh' ? 'en' : 'zh')}
              style={{
                flex: 1, padding: '6px', background: '#27272a', color: '#a1a1aa',
                border: 'none', borderRadius: 4, cursor: 'pointer', fontSize: 11,
              }}
            >
              {locale === 'zh' ? 'EN' : '中文'}
            </button>
            <button
              onClick={onLogout}
              style={{
                flex: 1, padding: '6px', background: '#27272a', color: '#ef4444',
                border: 'none', borderRadius: 4, cursor: 'pointer', fontSize: 11,
              }}
            >
              {t('logout')}
            </button>
          </div>
        </div>
      </nav>
      <main style={{ flex: 1, padding: 24 }}>
        <Outlet />
      </main>
    </div>
  )
}
