import type { ApiResponse, SessionInfo, SessionListResponse, CreateSessionRequest, HealthResponse } from '../types/api'

const BASE = '/api/v1'

async function request<T>(path: string, options?: RequestInit): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    headers: { 'Content-Type': 'application/json' },
    ...options,
  })
  if (!res.ok) {
    const text = await res.text().catch(() => '')
    try {
      const err = JSON.parse(text)
      throw new Error(`[${err.error?.code || res.status}] ${err.error?.message || res.statusText}`)
    } catch {
      throw new Error(`[${res.status}] ${res.statusText}: ${text.slice(0, 100)}`)
    }
  }
  const text = await res.text()
  if (!text) throw new Error('Empty response from server')
  const json: ApiResponse<T> = JSON.parse(text)
  if (json.error) {
    throw new Error(`[${json.error.code}] ${json.error.message}`)
  }
  return json.data as T
}

export const api = {
  /** Create a new session */
  createSession: (req: CreateSessionRequest = {}) =>
    request<SessionInfo>('/sessions', {
      method: 'POST',
      body: JSON.stringify(req),
    }),

  /** List sessions */
  listSessions: (limit = 20) =>
    request<SessionListResponse>(`/sessions?limit=${limit}`),

  /** Get session details */
  getSession: (id: string) =>
    request<SessionInfo>(`/sessions/${id}`),

  /** Close a session */
  closeSession: (id: string, reason = 'normal_clearing') =>
    request<SessionInfo>(`/sessions/${id}/close`, {
      method: 'POST',
      body: JSON.stringify({ reason }),
    }),

  /** Interrupt a session */
  interruptSession: (id: string) =>
    request<SessionInfo>(`/sessions/${id}/interrupt`, { method: 'POST' }),

  /** Execute a turn (simulate user speech → ASR → Agent → TTS) */
  executeTurn: (id: string, text?: string) =>
    request<{ session_id: string; turn_id: number; state: string; user_transcript: string; agent_response: string }>(
      `/sessions/${id}/turns`,
      { method: 'POST', body: JSON.stringify({ text }) },
    ),

  /** Health check */
  health: () => request<HealthResponse>('/health'),

  /** Get metrics (returns plain text) */
  metrics: async () => {
    const res = await fetch(`${BASE}/metrics`)
    return res.text()
  },

  /** Subscribe to session events via WebSocket */
  subscribeEvents: (sessionId: string): WebSocket => {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const ws = new WebSocket(`${protocol}//${window.location.host}${BASE}/sessions/${sessionId}/events`)
    return ws
  },

  /** Open real-time WebSocket stream for a session */
  openStream: (sessionId: string): WebSocket => {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    return new WebSocket(`${protocol}//${window.location.host}${BASE}/sessions/${sessionId}/stream`)
  },
}
