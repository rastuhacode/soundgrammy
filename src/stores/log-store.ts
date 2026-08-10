import { create } from 'zustand'

export type AppLogLevel = 'error' | 'warning' | 'info'

export type AppLogSource
  = | 'application'
    | 'audio'
    | 'backend'
    | 'cache'
    | 'playback'
    | 'telegram'
    | 'ui'

export interface AppLogEntry {
  id: string
  timestamp: string
  level: AppLogLevel
  source: AppLogSource
  title: string
  description: string
  details?: string
}

interface PersistedLogState {
  enabled: boolean
  entries: AppLogEntry[]
}

interface LogState extends PersistedLogState {
  setEnabled: (enabled: boolean) => void
  add: (entry: AppLogEntry) => void
  clear: () => void
}

export const MAX_LOG_ENTRIES = 300
const LOG_STORAGE_KEY = 'soundgrammy-diagnostic-logs'

const levels = new Set<AppLogLevel>(['error', 'warning', 'info'])
const sources = new Set<AppLogSource>([
  'application',
  'audio',
  'backend',
  'cache',
  'playback',
  'telegram',
  'ui',
])

function isLogEntry(value: unknown): value is AppLogEntry {
  if (!value || typeof value !== 'object') return false
  const entry = value as Partial<AppLogEntry>
  return typeof entry.id === 'string'
    && typeof entry.timestamp === 'string'
    && typeof entry.level === 'string'
    && levels.has(entry.level as AppLogLevel)
    && typeof entry.source === 'string'
    && sources.has(entry.source as AppLogSource)
    && typeof entry.title === 'string'
    && typeof entry.description === 'string'
    && (entry.details === undefined || typeof entry.details === 'string')
}

function loadPersistedState(): PersistedLogState {
  if (typeof window === 'undefined') return { enabled: false, entries: [] }
  try {
    const raw = window.localStorage.getItem(LOG_STORAGE_KEY)
    if (!raw) return { enabled: false, entries: [] }
    const parsed = JSON.parse(raw) as Partial<PersistedLogState>
    const entries = Array.isArray(parsed.entries)
      ? parsed.entries.filter(isLogEntry).slice(-MAX_LOG_ENTRIES)
      : []
    return {
      enabled: parsed.enabled === true,
      entries,
    }
  }
  catch {
    return { enabled: false, entries: [] }
  }
}

function persist(state: PersistedLogState) {
  if (typeof window === 'undefined') return
  try {
    window.localStorage.setItem(LOG_STORAGE_KEY, JSON.stringify(state))
  }
  catch {
    // Diagnostics must never interfere with application behavior.
  }
}

const initialState = loadPersistedState()

export const useLogStore = create<LogState>(set => ({
  ...initialState,

  setEnabled: (enabled) => {
    set((state) => {
      const next = { enabled, entries: state.entries }
      persist(next)
      return next
    })
  },

  add: (entry) => {
    set((state) => {
      if (!state.enabled) return state
      const entries = [...state.entries, entry].slice(-MAX_LOG_ENTRIES)
      persist({ enabled: state.enabled, entries })
      return { entries }
    })
  },

  clear: () => {
    set((state) => {
      persist({ enabled: state.enabled, entries: [] })
      return { entries: [] }
    })
  },
}))
