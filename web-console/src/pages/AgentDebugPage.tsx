import { useStore } from '../hooks/useStore'

export function AgentDebugPage() {
  const events = useStore((s) => s.events)

  const agentEvents = events.filter((e) => e.type.includes('agent'))
  const thinkingEvents = agentEvents.filter((e) => e.type.includes('thinking_started'))
  const tokenEvents = agentEvents.filter((e) => e.type.includes('token_stream'))
  const completeEvents = agentEvents.filter((e) => e.type.includes('response_complete'))
  const errorEvents = agentEvents.filter((e) => e.type.includes('agent') && e.type.includes('error'))

  return (
    <div>
      <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 24 }}>Agent Debug</h1>

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 16, marginBottom: 24 }}>
        {[
          { label: 'Thinking Events', value: thinkingEvents.length, color: '#f59e0b' },
          { label: 'Token Chunks', value: tokenEvents.length, color: '#3b82f6' },
          { label: 'Completions', value: completeEvents.length, color: '#22c55e' },
          { label: 'Errors', value: errorEvents.length, color: '#ef4444' },
        ].map((stat, i) => (
          <div key={i} style={{ background: '#111118', border: '1px solid #27272a', borderRadius: 8, padding: 16, textAlign: 'center' as const }}>
            <div style={{ fontSize: 28, fontWeight: 700, color: stat.color }}>{stat.value}</div>
            <div style={{ fontSize: 11, color: '#71717a', marginTop: 4 }}>{stat.label}</div>
          </div>
        ))}
      </div>

      <div style={{ background: '#111118', border: '1px solid #27272a', borderRadius: 8, padding: 16, marginBottom: 16 }}>
        <div style={{ fontSize: 12, color: '#71717a', marginBottom: 8 }}>Agent Responses</div>
        {completeEvents.length === 0 ? (
          <div style={{ color: '#3f3f46', fontSize: 13 }}>No responses yet. Start a session.</div>
        ) : (
          completeEvents.map((evt, i) => {
            const data = evt.data as Record<string, unknown>
            return (
              <div key={i} style={{ padding: '12px 0', borderBottom: '1px solid #1e1e2e' }}>
                <div style={{ fontSize: 14, color: '#e4e4e7', marginBottom: 8 }}>{data.response_text as string}</div>
                <div style={{ display: 'flex', gap: 16, fontSize: 11, color: '#52525b' }}>
                  <span>Provider: {data.provider as string || 'unknown'}</span>
                  <span>Model: {data.model as string || 'unknown'}</span>
                  <span>Tokens: {data.total_tokens as number || 0}</span>
                  <span>Latency: {data.first_token_latency_ms as number || 0}ms</span>
                  <span>Finish: {data.finish_reason as string || '-'}</span>
                </div>
              </div>
            )
          })
        )}
      </div>

      <div style={{ background: '#111118', border: '1px solid #27272a', borderRadius: 8, padding: 16 }}>
        <div style={{ fontSize: 12, color: '#71717a', marginBottom: 8 }}>Token Stream (Last 50)</div>
        <div style={{ fontFamily: 'monospace', fontSize: 12, color: '#a1a1aa', lineHeight: 1.8, maxHeight: 200, overflow: 'auto' }}>
          {tokenEvents.length === 0 ? (
            <span style={{ color: '#3f3f46' }}>No tokens streamed yet</span>
          ) : (
            tokenEvents.slice(-50).map((evt, i) => (
              <span key={i} style={{ color: (evt.data as Record<string, unknown>).finish_reason ? '#22c55e' : '#a1a1aa' }}>
                {(evt.data as Record<string, unknown>).token as string}
              </span>
            ))
          )}
        </div>
      </div>
    </div>
  )
}
