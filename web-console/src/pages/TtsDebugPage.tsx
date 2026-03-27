import { useStore } from '../hooks/useStore'

export function TtsDebugPage() {
  const events = useStore((s) => s.events)

  const ttsEvents = events.filter((e) => e.type.includes('tts') || e.type.includes('playback'))
  const queuedEvents = ttsEvents.filter((e) => e.type.includes('segment_queued'))
  const chunkEvents = ttsEvents.filter((e) => e.type.includes('chunk_ready'))
  const playbackEvents = ttsEvents.filter((e) => e.type.includes('playback'))
  const stopEvents = ttsEvents.filter((e) => e.type.includes('stopped'))

  return (
    <div>
      <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 24 }}>TTS Debug</h1>

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 16, marginBottom: 24 }}>
        {[
          { label: 'Segments Queued', value: queuedEvents.length, color: '#8b5cf6' },
          { label: 'Chunks Ready', value: chunkEvents.length, color: '#3b82f6' },
          { label: 'Playback Events', value: playbackEvents.length, color: '#22c55e' },
          { label: 'Stops/Flushes', value: stopEvents.length, color: '#f59e0b' },
        ].map((stat, i) => (
          <div key={i} style={{ background: '#111118', border: '1px solid #27272a', borderRadius: 8, padding: 16, textAlign: 'center' as const }}>
            <div style={{ fontSize: 28, fontWeight: 700, color: stat.color }}>{stat.value}</div>
            <div style={{ fontSize: 11, color: '#71717a', marginTop: 4 }}>{stat.label}</div>
          </div>
        ))}
      </div>

      <div style={{ background: '#111118', border: '1px solid #27272a', borderRadius: 8, padding: 16, marginBottom: 16 }}>
        <div style={{ fontSize: 12, color: '#71717a', marginBottom: 8 }}>Segment Queue</div>
        {queuedEvents.length === 0 ? (
          <div style={{ color: '#3f3f46', fontSize: 13 }}>No segments queued</div>
        ) : (
          <table style={{ width: '100%', fontSize: 12 }}>
            <thead>
              <tr style={{ color: '#71717a', borderBottom: '1px solid #27272a' }}>
                <th style={{ padding: 8, textAlign: 'left' }}>Segment</th>
                <th style={{ padding: 8, textAlign: 'left' }}>Text</th>
                <th style={{ padding: 8, textAlign: 'left' }}>Provider</th>
                <th style={{ padding: 8, textAlign: 'left' }}>Voice</th>
                <th style={{ padding: 8, textAlign: 'left' }}>Est. Duration</th>
              </tr>
            </thead>
            <tbody>
              {queuedEvents.map((evt, i) => {
                const d = evt.data as Record<string, unknown>
                return (
                  <tr key={i} style={{ borderBottom: '1px solid #1e1e2e' }}>
                    <td style={{ padding: 8, fontFamily: 'monospace' }}>{d.segment_id as string}</td>
                    <td style={{ padding: 8, maxWidth: 300, overflow: 'hidden', textOverflow: 'ellipsis' as const, whiteSpace: 'nowrap' as const }}>{d.text as string}</td>
                    <td style={{ padding: 8 }}>{d.provider as string}</td>
                    <td style={{ padding: 8 }}>{d.voice as string}</td>
                    <td style={{ padding: 8 }}>{d.estimated_duration_ms as number}ms</td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        )}
      </div>

      <div style={{ background: '#111118', border: '1px solid #27272a', borderRadius: 8, padding: 16 }}>
        <div style={{ fontSize: 12, color: '#71717a', marginBottom: 8 }}>Playback Timeline</div>
        {playbackEvents.length === 0 ? (
          <div style={{ color: '#3f3f46', fontSize: 13 }}>No playback events</div>
        ) : (
          playbackEvents.map((evt, i) => {
            const d = evt.data as Record<string, unknown>
            const isStart = evt.type.includes('started')
            return (
              <div key={i} style={{ padding: '6px 0', borderBottom: '1px solid #1e1e2e', fontSize: 12 }}>
                <span style={{ color: isStart ? '#22c55e' : '#3b82f6', marginRight: 8 }}>
                  {isStart ? 'PLAY' : 'DONE'}
                </span>
                <span style={{ fontFamily: 'monospace', marginRight: 8 }}>{d.segment_id as string}</span>
                {d.duration_ms != null && <span style={{ color: '#71717a' }}>{`${d.duration_ms}`}ms</span>}
                {d.first_byte_latency_ms != null && <span style={{ color: '#f59e0b', marginLeft: 8 }}>FBL: {`${d.first_byte_latency_ms}`}ms</span>}
              </div>
            )
          })
        )}
      </div>
    </div>
  )
}
