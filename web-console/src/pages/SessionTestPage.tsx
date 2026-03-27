import { useState, useEffect, useRef, useCallback } from 'react'
import { api } from '../api/client'
import { useLocale } from '../hooks/useLocale'
import type { SessionInfo } from '../types/api'

interface ChatMessage {
  role: 'user' | 'assistant' | 'system'
  content: string
  timestamp: string
}

interface WsMsg {
  type: string
  [key: string]: unknown
}

// Simple browser-side VAD: detect silence to auto-end turn
const SILENCE_THRESHOLD = 0.01  // RMS below this = silence
const SILENCE_DURATION_MS = 1200 // 1.2s silence = end of speech
const SPEECH_MIN_MS = 300 // minimum speech duration before accepting

export function SessionTestPage() {
  const { t: _t } = useLocale()
  const [_session, setSession] = useState<SessionInfo | null>(null)
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [events, setEvents] = useState<WsMsg[]>([])
  const [phase, setPhase] = useState<'idle' | 'listening' | 'speaking' | 'thinking' | 'playing'>('idle')
  const [_wsReady, setWsReady] = useState(false)
  const [volume, setVolume] = useState(0)
  const [providers, setProviders] = useState({ asr: 'sherpa', agent: 'ollama', tts: 'sherpa' })
  const [translateMode, setTranslateMode] = useState(false)
  const translateRef = useRef(false)
  const [cloneMode, setCloneMode] = useState(false)
  const cloneRef = useRef(false)
  const [cloneSpeaker, setCloneSpeaker] = useState('')
  const cloneSpeakerRef = useRef('')
  const [savedSpeakers, setSavedSpeakers] = useState<{id:string,name:string}[]>([])

  const wsRef = useRef<WebSocket | null>(null)
  const playCtxRef = useRef<AudioContext | null>(null)
  const playBufferRef = useRef<Int16Array[]>([])
  const playSampleRateRef = useRef(16000)
  const audioRef = useRef<{ stream: MediaStream; ctx: AudioContext; source: MediaStreamAudioSourceNode; processor: ScriptProcessorNode; sink: GainNode } | null>(null)
  const vadRef = useRef({ isSpeaking: false, silenceStart: 0, speechStart: 0, hasSpeech: false })
  const chatEndRef = useRef<HTMLDivElement>(null)
  const eventsEndRef = useRef<HTMLDivElement>(null)

  const now = () => new Date().toLocaleTimeString()
  const log = (msg: string) => setMessages(prev => [...prev, { role: 'system', content: msg, timestamp: now() }])

  // --- WebSocket ---
  const connectStream = useCallback((sessionId: string) => {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const ws = new WebSocket(`${protocol}//${window.location.host}/api/v1/sessions/${sessionId}/stream`)
    wsRef.current = ws

    ws.onopen = () => {
      setWsReady(true)
      setPhase('listening')
      // Sync modes to server on connect
      if (translateRef.current) {
        ws.send(JSON.stringify({ type: 'set_translate', enabled: true }))
      }
      if (cloneRef.current && cloneSpeakerRef.current) {
        ws.send(JSON.stringify({ type: 'set_clone', enabled: true, speaker: cloneSpeakerRef.current }))
      }
      startListening()
    }

    ws.binaryType = 'arraybuffer'

    ws.onmessage = (e: MessageEvent) => {
      // Binary: TTS audio PCM16 data
      if (e.data instanceof ArrayBuffer) {
        playBufferRef.current.push(new Int16Array(e.data))
        return
      }

      try {
        const msg: WsMsg = JSON.parse(e.data)

        if (msg.type === 'tts_start') {
          // Reset play buffer
          playBufferRef.current = []
          playSampleRateRef.current = (msg.sample_rate as number) || 16000
        } else if (msg.type === 'tts_end') {
          // Play accumulated audio
          const chunks = playBufferRef.current
          playBufferRef.current = []
          if (chunks.length > 0) {
            const totalLen = chunks.reduce((sum, c) => sum + c.length, 0)
            const merged = new Float32Array(totalLen)
            let offset = 0
            for (const chunk of chunks) {
              for (let i = 0; i < chunk.length; i++) {
                merged[offset++] = chunk[i] / 32768.0
              }
            }
            const sampleRate = playSampleRateRef.current
            if (!playCtxRef.current) playCtxRef.current = new AudioContext({ sampleRate })
            const ctx = playCtxRef.current
            const buf = ctx.createBuffer(1, merged.length, sampleRate)
            buf.copyToChannel(merged, 0)
            const src = ctx.createBufferSource()
            src.buffer = buf
            src.connect(ctx.destination)
            src.onended = () => {
              setPhase('listening')
              startListening()
            }
            src.start()
          } else {
            // No audio, resume listening
            setPhase('listening')
            startListening()
          }
          return
        } else if (msg.type === 'transcript') {
          // ASR recognized text — show as user message
          const text = (msg.text as string) || ''
          if (text && text !== '(未识别到语音)') {
            setMessages(prev => {
              const filtered = prev.filter(m => !(m.role === 'user' && m.content === '🎤 (语音输入)'))
              return [...filtered, { role: 'user' as const, content: text, timestamp: now() }]
            })
          }
        } else if (msg.type === 'token') {
          // Streaming token from Agent — update assistant message in real-time
          const cumulative = (msg.cumulative as string) || ''
          setPhase('playing')
          setMessages(prev => {
            // Find or create the streaming assistant message (last one)
            const last = prev[prev.length - 1]
            if (last && last.role === 'assistant' && last.content !== cumulative) {
              return [...prev.slice(0, -1), { ...last, content: cumulative }]
            } else if (!last || last.role !== 'assistant') {
              return [...prev, { role: 'assistant' as const, content: cumulative, timestamp: now() }]
            }
            return prev
          })
        } else if (msg.type === 'response') {
          // Final response — finalize the streaming message
          const userText = (msg.user_text as string) || ''
          const agentText = (msg.agent_text as string) || ''
          setMessages(prev => {
            const filtered = prev.filter(m => !(m.role === 'user' && m.content === '🎤 (语音输入)'))
            const hasUserMsg = filtered.some(m => m.role === 'user' && m.content === userText)
            // Replace the streaming assistant message with the final one
            const hasStreaming = filtered.length > 0 && filtered[filtered.length - 1]?.role === 'assistant'
            const base = hasStreaming ? filtered.slice(0, -1) : filtered
            return [
              ...base,
              ...(!hasUserMsg && userText ? [{ role: 'user' as const, content: userText, timestamp: now() }] : []),
              { role: 'assistant' as const, content: agentText, timestamp: now() },
            ]
          })
          setPhase('playing')
        } else if (msg.type === 'status') {
          if (msg.status === 'closed') {
            setSession(null)
            setWsReady(false)
            setPhase('idle')
            stopMic()
          }
        } else if (msg.type === 'event') {
          setEvents(prev => [...prev, msg])
          // Extract ASR transcript from event and display as user message
          const et = (msg.event_type as string) || ''
          if (et.includes('transcript_final')) {
            const data = (msg.data as Record<string, unknown>) || {}
            const transcript = (data.transcript as string) || ''
            if (transcript) {
              // Replace placeholder with real ASR text
              setMessages(prev => {
                const filtered = prev.filter(m => !(m.role === 'user' && m.content === '🎤 (语音输入)'))
                return [...filtered, { role: 'user', content: transcript, timestamp: now() }]
              })
            }
          }
        } else if (msg.type === 'error') {
          log(`错误: ${msg.message}`)
          setPhase('listening')
          startListening()
        }
      } catch { /* ignore */ }
    }

    ws.onclose = () => { wsRef.current = null; setWsReady(false); setPhase('idle') }
  }, [])

  // --- Mic: open once, keep open ---
  const openMic = async () => {
    if (audioRef.current) return // already open
    const stream = await navigator.mediaDevices.getUserMedia({
      audio: { sampleRate: 16000, channelCount: 1, echoCancellation: true, noiseSuppression: true },
    })
    const ctx = new AudioContext({ sampleRate: 16000 })
    const source = ctx.createMediaStreamSource(stream)
    const processor = ctx.createScriptProcessor(2048, 1, 1)
    const sink = ctx.createGain()
    sink.gain.value = 0
    source.connect(processor)
    processor.connect(sink)
    sink.connect(ctx.destination)
    audioRef.current = { stream, ctx, source, processor, sink }
  }

  const stopMic = () => {
    const a = audioRef.current
    if (a) {
      a.processor.disconnect()
      a.sink.disconnect()
      a.source.disconnect()
      a.ctx.close()
      a.stream.getTracks().forEach(t => t.stop())
    }
    audioRef.current = null
    vadRef.current = { isSpeaking: false, silenceStart: 0, speechStart: 0, hasSpeech: false }
    setVolume(0)
  }

  // --- VAD + streaming: start listening for speech ---
  const startListening = useCallback(() => {
    const audio = audioRef.current
    if (!audio) return
    const vad = vadRef.current
    vad.isSpeaking = false
    vad.silenceStart = 0
    vad.speechStart = 0
    vad.hasSpeech = false

    audio.processor.onaudioprocess = (e) => {
      const float32 = e.inputBuffer.getChannelData(0)

      // Compute RMS volume
      let sum = 0
      for (let i = 0; i < float32.length; i++) sum += float32[i] * float32[i]
      const rms = Math.sqrt(sum / float32.length)
      setVolume(rms)

      const now = Date.now()
      const isSpeech = rms > SILENCE_THRESHOLD

      if (isSpeech) {
        if (!vad.isSpeaking) {
          // Speech started
          vad.isSpeaking = true
          vad.speechStart = now
          vad.hasSpeech = false
          // Tell server: start audio
          if (wsRef.current?.readyState === WebSocket.OPEN) {
            wsRef.current.send(JSON.stringify({ type: 'audio_start' }))
          }
        }
        vad.silenceStart = 0

        // Mark as real speech after minimum duration
        if (now - vad.speechStart > SPEECH_MIN_MS) {
          vad.hasSpeech = true
        }

        // Send audio binary
        if (wsRef.current?.readyState === WebSocket.OPEN) {
          const int16 = new Int16Array(float32.length)
          for (let i = 0; i < float32.length; i++) {
            int16[i] = Math.max(-32768, Math.min(32767, Math.round(float32[i] * 32767)))
          }
          wsRef.current.send(int16.buffer)
        }
      } else if (vad.isSpeaking) {
        // Still send audio during silence gap (might be a pause between words)
        if (wsRef.current?.readyState === WebSocket.OPEN) {
          const int16 = new Int16Array(float32.length)
          for (let i = 0; i < float32.length; i++) {
            int16[i] = Math.max(-32768, Math.min(32767, Math.round(float32[i] * 32767)))
          }
          wsRef.current.send(int16.buffer)
        }

        if (!vad.silenceStart) vad.silenceStart = now

        // Check if silence long enough to end turn
        if (vad.hasSpeech && now - vad.silenceStart > SILENCE_DURATION_MS) {
          // End of speech detected
          vad.isSpeaking = false
          audio.processor.onaudioprocess = null // stop processing
          setPhase('thinking')
          setMessages(prev => [...prev, { role: 'user', content: '🎤 (语音输入)', timestamp: new Date().toLocaleTimeString() }])

          // Tell server: audio ended, process it
          if (wsRef.current?.readyState === WebSocket.OPEN) {
            wsRef.current.send(JSON.stringify({ type: 'audio_end' }))
          }
        }
      }
    }
  }, [])

  // --- Session lifecycle ---
  const startConversation = async () => {
    try {
      await openMic()
      const s = await api.createSession({
        language: 'zh-CN',
        asr_providers: [providers.asr],
        agent_providers: [providers.agent],
        tts_providers: [providers.tts],
      })
      setSession(s)
      setEvents([])
      setMessages([])
      connectStream(s.session_id)
    } catch (e) {
      log(`失败: ${e}`)
    }
  }

  const endConversation = () => {
    
    stopMic()
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify({ type: 'close' }))
    }
    setPhase('idle')
    setSession(null)
  }

  const interrupt = () => {
    
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify({ type: 'interrupt' }))
    }
    setPhase('listening')
    startListening()
  }

  // Load saved speakers for clone mode
  useEffect(() => {
    fetch('/api/v1/clone/speakers').then(r => r.json())
      .then(d => setSavedSpeakers((d.speakers || []).map((s: {id:string,name:string}) => ({ id: s.id, name: s.name }))))
      .catch(() => {})
  }, [])

  useEffect(() => { chatEndRef.current?.scrollIntoView({ behavior: 'smooth' }) }, [messages])
  useEffect(() => { eventsEndRef.current?.scrollIntoView({ behavior: 'smooth' }) }, [events])
  useEffect(() => { return () => { stopMic(); wsRef.current?.close();  } }, [])

  // Phase colors and labels
  const phaseConfig: Record<string, { color: string; label: string; pulse: boolean }> = {
    idle: { color: '#27272a', label: '未开始', pulse: false },
    listening: { color: '#22c55e', label: '聆听中...', pulse: true },
    speaking: { color: '#06b6d4', label: '说话中...', pulse: true },
    thinking: { color: '#f59e0b', label: 'AI 思考中...', pulse: true },
    playing: { color: '#8b5cf6', label: 'AI 回复中...', pulse: true },
  }
  const pc = phaseConfig[phase] || phaseConfig.idle

  const eventColor = (et: string) => {
    if (!et) return '#6b7280'
    if (et.includes('session')) return '#8b5cf6'
    if (et.includes('asr') || et.includes('transcript')) return '#06b6d4'
    if (et.includes('agent')) return '#f59e0b'
    if (et.includes('tts') || et.includes('playback')) return '#22c55e'
    if (et.includes('interrupt')) return '#ef4444'
    return '#6b7280'
  }

  return (
    <div>
      <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 16 }}>实时语音对话</h1>

      {/* Config */}
      <div style={{ display: 'flex', gap: 10, marginBottom: 16, alignItems: 'flex-end', flexWrap: 'wrap' }}>
        {(['agent'] as const).map((type) => (
          <div key={type}>
            <label style={{ fontSize: 10, color: '#52525b', display: 'block', marginBottom: 2 }}>Agent</label>
            <select value={providers[type]} onChange={(e) => setProviders(p => ({ ...p, [type]: e.target.value }))}
              style={{ padding: '4px 8px', background: '#1e1e2e', color: '#a1a1aa', border: '1px solid #27272a', borderRadius: 4, fontSize: 11 }}>
              <option value="ollama">Ollama 本地</option>
              <option value="mock">Mock</option>
              <option value="openai">OpenAI</option>
            </select>
          </div>
        ))}
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginLeft: 8 }}>
          <button
            onClick={() => {
              const next = !translateMode
              setTranslateMode(next)
              translateRef.current = next
              if (wsRef.current?.readyState === WebSocket.OPEN) {
                wsRef.current.send(JSON.stringify({ type: 'set_translate', enabled: next }))
              }
            }}
            style={{
              padding: '4px 12px',
              background: translateMode ? '#7c3aed' : '#1e1e2e',
              color: translateMode ? '#fff' : '#71717a',
              border: `1px solid ${translateMode ? '#7c3aed' : '#27272a'}`,
              borderRadius: 4,
              cursor: 'pointer',
              fontSize: 11,
              fontWeight: 600,
              transition: 'all 0.2s',
            }}
          >
            {translateMode ? '中 → EN' : '中 → 中'}
          </button>
          <span style={{ fontSize: 10, color: '#52525b' }}>
            {translateMode ? '说中文，AI 用英语回答' : '中文对话'}
          </span>
        </div>

        {/* Clone voice toggle */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginLeft: 8 }}>
          <button
            onClick={() => {
              const next = !cloneMode
              setCloneMode(next)
              cloneRef.current = next
              if (wsRef.current?.readyState === WebSocket.OPEN) {
                wsRef.current.send(JSON.stringify({
                  type: 'set_clone', enabled: next, speaker: cloneSpeakerRef.current,
                }))
              }
            }}
            style={{
              padding: '4px 12px',
              background: cloneMode ? '#22c55e' : '#1e1e2e',
              color: cloneMode ? '#fff' : '#71717a',
              border: `1px solid ${cloneMode ? '#22c55e' : '#27272a'}`,
              borderRadius: 4, cursor: 'pointer', fontSize: 11, fontWeight: 600,
              transition: 'all 0.2s',
            }}
          >
            {cloneMode ? 'Clone ON' : 'Clone'}
          </button>
          {cloneMode && (
            <select
              value={cloneSpeaker}
              onChange={e => {
                setCloneSpeaker(e.target.value)
                cloneSpeakerRef.current = e.target.value
                if (wsRef.current?.readyState === WebSocket.OPEN) {
                  wsRef.current.send(JSON.stringify({
                    type: 'set_clone', enabled: true, speaker: e.target.value,
                  }))
                }
              }}
              style={{
                padding: '4px 8px', background: '#1e1e2e', color: '#a1a1aa',
                border: '1px solid #27272a', borderRadius: 4, fontSize: 11,
              }}
            >
              <option value="">Select voice...</option>
              {savedSpeakers.map(s => (
                <option key={s.id} value={s.id}>{s.name}</option>
              ))}
            </select>
          )}
        </div>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 300px', gap: 12 }}>
        {/* Main area */}
        <div style={{ background: '#111118', border: '1px solid #27272a', borderRadius: 12, display: 'flex', flexDirection: 'column', height: 540 }}>
          {/* Chat messages */}
          <div style={{ flex: 1, overflow: 'auto', padding: 16 }}>
            {messages.map((msg, i) => (
              <div key={i} style={{ marginBottom: 10, display: 'flex', justifyContent: msg.role === 'user' ? 'flex-end' : 'flex-start' }}>
                <div style={{
                  maxWidth: '85%', padding: '10px 14px',
                  borderRadius: msg.role === 'user' ? '16px 16px 4px 16px' : '16px 16px 16px 4px',
                  fontSize: 15, lineHeight: 1.6, whiteSpace: 'pre-wrap' as const,
                  background: msg.role === 'user' ? '#7c3aed' : msg.role === 'assistant' ? '#1e1e2e' : 'transparent',
                  color: msg.role === 'system' ? '#52525b' : '#e4e4e7',
                }}>
                  {msg.content}
                </div>
              </div>
            ))}
            {phase === 'thinking' && (
              <div style={{ display: 'flex', gap: 5, padding: '10px 0' }}>
                {[0, 1, 2].map(i => <div key={i} style={{ width: 8, height: 8, borderRadius: '50%', background: '#f59e0b', animation: `pulse 1s ease-in-out ${i * 0.2}s infinite` }} />)}
              </div>
            )}
            <div ref={chatEndRef} />
          </div>

          {/* Bottom: voice control */}
          <div style={{ padding: 20, borderTop: '1px solid #27272a', display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 12 }}>
            {phase === 'idle' ? (
              <button onClick={startConversation} style={{
                padding: '14px 40px', background: '#22c55e', color: '#fff', border: 'none',
                borderRadius: 30, cursor: 'pointer', fontWeight: 700, fontSize: 16,
              }}>
                开始对话
              </button>
            ) : (
              <>
                {/* Status indicator */}
                <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                  <div style={{
                    width: 14, height: 14, borderRadius: '50%', background: pc.color,
                    boxShadow: pc.pulse ? `0 0 12px ${pc.color}` : 'none',
                    animation: pc.pulse ? 'pulse-glow 1.5s ease-in-out infinite' : 'none',
                  }} />
                  <span style={{ fontSize: 14, color: pc.color, fontWeight: 500 }}>{pc.label}</span>
                </div>

                {/* Volume bar (when listening) */}
                {(phase === 'listening' || phase === 'speaking') && (
                  <div style={{ width: 200, height: 4, background: '#27272a', borderRadius: 2, overflow: 'hidden' }}>
                    <div style={{
                      height: '100%', background: volume > SILENCE_THRESHOLD ? '#22c55e' : '#3f3f46',
                      width: `${Math.min(100, volume * 2000)}%`, transition: 'width 0.05s',
                    }} />
                  </div>
                )}

                {/* Buttons */}
                <div style={{ display: 'flex', gap: 10 }}>
                  {phase === 'playing' && (
                    <button onClick={interrupt} style={{
                      padding: '8px 20px', background: '#f59e0b', color: '#000', border: 'none',
                      borderRadius: 20, cursor: 'pointer', fontSize: 13, fontWeight: 600,
                    }}>
                      打断
                    </button>
                  )}
                  <button onClick={endConversation} style={{
                    padding: '8px 20px', background: '#ef4444', color: '#fff', border: 'none',
                    borderRadius: 20, cursor: 'pointer', fontSize: 13, fontWeight: 600,
                  }}>
                    结束对话
                  </button>
                </div>
              </>
            )}
          </div>
        </div>

        {/* Events */}
        <div style={{ background: '#111118', border: '1px solid #27272a', borderRadius: 12, height: 540, overflow: 'auto', padding: 10 }}>
          <div style={{ fontSize: 10, color: '#3f3f46', marginBottom: 6, letterSpacing: 1 }}>事件流</div>
          {events.length === 0 ? (
            <div style={{ color: '#1e1e2e', fontSize: 11, paddingTop: 30, textAlign: 'center' }}>等待中...</div>
          ) : (
            events.map((evt, i) => (
              <div key={i} style={{ padding: '3px 0', fontSize: 10, borderBottom: '1px solid #1a1a1f' }}>
                <span style={{ color: '#3f3f46', marginRight: 4, fontFamily: 'monospace' }}>#{String(evt.seq || '')}</span>
                <span style={{ color: eventColor((evt.event_type as string) || ''), fontWeight: 500 }}>
                  {((evt.event_type as string) || '').replace('prx.voice.', '')}
                </span>
              </div>
            ))
          )}
          <div ref={eventsEndRef} />
        </div>
      </div>

      <style>{`
        @keyframes pulse { 0%, 100% { opacity: 0.3; } 50% { opacity: 1; } }
        @keyframes pulse-glow { 0%, 100% { box-shadow: 0 0 4px currentColor; } 50% { box-shadow: 0 0 16px currentColor; } }
      `}</style>
    </div>
  )
}
