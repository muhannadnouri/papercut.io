import { invoke, isTauri } from '@tauri-apps/api/core'

export interface LibraryTransferErrorPayload {
  code: string
  message: string
  requiredBytes?: number
  availableBytes?: number
}

export interface LibraryTransferExportResult {
  documents: number
  audiobooks: number
}

export interface LibraryTransferFailure {
  item: string
  error: string
}

export interface LibraryTransferImportResult {
  selected: number
  imported: number
  skipped: number
  failed: number
  foldersCreated: number
  audiobooksSelected: number
  audiobooksImported: number
  audiobooksSkipped: number
  audiobooksFailed: number
  importedAudiobooks: ImportedAudiobookRecord[]
  failures: LibraryTransferFailure[]
}

export interface ImportedAudiobookRecord {
  id: string
  documentUrl: string
  title: string
  voice: string
  speed: number
  modelId: string
  textPreprocessor: string
  silmaNfeStep?: number
  dtype: string
  savedAt: number
  chunks: number
  audioDurationSec: number
  wavBytes: number
}

export type LibraryTransferSendState = 'preparing' | 'binding' | 'waiting' | 'sending' | 'complete' | 'cancelled' | 'failed'

export interface LibraryTransferSendStatus {
  id: number
  listenerReady: boolean
  addresses: string[]
  expiresAtMs?: number | null
  reason?: string | null
  state: LibraryTransferSendState
  address: string
  code: string
  documents: number
  audiobooks: number
  packageBytes: number
  bytesTransferred: number
  receiverProgress?: LibraryTransferProgress | null
  error?: string | null
}

export interface TransferInterface {
  id: string
  name: string
  index: number
  addresses: string[]
  automatic: boolean
}

export interface TransferConnection {
  interfaceId: string
  port: number
}

export interface LibraryTransferReceiveStatus {
  id: number
  phase: 'connecting' | 'receiving' | 'finishingImport' | 'complete' | 'cancelled' | 'failed'
  cancellable: boolean
}

export async function getTransferInterfaces(): Promise<TransferInterface[]> {
  if (!isTauri()) return []
  return invoke('library_transfer_interfaces')
}

export async function getLibraryReceiveStatus(): Promise<LibraryTransferReceiveStatus | null> {
  return invoke('library_transfer_receive_status')
}

export async function cancelLibraryReceive(id: number): Promise<boolean> {
  return invoke('library_transfer_receive_cancel', { id })
}

export type LibraryTransferOperation = 'import' | 'receive'
export type LibraryTransferProgressPhase =
  | 'connecting'
  | 'receiving'
  | 'verifying'
  | 'importingDocuments'
  | 'restoringAudiobooks'

export interface LibraryTransferProgress {
  sessionId?: number
  operation: LibraryTransferOperation
  phase: LibraryTransferProgressPhase
  bytesProcessed?: number
  bytesTotal?: number
  itemsProcessed?: number
  itemsTotal?: number
  item?: string
}

const LIBRARY_TRANSFER_PROGRESS_EVENT = 'library-transfer-progress'

export interface TransferDiagnosticEvent {
  attempt: number
  role: string
  timestampMs: number
  elapsedMs: number
  phase: string
  level: string
  code: string
  bytes: number | null
  totalBytes: number | null
  items: number | null
  totalItems: number | null
  ioKind: string | null
  osCode: number | null
  port: number | null
  addressScope: string | null
  requiredBytes: number | null
  availableBytes: number | null
  imported: number | null
  skipped: number | null
  failed: number | null
}

export interface TransferDiagnosticSnapshot {
  appVersion: string
  build: string | null
  os: string
  arch: string
  protocol: string
  dropped: number
  events: TransferDiagnosticEvent[]
}

export async function getTransferDiagnostics(): Promise<TransferDiagnosticSnapshot | null> {
  if (!isTauri()) return null
  return invoke<TransferDiagnosticSnapshot>('library_transfer_diagnostics')
}

/** Copy only the native diagnostic allowlist, never command requests or progress item labels. */
export function formatTransferDiagnosticReport(snapshot: TransferDiagnosticSnapshot): string {
  return JSON.stringify({
    reportVersion: 1, redacted: true,
    appVersion: snapshot.appVersion, build: snapshot.build, os: snapshot.os, arch: snapshot.arch,
    protocol: snapshot.protocol, dropped: snapshot.dropped + Math.max(0, snapshot.events.length - 200),
    events: snapshot.events.slice(-200).map(event => ({
      attempt: event.attempt, role: event.role, timestampMs: event.timestampMs,
      elapsedMs: event.elapsedMs, phase: event.phase, level: event.level, code: event.code,
      bytes: event.bytes, totalBytes: event.totalBytes, items: event.items, totalItems: event.totalItems,
      ioKind: event.ioKind, osCode: event.osCode, port: event.port, addressScope: event.addressScope,
      requiredBytes: event.requiredBytes, availableBytes: event.availableBytes,
      imported: event.imported, skipped: event.skipped, failed: event.failed,
    })),
  }, null, 2)
}

export async function exportLibrary(
  documentIds: string[],
  audiobookIds: string[] = [],
): Promise<LibraryTransferExportResult | null> {
  if (!isTauri()) return null
  return invoke<LibraryTransferExportResult | null>('library_transfer_export', {
    request: { documentIds, audiobookIds },
  })
}

export async function importLibrary(): Promise<LibraryTransferImportResult | null> {
  if (!isTauri()) return null
  return invoke<LibraryTransferImportResult | null>('library_transfer_import')
}

export async function startLibrarySend(
  documentIds: string[],
  audiobookIds: string[] = [],
  connection?: TransferConnection,
): Promise<LibraryTransferSendStatus> {
  return invoke<LibraryTransferSendStatus>('library_transfer_send_start', {
    ...(connection ? { connection } : {}),
    request: { documentIds, audiobookIds },
  })
}

export async function getLibrarySendStatus(): Promise<LibraryTransferSendStatus | null> {
  if (!isTauri()) return null
  return invoke<LibraryTransferSendStatus | null>('library_transfer_send_status')
}

export async function cancelLibrarySend(id: number): Promise<void> {
  if (!isTauri()) return
  await invoke('library_transfer_send_cancel', { id })
}

export async function receiveLibrary(address: string, code: string, interfaceId?: string): Promise<LibraryTransferImportResult> {
  return invoke<LibraryTransferImportResult>('library_transfer_receive', {
    request: { address, code, ...(interfaceId ? { interfaceId } : {}) },
  })
}

export async function listenLibraryTransferProgress(
  onProgress: (progress: LibraryTransferProgress) => void,
): Promise<() => void> {
  if (!isTauri()) return () => {}
  const { listen } = await import('@tauri-apps/api/event')
  return listen<LibraryTransferProgress>(LIBRARY_TRANSFER_PROGRESS_EVENT, (event) => {
    onProgress(event.payload)
  })
}

export function isLibraryTransferErrorPayload(value: unknown): value is LibraryTransferErrorPayload {
  if (!value || typeof value !== 'object') return false
  const payload = value as Record<string, unknown>
  return typeof payload.code === 'string'
    && typeof payload.message === 'string'
    && (payload.requiredBytes === undefined || typeof payload.requiredBytes === 'number')
    && (payload.availableBytes === undefined || typeof payload.availableBytes === 'number')
}
