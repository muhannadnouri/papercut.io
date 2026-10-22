import type { TtsChunk } from '../types'
import type { ChunkTargetResult } from './sourceChunkResolver'

export interface PdfSourceTextPoint {
  pageIndex: number
  blockOrder: number
  offset: number
}

type PdfChunkSourceResolver = { status: 'ready'; resolve: (point: PdfSourceTextPoint) => ChunkTargetResult }
  | Extract<ChunkTargetResult, { status: 'unavailable' }>

/** Index runtime PDF spans without changing canonical playback order or durable chunk identity. */
export function createPdfChunkSourceResolver(chunks: readonly TtsChunk[]): PdfChunkSourceResolver {
  const blocks = new Map<string, Array<{ start: number; end: number; chunkIndex: number; chunkId: string }>>()
  const ids = new Set<string>()
  let chunkIndex = 0
  for (const chunk of chunks) {
    if (!chunk.text.trim()) continue
    if (!chunk.id.trim() || ids.has(chunk.id)) return { status: 'unavailable', reason: 'invalid-source' }
    if (!chunk.pdfSourceSpans?.length) return { status: 'unavailable', reason: 'unmapped' }
    ids.add(chunk.id)
    for (const span of chunk.pdfSourceSpans) {
      if (![span.pageIndex, span.blockOrder, span.startOffset, span.endOffset].every(validIndex) ||
        span.endOffset <= span.startOffset) return { status: 'unavailable', reason: 'invalid-source' }
      const key = `${span.pageIndex}:${span.blockOrder}`
      const entries = blocks.get(key) ?? []
      entries.push({ start: span.startOffset, end: span.endOffset, chunkIndex, chunkId: chunk.id })
      blocks.set(key, entries)
    }
    chunkIndex++
  }
  if (!chunkIndex) return { status: 'unavailable', reason: 'not-ready' }
  return {
    status: 'ready',
    resolve(point) {
      if (![point.pageIndex, point.blockOrder, point.offset].every(validIndex)) {
        return { status: 'unavailable', reason: 'invalid-point' }
      }
      // Expanded ligatures can belong to adjacent chunks; the first canonical chunk owns the glyph.
      const entry = blocks.get(`${point.pageIndex}:${point.blockOrder}`)
        ?.find(entry => point.offset >= entry.start && point.offset < entry.end)
      return entry ? { status: 'resolved', chunkIndex: entry.chunkIndex, chunkId: entry.chunkId }
        : { status: 'unavailable', reason: 'unmapped' }
    },
  }
}

function validIndex(value: number): boolean { return Number.isSafeInteger(value) && value >= 0 }
