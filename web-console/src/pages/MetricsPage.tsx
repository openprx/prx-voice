import { useEffect, useState } from 'react'
import { api } from '../api/client'

export function MetricsPage() {
  const [metricsText, setMetricsText] = useState('')
  const [loading, setLoading] = useState(true)

  const load = async () => {
    try {
      const text = await api.metrics()
      setMetricsText(text)
    } catch (e) {
      console.error('Failed to load metrics:', e)
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => { load() }, [])

  // Parse prometheus text format into sections
  const sections = metricsText.split('\n').reduce<{ type: string; name: string; value: string }[]>((acc, line) => {
    if (line.startsWith('# TYPE')) return acc
    if (line.trim() && !line.startsWith('#')) {
      const parts = line.split(' ')
      if (parts.length >= 2) {
        acc.push({ type: 'metric', name: parts[0], value: parts[1] })
      }
    }
    return acc
  }, [])

  return (
    <div>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 24 }}>
        <h1 style={{ fontSize: 24, fontWeight: 700 }}>Metrics</h1>
        <button onClick={load} style={{ padding: '6px 16px', background: '#27272a', color: '#e4e4e7', border: 'none', borderRadius: 4, cursor: 'pointer', fontSize: 13 }}>
          Refresh
        </button>
      </div>

      {loading ? (
        <div style={{ color: '#71717a' }}>Loading...</div>
      ) : sections.length === 0 ? (
        <div style={{ color: '#71717a', textAlign: 'center', padding: 40 }}>No metrics available yet. Create some sessions first.</div>
      ) : (
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))', gap: 16 }}>
          {sections.map((m, i) => (
            <div key={i} style={{ background: '#111118', border: '1px solid #27272a', borderRadius: 8, padding: 16 }}>
              <div style={{ fontSize: 12, color: '#71717a', marginBottom: 8, wordBreak: 'break-all' as const }}>{m.name}</div>
              <div style={{ fontSize: 28, fontWeight: 700, color: '#e4e4e7' }}>{m.value}</div>
            </div>
          ))}
        </div>
      )}

      {/* Raw output */}
      <details style={{ marginTop: 24 }}>
        <summary style={{ color: '#71717a', cursor: 'pointer', fontSize: 13 }}>Raw Prometheus Output</summary>
        <pre style={{ marginTop: 8, padding: 16, background: '#111118', borderRadius: 8, fontSize: 12, color: '#a1a1aa', overflow: 'auto' }}>
          {metricsText || '(empty)'}
        </pre>
      </details>
    </div>
  )
}
