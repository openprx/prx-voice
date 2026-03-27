import { create } from 'zustand'
import type { SessionInfo, VoiceEvent } from '../types/api'

interface AppStore {
  sessions: SessionInfo[]
  activeSession: SessionInfo | null
  events: VoiceEvent[]
  connected: boolean

  setSessions: (sessions: SessionInfo[]) => void
  setActiveSession: (session: SessionInfo | null) => void
  addEvent: (event: VoiceEvent) => void
  clearEvents: () => void
  setConnected: (connected: boolean) => void
}

export const useStore = create<AppStore>((set) => ({
  sessions: [],
  activeSession: null,
  events: [],
  connected: false,

  setSessions: (sessions) => set({ sessions }),
  setActiveSession: (session) => set({ activeSession: session }),
  addEvent: (event) => set((state) => ({ events: [...state.events, event] })),
  clearEvents: () => set({ events: [] }),
  setConnected: (connected) => set({ connected }),
}))
