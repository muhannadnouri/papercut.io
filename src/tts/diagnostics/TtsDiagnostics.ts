export type TtsDiagnosticLevel = 'info' | 'warn' | 'error'

export interface TtsDiagnosticEvent {
  id: string
  timestamp: number
  level: TtsDiagnosticLevel
  label: string
  data: Record<string, unknown>
}

const STORAGE_KEY = 'papercut.ttsDiagnostics.v1'
const MAX_EVENTS = 100
const MAX_ARRAY_ITEMS = 20
const MAX_OBJECT_KEYS = 30

let events: TtsDiagnosticEvent[] = loadEvents()
const listeners = new Set<() => void>()

export function getTtsDiagnostics(): TtsDiagnosticEvent[] {
  return events
}

export function subscribeTtsDiagnostics(listener: () => void): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function clearTtsDiagnostics(): void {
  events = []
  persistEvents()
  notifyListeners()
}

export function logTtsDiagnostic(
  label: string,
  data: Record<string, unknown> = {},
  level: TtsDiagnosticLevel = 'info',
): void {
  const event: TtsDiagnosticEvent = {
    id: String(Date.now()) + '-' + Math.random().toString(16).slice(2),
    timestamp: Date.now(),
    level,
    label,
    data: normalizeData(data),
  }

  events = [event, ...events].slice(0, MAX_EVENTS)
  persistEvents()
  notifyListeners()

  if (level === 'error') {
    console.error(label, data)
  } else if (level === 'warn') {
    console.warn(label, data)
  } else {
    console.info(label, data)
  }
}

export function summarizeTtsCapabilities(capabilities: {
  available: boolean
  backend: string
  defaultThreadCount: number
  maxThreadCount: number
  modelDir?: string | null
  models?: Array<{ id: string }>
  platform: string
  reason: string
}): Record<string, unknown> {
  const modelIds = capabilities.models?.map((model) => model.id) ?? []
  return {
    available: capabilities.available,
    backend: capabilities.backend,
    reason: capabilities.reason,
    platform: capabilities.platform,
    defaultThreadCount: capabilities.defaultThreadCount,
    maxThreadCount: capabilities.maxThreadCount,
    modelCount: modelIds.length,
    modelIds: modelIds.join(', '),
    modelDir: capabilities.modelDir ?? '',
  }
}

function normalizeData(data: Record<string, unknown>): Record<string, unknown> {
  const normalized: Record<string, unknown> = {}
  for (const [key, value] of Object.entries(data)) {
    normalized[key] = normalizeDiagnosticValue(value)
  }
  return normalized
}

function normalizeDiagnosticValue(
  value: unknown,
  seen: WeakSet<object> = new WeakSet(),
): unknown {
  if (
    value === null ||
    typeof value === 'string' ||
    typeof value === 'number' ||
    typeof value === 'boolean'
  ) {
    return value
  }
  if (value === undefined) return ''
  if (Array.isArray(value)) {
    if (seen.has(value)) return '[Circular]'
    seen.add(value)
    return value.slice(0, MAX_ARRAY_ITEMS).map((item) => normalizeDiagnosticValue(item, seen))
  }
  if (typeof value === 'object') {
    if (seen.has(value)) return '[Circular]'
    seen.add(value)
    const normalized: Record<string, unknown> = {}
    for (const [key, item] of Object.entries(value).slice(0, MAX_OBJECT_KEYS)) {
      normalized[key] = normalizeDiagnosticValue(item, seen)
    }
    return normalized
  }
  return String(value)
}

function loadEvents(): TtsDiagnosticEvent[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw) as TtsDiagnosticEvent[]
    if (!Array.isArray(parsed)) return []
    return parsed.filter(isDiagnosticEvent).slice(0, MAX_EVENTS)
  } catch {
    return []
  }
}

function persistEvents(): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(events))
  } catch {
    // Diagnostics should never interrupt narration or saving.
  }
}

function notifyListeners(): void {
  for (const listener of listeners) listener()
}

function isDiagnosticEvent(value: TtsDiagnosticEvent): value is TtsDiagnosticEvent {
  return Boolean(
    value &&
    typeof value.id === 'string' &&
    typeof value.timestamp === 'number' &&
    typeof value.label === 'string' &&
    value.data &&
    typeof value.data === 'object',
  )
}
