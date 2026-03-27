import { useStore } from '../hooks/useStore'

export function EventTimelinePage() {
  const events = useStore((s) => s.events)

  const categories = ['session', 'media', 'asr', 'agent', 'tts', 'playback', 'interrupt', 'governance']
  const getCategory = (type: string) => {
    const stripped = type.replace('prx.voice.', '')
    return categories.find((c) => stripped.startsWith(c)) || 'other'
  }

  const categoryColors: Record<string, string> = {
    session: '#8b5cf6',
    media: '#06b6d4',
    asr: '#3b82f6',
    agent: '#f59e0b',
    tts: '#22c55e',
    playback: '#10b981',
    interrupt: '#ef4444',
    governance: '#ec4899',
    other: '#6b7280',
  }

  return (
    <div>
      <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 24 }}>Event Timeline</h1>

      {events.length === 0 ? (
        <div style={{ color: '#71717a', textAlign: 'center', padding: 40 }}>
          No events captured. Start a session from the Session Test page.
        </div>
      ) : (
        <div style={{ position: 'relative', paddingLeft: 40 }}>
          {/* Timeline line */}
          <div style={{ position: 'absolute', left: 18, top: 0, bottom: 0, width: 2, background: '#27272a' }} />

          {events.map((evt, i) => {
            const cat = getCategory(evt.type)
            return (
              <div key={i} style={{ position: 'relative', marginBottom: 12, paddingLeft: 20 }}>
                {/* Dot */}
                <div style={{
                  position: 'absolute', left: -26, top: 6,
                  width: 10, height: 10, borderRadius: '50%',
                  background: categoryColors[cat],
                }} />

                <div style={{ background: '#111118', border: '1px solid #27272a', borderRadius: 8, padding: 12 }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 4 }}>
                    <span style={{ color: categoryColors[cat], fontSize: 13, fontWeight: 600 }}>
                      {evt.type.replace('prx.voice.', '')}
                    </span>
                    <span style={{ color: '#52525b', fontSize: 11 }}>
                      seq={evt.prx_seq} | turn={evt.prx_turn_id}
                    </span>
                  </div>
                  <div style={{ color: '#71717a', fontSize: 11 }}>
                    {new Date(evt.time).toLocaleTimeString()} | {evt.prx_severity}
                  </div>
                  {evt.data && Object.keys(evt.data).length > 0 && (
                    <pre style={{ marginTop: 8, fontSize: 11, color: '#a1a1aa', overflow: 'auto', maxHeight: 100 }}>
                      {JSON.stringify(evt.data, null, 2)}
                    </pre>
                  )}
                </div>
              </div>
            )
          })}
        </div>
      )}
    </div>
  )
}
