import { useStore } from '../hooks/useStore'

export function TranscriptDebugPage() {
  const events = useStore((s) => s.events)

  const asrEvents = events.filter((e) => e.type.includes('asr') || e.type.includes('media.vad'))
  const partials = asrEvents.filter((e) => e.type.includes('transcript_partial'))
  const finals = asrEvents.filter((e) => e.type.includes('transcript_final'))
  const vadEvents = asrEvents.filter((e) => e.type.includes('vad'))

  return (
    <div>
      <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 24 }}>Transcript Debug</h1>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16, marginBottom: 24 }}>
        <div style={{ background: '#111118', border: '1px solid #27272a', borderRadius: 8, padding: 16 }}>
          <div style={{ fontSize: 12, color: '#71717a', marginBottom: 8 }}>Final Transcripts</div>
          {finals.length === 0 ? (
            <div style={{ color: '#3f3f46', fontSize: 13 }}>No final transcripts yet</div>
          ) : (
            finals.map((evt, i) => (
              <div key={i} style={{ padding: '8px 0', borderBottom: '1px solid #1e1e2e' }}>
                <div style={{ fontSize: 14, color: '#e4e4e7' }}>
                  {(evt.data as Record<string, unknown>).transcript as string || '(empty)'}
                </div>
                <div style={{ fontSize: 11, color: '#52525b', marginTop: 4 }}>
                  Turn {evt.prx_turn_id} | Confidence: {((evt.data as Record<string, unknown>).confidence as number || 0).toFixed(2)} | {new Date(evt.time).toLocaleTimeString()}
                </div>
              </div>
            ))
          )}
        </div>

        <div style={{ background: '#111118', border: '1px solid #27272a', borderRadius: 8, padding: 16 }}>
          <div style={{ fontSize: 12, color: '#71717a', marginBottom: 8 }}>Partial Transcripts (Revisions)</div>
          {partials.length === 0 ? (
            <div style={{ color: '#3f3f46', fontSize: 13 }}>No partials captured</div>
          ) : (
            partials.slice(-20).map((evt, i) => (
              <div key={i} style={{ padding: '4px 0', fontSize: 12, color: '#a1a1aa' }}>
                <span style={{ color: '#71717a', marginRight: 8 }}>rev {(evt.data as Record<string, unknown>).revision as number}</span>
                {(evt.data as Record<string, unknown>).transcript as string}
                <span style={{ color: '#52525b', marginLeft: 8 }}>
                  stability: {((evt.data as Record<string, unknown>).stability as number || 0).toFixed(2)}
                </span>
              </div>
            ))
          )}
        </div>
      </div>

      <div style={{ background: '#111118', border: '1px solid #27272a', borderRadius: 8, padding: 16 }}>
        <div style={{ fontSize: 12, color: '#71717a', marginBottom: 8 }}>VAD Events</div>
        {vadEvents.length === 0 ? (
          <div style={{ color: '#3f3f46', fontSize: 13 }}>No VAD events captured</div>
        ) : (
          <div style={{ display: 'flex', flexWrap: 'wrap' as const, gap: 8 }}>
            {vadEvents.map((evt, i) => (
              <span key={i} style={{
                padding: '4px 10px', borderRadius: 4, fontSize: 11,
                background: evt.type.includes('started') ? '#22c55e22' : '#ef444422',
                color: evt.type.includes('started') ? '#22c55e' : '#ef4444',
              }}>
                {evt.type.includes('started') ? 'Speech Start' : 'Speech End'} @ {new Date(evt.time).toLocaleTimeString()}
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
