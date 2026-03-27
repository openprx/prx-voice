import { StrictMode, useState } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter, Routes, Route } from 'react-router-dom'
import { Layout } from './components/Layout'
import { LoginPage } from './pages/LoginPage'
import { SessionsPage } from './pages/SessionsPage'
import { SessionTestPage } from './pages/SessionTestPage'
import { AudioDebugPage } from './pages/AudioDebugPage'
import { TranscriptDebugPage } from './pages/TranscriptDebugPage'
import { AgentDebugPage } from './pages/AgentDebugPage'
import { TtsDebugPage } from './pages/TtsDebugPage'
import { EventTimelinePage } from './pages/EventTimelinePage'
import { ReplayPage } from './pages/ReplayPage'
import { MetricsPage } from './pages/MetricsPage'
import { AdminPage } from './pages/AdminPage'
import { VoiceClonePage } from './pages/VoiceClonePage'

function App() {
  const [token, setToken] = useState<string | null>(
    localStorage.getItem('prx-token') || 'dev-auto-login'
  )
  const [username, setUsername] = useState(
    localStorage.getItem('prx-user') || 'admin'
  )

  const handleLogin = (newToken: string, user: string) => {
    setToken(newToken)
    setUsername(user)
  }

  const handleLogout = () => {
    setToken(null)
    setUsername('')
    localStorage.removeItem('prx-token')
    localStorage.removeItem('prx-user')
  }

  if (!token) {
    return <LoginPage onLogin={handleLogin} />
  }

  return (
    <BrowserRouter future={{ v7_startTransition: true, v7_relativeSplatPath: true }}>
      <Routes>
        <Route element={<Layout username={username} onLogout={handleLogout} />}>
          <Route path="/" element={<SessionsPage />} />
          <Route path="/test" element={<SessionTestPage />} />
          <Route path="/voice-clone" element={<VoiceClonePage />} />
          <Route path="/audio" element={<AudioDebugPage />} />
          <Route path="/transcript" element={<TranscriptDebugPage />} />
          <Route path="/agent" element={<AgentDebugPage />} />
          <Route path="/tts" element={<TtsDebugPage />} />
          <Route path="/events" element={<EventTimelinePage />} />
          <Route path="/replay" element={<ReplayPage />} />
          <Route path="/metrics" element={<MetricsPage />} />
          <Route path="/admin" element={<AdminPage />} />
        </Route>
      </Routes>
    </BrowserRouter>
  )
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>
)
