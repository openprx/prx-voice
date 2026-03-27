/** Unified API response envelope */
export interface ApiResponse<T> {
  request_id: string
  timestamp: string
  data?: T
  error?: ApiError
}

export interface ApiError {
  code: string
  message: string
  retryable: boolean
}

export interface SessionInfo {
  session_id: string
  state: string
  channel: string
  language: string
  current_turn_id: number
}

export interface SessionListResponse {
  items: SessionInfo[]
  pagination: {
    has_more: boolean
    total_count: number
    cursor?: string
  }
}

export interface CreateSessionRequest {
  channel?: string
  language?: string
  asr_providers?: string[]
  agent_providers?: string[]
  tts_providers?: string[]
}

export interface HealthResponse {
  status: string
  version: string
}

/** WebSocket event message */
export interface WsEventMessage {
  type: 'event' | 'heartbeat'
  seq?: number
  event?: VoiceEvent
  ts?: string
  last_seq?: number
}

export interface VoiceEvent {
  specversion: string
  id: string
  source: string
  type: string
  subject: string
  time: string
  prx_session_id: string
  prx_turn_id: number
  prx_seq: number
  prx_severity: string
  data: Record<string, unknown>
}

export type SessionState =
  | 'Idle' | 'Connecting' | 'Listening' | 'UserSpeaking'
  | 'AsrProcessing' | 'Thinking' | 'Speaking' | 'Interrupted'
  | 'Paused' | 'HandoffPending' | 'Closed' | 'Failed'
