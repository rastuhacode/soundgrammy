import {
  useLogStore,
  type AppLogEntry,
  type AppLogLevel,
  type AppLogSource,
} from '@/stores/log-store'

export interface AppLogInput {
  source: AppLogSource
  title: string
  description?: string
  error?: unknown
  context?: Record<string, unknown>
}

const MAX_TITLE_LENGTH = 160
const MAX_DESCRIPTION_LENGTH = 2_000
const MAX_DETAILS_LENGTH = 8_000
const MAX_VALUE_DEPTH = 5
const MAX_COLLECTION_ITEMS = 50
const REDACTED = '[REDACTED]'
const sensitiveKey = /authorization|cookie|credential|password|proxy.?secret|session|telegram.?api|token/i

function truncate(value: string, max: number): string {
  if (value.length <= max) return value
  return `${value.slice(0, Math.max(0, max - 14))}\n…[truncated]`
}

function errorDescription(error: unknown): string | null {
  if (error instanceof Error && error.message) return error.message
  if (typeof error === 'string' && error.trim()) return error
  if (error && typeof error === 'object' && 'message' in error) {
    const message = (error as { message?: unknown }).message
    if (typeof message === 'string' && message.trim()) return message
  }
  return null
}

function sanitize(
  value: unknown,
  seen: WeakSet<object>,
  depth = 0,
  key?: string,
): unknown {
  if (key && sensitiveKey.test(key)) return REDACTED
  if (value == null || typeof value === 'string' || typeof value === 'boolean') {
    return value
  }
  if (typeof value === 'number') {
    return Number.isFinite(value) ? value : String(value)
  }
  if (typeof value === 'bigint' || typeof value === 'symbol') return String(value)
  if (typeof value === 'function') return `[Function ${value.name || 'anonymous'}]`
  if (depth >= MAX_VALUE_DEPTH) return '[Maximum depth reached]'

  if (value instanceof Error) {
    return {
      name: value.name,
      message: value.message,
      stack: value.stack,
    }
  }
  if (typeof value !== 'object') return String(value)
  if (seen.has(value)) return '[Circular]'
  seen.add(value)

  if (Array.isArray(value)) {
    return value
      .slice(0, MAX_COLLECTION_ITEMS)
      .map(item => sanitize(item, seen, depth + 1))
  }

  const result: Record<string, unknown> = {}
  for (const [childKey, childValue] of Object.entries(value).slice(0, MAX_COLLECTION_ITEMS)) {
    result[childKey] = sanitize(childValue, seen, depth + 1, childKey)
  }
  return result
}

function stringify(value: unknown): string | null {
  if (value === undefined) return null
  try {
    return JSON.stringify(sanitize(value, new WeakSet()), null, 2)
  }
  catch {
    return String(value)
  }
}

function createId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID()
  }
  return `${Date.now()}-${Math.random().toString(36).slice(2)}`
}

function createEntry(level: AppLogLevel, input: AppLogInput): AppLogEntry {
  const description = input.description
    ?? errorDescription(input.error)
    ?? input.title
  const details: string[] = []
  const serializedError = stringify(input.error)
  const serializedContext = stringify(input.context)
  if (serializedError) details.push(`Error:\n${serializedError}`)
  if (serializedContext) details.push(`Context:\n${serializedContext}`)

  return {
    id: createId(),
    timestamp: new Date().toISOString(),
    level,
    source: input.source,
    title: truncate(input.title.trim() || 'Untitled diagnostic', MAX_TITLE_LENGTH),
    description: truncate(description, MAX_DESCRIPTION_LENGTH),
    ...(details.length > 0
      ? { details: truncate(details.join('\n\n'), MAX_DETAILS_LENGTH) }
      : {}),
  }
}

function write(level: AppLogLevel, input: AppLogInput): AppLogEntry {
  const entry = createEntry(level, input)
  useLogStore.getState().add(entry)

  const output = `[${input.source}] ${input.title}`
  if (level === 'error') console.error(output, input.error, input.context)
  else if (level === 'warning') console.warn(output, input.error, input.context)
  else console.info(output, input.context)

  return entry
}

export const appLogger = {
  error: (input: AppLogInput) => write('error', input),
  warning: (input: AppLogInput) => write('warning', input),
  info: (input: AppLogInput) => write('info', input),
}

export function formatLogEntry(entry: AppLogEntry): string {
  const header = `[${entry.timestamp}] ${entry.level.toUpperCase()} ${entry.source} — ${entry.title}`
  return [header, entry.description, entry.details].filter(Boolean).join('\n')
}

export function formatLogEntries(entries: AppLogEntry[]): string {
  return entries.map(formatLogEntry).join('\n\n')
}

let globalHandlersInstalled = false

export function installGlobalErrorLogging() {
  if (globalHandlersInstalled || typeof window === 'undefined') return
  globalHandlersInstalled = true

  window.addEventListener('error', (event) => {
    appLogger.error({
      source: 'application',
      title: 'Unhandled application error',
      error: event.error ?? event.message,
      context: {
        filename: event.filename,
        line: event.lineno,
        column: event.colno,
      },
    })
  })

  window.addEventListener('unhandledrejection', (event) => {
    appLogger.error({
      source: 'application',
      title: 'Unhandled promise rejection',
      error: event.reason,
    })
  })
}
