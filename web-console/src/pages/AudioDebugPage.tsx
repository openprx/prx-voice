import { useState } from 'react'

interface AudioStats {
  sampleRate: number
  channels: number
  encoding: string
  inputLevel: number
  outputLevel: number
  vadActive: boolean
  droppedFrames: number
  bufferDepthMs: number
}

export function AudioDebugPage() {
  const [stats] = useState<AudioStats>({
    sampleRate: 16000, channels: 1, encoding: 'PCM16',
    inputLevel: -28.5, outputLevel: -22.0, vadActive: false,
    droppedFrames: 0, bufferDepthMs: 200,
  })

  const levelBar = (db: number, label: string, color: string) => {
    const pct = Math.max(0, Math.min(100, ((db + 60) / 60) * 100))
    return (
      <div style={{ marginBottom: 16 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12, color: '#71717a', marginBottom: 4 }}>
          <span>{label}</span>
          <span>{db.toFixed(1)} dB</span>
        </div>
        <div style={{ height: 8, background: '#1e1e2e', borderRadius: 4, overflow: 'hidden' }}>
          <div style={{ height: '100%', width: `${pct}%`, background: color, borderRadius: 4, transition: 'width 0.1s' }} />
        </div>
      </div>
    )
  }

  return (
    <div>
      <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 24 }}>Audio Debug</h1>

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))', gap: 16, marginBottom: 24 }}>
        {[
          { label: 'Sample Rate', value: `${stats.sampleRate} Hz` },
          { label: 'Channels', value: stats.channels.toString() },
          { label: 'Encoding', value: stats.encoding },
          { label: 'VAD Active', value: stats.vadActive ? 'YES' : 'No' },
          { label: 'Dropped Frames', value: stats.droppedFrames.toString() },
          { label: 'Buffer Depth', value: `${stats.bufferDepthMs} ms` },
        ].map((item, i) => (
          <div key={i} style={{ background: '#111118', border: '1px solid #27272a', borderRadius: 8, padding: 16 }}>
            <div style={{ fontSize: 11, color: '#71717a', textTransform: 'uppercase' as const, marginBottom: 4 }}>{item.label}</div>
            <div style={{ fontSize: 20, fontWeight: 600 }}>{item.value}</div>
          </div>
        ))}
      </div>

      {levelBar(stats.inputLevel, 'Input Level (Microphone)', '#22c55e')}
      {levelBar(stats.outputLevel, 'Output Level (Playback)', '#3b82f6')}

      <div style={{ background: '#111118', border: '1px solid #27272a', borderRadius: 8, padding: 16, marginTop: 24 }}>
        <div style={{ fontSize: 12, color: '#71717a', marginBottom: 12 }}>Waveform (Audio Input)</div>
        <canvas id="waveform" style={{ width: '100%', height: 120, background: '#0a0a0f', borderRadius: 4 }} />
        <div style={{ fontSize: 11, color: '#3f3f46', marginTop: 8 }}>
          Connect to a session to see live audio waveform. VAD trigger points shown as vertical markers.
        </div>
      </div>
    </div>
  )
}
