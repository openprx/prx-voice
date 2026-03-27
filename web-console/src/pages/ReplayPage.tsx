import { useState } from 'react'
import { useStore } from '../hooks/useStore'

export function ReplayPage() {
  const events = useStore((s) => s.events)
  const [playing, setPlaying] = useState(false)
  const [currentIdx, setCurrentIdx] = useState(0)
  const [speed, setSpeed] = useState(1)

  const startReplay = () => {
    if (events.length === 0) return
    setPlaying(true)
    setCurrentIdx(0)
  }

  const stopReplay = () => {
    setPlaying(false)
  }

  const visibleEvents = events.slice(0, playing ? currentIdx + 1 : events.length)

  const categoryIcon = (type: string) => {
    if (type.includes('session')) return 'S'
    if (type.includes('asr') || type.includes('vad')) return 'A'
    if (type.includes('agent')) return 'G'
    if (type.includes('tts')) return 'T'
    if (type.includes('playback')) return 'P'
    if (type.includes('interrupt')) return '!'
    return '?'
  }

  return (
    <div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 24 }}>
        <h1 style={{ fontSize: 24, fontWeight: 700 }}>Session Replay</h1>
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          <select value={speed} onChange={(e) => setSpeed(Number(e.target.value))} style={{ padding: '6px 10px', background: '#1e1e2e', color: '#e4e4e7', border: '1px solid #27272a', borderRadius: 4, fontSize: 12 }}>
            <option value={0.5}>0.5x</option>
            <option value={1}>1x</option>
            <option value={2}>2x</option>
            <option value={5}>5x</option>
          </select>
          {!playing ? (
            <button onClick={startReplay} style={{ padding: '8px 20px', background: '#22c55e', color: '#fff', border: 'none', borderRadius: 6, cursor: 'pointer', fontSize: 13 }}>
              Replay
            </button>
          ) : (
            <button onClick={stopReplay} style={{ padding: '8px 20px', background: '#ef4444', color: '#fff', border: 'none', borderRadius: 6, cursor: 'pointer', fontSize: 13 }}>
              Stop
            </button>
          )}
        </div>
      </div>

      {events.length === 0 ? (
        <div style={{ textAlign: 'center', padding: 60, color: '#3f3f46' }}>
          <div style={{ fontSize: 48, marginBottom: 16 }}>No events to replay</div>
          <div>Run a session from the Session Test page first.</div>
        </div>
      ) : (
        <div>
          <div style={{ marginBottom: 16, fontSize: 12, color: '#71717a' }}>
            {visibleEvents.length} / {events.length} events | Speed: {speed}x
          </div>
          <div style={{ display: 'flex', flexDirection: 'column' as const, gap: 4 }}>
            {visibleEvents.map((evt, i) => (
              <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '6px 12px', background: i === currentIdx && playing ? '#1e1e3e' : '#111118', borderRadius: 6, border: '1px solid #1e1e2e' }}>
                <div style={{ width: 28, height: 28, borderRadius: '50%', background: '#27272a', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 12, fontWeight: 700, flexShrink: 0 }}>
                  {categoryIcon(evt.type)}
                </div>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 13, color: '#e4e4e7' }}>{evt.type.replace('prx.voice.', '')}</div>
                  <div style={{ fontSize: 11, color: '#52525b' }}>seq={evt.prx_seq} turn={evt.prx_turn_id}</div>
                </div>
                <div style={{ fontSize: 11, color: '#52525b', flexShrink: 0 }}>{new Date(evt.time).toLocaleTimeString()}</div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}
