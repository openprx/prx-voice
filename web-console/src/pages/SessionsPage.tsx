import { useEffect, useState } from 'react'
import { api } from '../api/client'
import type { SessionInfo } from '../types/api'

const stateColors: Record<string, string> = {
  Listening: '#22c55e',
  Speaking: '#3b82f6',
  Thinking: '#f59e0b',
  Interrupted: '#ef4444',
  Closed: '#6b7280',
  Failed: '#dc2626',
  Connecting: '#8b5cf6',
  UserSpeaking: '#06b6d4',
  AsrProcessing: '#f97316',
  Paused: '#a3a3a3',
  HandoffPending: '#ec4899',
}

export function SessionsPage() {
  const [sessions, setSessions] = useState<SessionInfo[]>([])
  const [loading, setLoading] = useState(true)

  const load = async () => {
    try {
      const data = await api.listSessions()
      setSessions(data.items)
    } catch (e) {
      console.error('Failed to load sessions:', e)
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => { load() }, [])

  const createSession = async () => {
    try {
      await api.createSession({ language: 'en-US' })
      load()
    } catch (e) {
      console.error('Failed to create session:', e)
    }
  }

  return (
    <div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 24 }}>
        <h1 style={{ fontSize: 24, fontWeight: 700 }}>Sessions</h1>
        <button
          onClick={createSession}
          style={{
            padding: '8px 20px',
            background: '#7c3aed',
            color: '#fff',
            border: 'none',
            borderRadius: 6,
            cursor: 'pointer',
            fontSize: 14,
          }}
        >
          + New Session
        </button>
      </div>

      {loading ? (
        <div style={{ color: '#71717a' }}>Loading...</div>
      ) : sessions.length === 0 ? (
        <div style={{ color: '#71717a', textAlign: 'center', padding: 40 }}>No active sessions</div>
      ) : (
        <table style={{ width: '100%', borderCollapse: 'collapse' }}>
          <thead>
            <tr style={{ borderBottom: '1px solid #27272a', color: '#71717a', fontSize: 12, textTransform: 'uppercase' as const }}>
              <th style={{ padding: '12px 16px', textAlign: 'left' }}>Session ID</th>
              <th style={{ padding: '12px 16px', textAlign: 'left' }}>State</th>
              <th style={{ padding: '12px 16px', textAlign: 'left' }}>Channel</th>
              <th style={{ padding: '12px 16px', textAlign: 'left' }}>Language</th>
              <th style={{ padding: '12px 16px', textAlign: 'left' }}>Turn</th>
              <th style={{ padding: '12px 16px', textAlign: 'left' }}>Actions</th>
            </tr>
          </thead>
          <tbody>
            {sessions.map((s) => (
              <tr key={s.session_id} style={{ borderBottom: '1px solid #1e1e2e' }}>
                <td style={{ padding: '12px 16px', fontFamily: 'monospace', fontSize: 13 }}>{s.session_id}</td>
                <td style={{ padding: '12px 16px' }}>
                  <span style={{
                    padding: '2px 8px',
                    borderRadius: 4,
                    fontSize: 12,
                    background: `${stateColors[s.state] || '#6b7280'}22`,
                    color: stateColors[s.state] || '#6b7280',
                  }}>
                    {s.state}
                  </span>
                </td>
                <td style={{ padding: '12px 16px', fontSize: 13 }}>{s.channel}</td>
                <td style={{ padding: '12px 16px', fontSize: 13 }}>{s.language}</td>
                <td style={{ padding: '12px 16px', fontSize: 13 }}>{s.current_turn_id}</td>
                <td style={{ padding: '12px 16px' }}>
                  <button
                    onClick={() => api.closeSession(s.session_id).then(load)}
                    style={{ padding: '4px 12px', background: '#27272a', color: '#e4e4e7', border: 'none', borderRadius: 4, cursor: 'pointer', fontSize: 12, marginRight: 8 }}
                  >
                    Close
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  )
}
