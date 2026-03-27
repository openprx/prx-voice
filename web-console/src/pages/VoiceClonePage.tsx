import { useState, useRef, useEffect } from 'react'

interface Speaker {
  id: string
  name: string
  voice_tag: string
  audio_duration_sec: number
  created: string
}

const CLONE_API = '/api/v1/clone'

export function VoiceClonePage() {
  const [speakers, setSpeakers] = useState<Speaker[]>([])
  const [recording, setRecording] = useState(false)
  const [recordTime, setRecordTime] = useState(0)
  const [speakerName, setSpeakerName] = useState('')
  const [voiceTag, setVoiceTag] = useState('zh-female')
  const [status, setStatus] = useState('')

  const audioRef = useRef<{ stream: MediaStream; ctx: AudioContext; processor: ScriptProcessorNode } | null>(null)
  const bufferRef = useRef<Int16Array[]>([])
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)

  // Load speakers on mount
  useEffect(() => { loadSpeakers() }, [])

  const loadSpeakers = async () => {
    try {
      const res = await fetch(`${CLONE_API}/speakers`)
      const data = await res.json()
      setSpeakers(data.speakers || [])
    } catch {
      setSpeakers([])
    }
  }

  const startRecording = async () => {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({
        audio: { sampleRate: 16000, channelCount: 1, echoCancellation: true, noiseSuppression: true },
      })
      const ctx = new AudioContext({ sampleRate: 16000 })
      const source = ctx.createMediaStreamSource(stream)
      const processor = ctx.createScriptProcessor(4096, 1, 1)
      const sink = ctx.createGain()
      sink.gain.value = 0

      bufferRef.current = []

      processor.onaudioprocess = (e) => {
        const float32 = e.inputBuffer.getChannelData(0)
        const int16 = new Int16Array(float32.length)
        for (let i = 0; i < float32.length; i++) {
          int16[i] = Math.max(-32768, Math.min(32767, Math.round(float32[i] * 32767)))
        }
        bufferRef.current.push(int16)
      }

      source.connect(processor)
      processor.connect(sink)
      sink.connect(ctx.destination)

      audioRef.current = { stream, ctx, processor }
      setRecording(true)
      setRecordTime(0)
      setStatus('Recording...')

      timerRef.current = setInterval(() => {
        setRecordTime(t => t + 1)
      }, 1000)
    } catch (e) {
      setStatus(`Mic error: ${e}`)
    }
  }

  const stopRecording = async () => {
    // Stop recording
    if (timerRef.current) clearInterval(timerRef.current)
    const audio = audioRef.current
    if (audio) {
      audio.processor.disconnect()
      audio.ctx.close()
      audio.stream.getTracks().forEach(t => t.stop())
    }
    audioRef.current = null
    setRecording(false)

    // Merge audio buffers
    const chunks = bufferRef.current
    if (chunks.length === 0) {
      setStatus('No audio recorded')
      return
    }

    const totalLen = chunks.reduce((sum, c) => sum + c.length, 0)
    const merged = new Int16Array(totalLen)
    let offset = 0
    for (const chunk of chunks) {
      merged.set(chunk, offset)
      offset += chunk.length
    }
    bufferRef.current = []

    const durationSec = totalLen / 16000
    if (durationSec < 3) {
      setStatus(`Too short (${durationSec.toFixed(1)}s). Need at least 3 seconds.`)
      return
    }

    setStatus(`Uploading ${durationSec.toFixed(1)}s audio...`)

    // Upload to server
    try {
      const pcmBytes = new Uint8Array(merged.buffer)
      const res = await fetch(`${CLONE_API}/speakers`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/octet-stream',
          'X-Speaker-Name': speakerName || `Voice-${Date.now().toString(36)}`,
          'X-Voice-Tag': voiceTag,
        },
        body: pcmBytes,
      })
      const data = await res.json()
      if (data.error) {
        setStatus(`Error: ${data.error}`)
      } else {
        setStatus(`Speaker "${data.name}" created! (${data.audio_duration_sec.toFixed(1)}s, dim=${data.embedding_dim})`)
        setSpeakerName('')
        loadSpeakers()
      }
    } catch (e) {
      setStatus(`Upload failed: ${e}`)
    }
  }

  const deleteSpeaker = async (id: string) => {
    try {
      await fetch(`${CLONE_API}/speakers/${id}`, { method: 'DELETE' })
      loadSpeakers()
    } catch { /* ignore */ }
  }

  const playRecording = async (id: string) => {
    setStatus('Playing recording...')
    try {
      const res = await fetch(`${CLONE_API}/speakers/${id}/audio`)
      if (!res.ok) { setStatus('Audio not found'); return }
      const audioData = await res.arrayBuffer()
      const int16 = new Int16Array(audioData)
      const float32 = new Float32Array(int16.length)
      for (let i = 0; i < int16.length; i++) float32[i] = int16[i] / 32768.0
      const ctx = new AudioContext({ sampleRate: 16000 })
      const buf = ctx.createBuffer(1, float32.length, 16000)
      buf.copyToChannel(float32, 0)
      const src = ctx.createBufferSource()
      src.buffer = buf
      src.connect(ctx.destination)
      src.start()
      src.onended = () => { ctx.close(); setStatus('Playback done') }
    } catch (e) { setStatus(`Play failed: ${e}`) }
  }

  const testSpeak = async (id: string) => {
    setStatus('Generating cloned voice...')
    try {
      const res = await fetch(`${CLONE_API}/tts`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ text: '你好，这是声纹克隆的测试语音。很高兴认识你。', speaker: id, lang: 'zh' }),
      })
      if (!res.ok) {
        setStatus('TTS failed')
        return
      }
      const audioData = await res.arrayBuffer()
      const int16 = new Int16Array(audioData)
      const float32 = new Float32Array(int16.length)
      for (let i = 0; i < int16.length; i++) float32[i] = int16[i] / 32768.0

      const ctx = new AudioContext({ sampleRate: 16000 })
      const buf = ctx.createBuffer(1, float32.length, 16000)
      buf.copyToChannel(float32, 0)
      const src = ctx.createBufferSource()
      src.buffer = buf
      src.connect(ctx.destination)
      src.start()
      src.onended = () => { ctx.close(); setStatus('Test complete') }
      setStatus('Playing test audio...')
    } catch (e) {
      setStatus(`Test failed: ${e}`)
    }
  }

  return (
    <div>
      <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 8 }}>Voice Clone</h1>
      <p style={{ color: '#71717a', fontSize: 13, marginBottom: 20 }}>
        Record your voice for 5-10 seconds. The system will extract your voice print and use it for TTS.
      </p>

      {/* Recording section */}
      <div style={{
        background: '#111118', border: '1px solid #27272a', borderRadius: 12,
        padding: 24, marginBottom: 16,
      }}>
        <div style={{ display: 'flex', gap: 12, alignItems: 'flex-end', marginBottom: 16, flexWrap: 'wrap' }}>
          <div>
            <label style={{ fontSize: 10, color: '#52525b', display: 'block', marginBottom: 2 }}>Name</label>
            <input
              value={speakerName}
              onChange={e => setSpeakerName(e.target.value)}
              placeholder="My Voice"
              disabled={recording}
              style={{
                padding: '6px 10px', background: '#1e1e2e', color: '#e4e4e7',
                border: '1px solid #27272a', borderRadius: 4, fontSize: 13, width: 140,
              }}
            />
          </div>
          <div>
            <label style={{ fontSize: 10, color: '#52525b', display: 'block', marginBottom: 2 }}>Closest voice</label>
            <select
              value={voiceTag}
              onChange={e => setVoiceTag(e.target.value)}
              disabled={recording}
              style={{
                padding: '6px 10px', background: '#1e1e2e', color: '#a1a1aa',
                border: '1px solid #27272a', borderRadius: 4, fontSize: 11,
              }}
            >
              <option value="zh-female">Chinese Female</option>
              <option value="zh-male">Chinese Male</option>
              <option value="en-female">English Female</option>
              <option value="en-male">English Male</option>
            </select>
          </div>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
          {!recording ? (
            <button onClick={startRecording} style={{
              padding: '12px 32px', background: '#ef4444', color: '#fff', border: 'none',
              borderRadius: 30, cursor: 'pointer', fontWeight: 700, fontSize: 15,
            }}>
              Start Recording
            </button>
          ) : (
            <button onClick={stopRecording} style={{
              padding: '12px 32px', background: '#27272a', color: '#ef4444', border: '2px solid #ef4444',
              borderRadius: 30, cursor: 'pointer', fontWeight: 700, fontSize: 15,
              animation: 'pulse-glow-red 1.5s ease-in-out infinite',
            }}>
              Stop ({recordTime}s)
            </button>
          )}

          {recording && (
            <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
              <div style={{
                width: 10, height: 10, borderRadius: '50%', background: '#ef4444',
                animation: 'pulse 1s ease-in-out infinite',
              }} />
              <span style={{ fontSize: 13, color: '#ef4444' }}>
                {recordTime < 3 ? `Keep talking... (${3 - recordTime}s more)` :
                 recordTime < 10 ? `Good! (${recordTime}s)` : `Great! You can stop now.`}
              </span>
            </div>
          )}
        </div>

        {status && (
          <div style={{ marginTop: 12, fontSize: 12, color: '#a1a1aa' }}>{status}</div>
        )}
      </div>

      {/* Speaker list */}
      <div style={{
        background: '#111118', border: '1px solid #27272a', borderRadius: 12, padding: 20,
      }}>
        <h2 style={{ fontSize: 16, fontWeight: 600, marginBottom: 12 }}>
          Saved Voices ({speakers.length})
        </h2>

        {speakers.length === 0 ? (
          <div style={{ color: '#3f3f46', fontSize: 13, padding: '20px 0', textAlign: 'center' }}>
            No voices yet. Record one above.
          </div>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            {speakers.map(s => (
              <div key={s.id} style={{
                display: 'flex', alignItems: 'center', justifyContent: 'space-between',
                padding: '10px 14px', background: '#1e1e2e', borderRadius: 8,
              }}>
                <div>
                  <div style={{ fontWeight: 600, fontSize: 14, color: '#e4e4e7' }}>{s.name}</div>
                  <div style={{ fontSize: 11, color: '#52525b' }}>
                    {s.voice_tag} · {s.audio_duration_sec.toFixed(1)}s · {s.created}
                  </div>
                </div>
                <div style={{ display: 'flex', gap: 6 }}>
                  <button onClick={() => playRecording(s.id)} style={{
                    padding: '4px 12px', background: '#06b6d4', color: '#fff', border: 'none',
                    borderRadius: 4, cursor: 'pointer', fontSize: 11, fontWeight: 600,
                  }}>
                    Play
                  </button>
                  <button onClick={() => testSpeak(s.id)} style={{
                    padding: '4px 12px', background: '#7c3aed', color: '#fff', border: 'none',
                    borderRadius: 4, cursor: 'pointer', fontSize: 11, fontWeight: 600,
                  }}>
                    Clone Test
                  </button>
                  <button onClick={() => deleteSpeaker(s.id)} style={{
                    padding: '4px 12px', background: 'transparent', color: '#ef4444',
                    border: '1px solid #ef4444', borderRadius: 4, cursor: 'pointer', fontSize: 11,
                  }}>
                    Delete
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}

        <div style={{ marginTop: 16, fontSize: 11, color: '#3f3f46', lineHeight: 1.6 }}>
          Tip: Go to Session Test page → enable "Clone Voice" toggle → select a saved voice.
        </div>
      </div>

      <style>{`
        @keyframes pulse { 0%, 100% { opacity: 0.3; } 50% { opacity: 1; } }
        @keyframes pulse-glow-red { 0%, 100% { box-shadow: 0 0 4px #ef4444; } 50% { box-shadow: 0 0 16px #ef4444; } }
      `}</style>
    </div>
  )
}
