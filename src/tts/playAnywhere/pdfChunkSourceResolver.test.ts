import { describe, expect, it } from 'vitest'
import { createPdfChunkSourceResolver } from './pdfChunkSourceResolver'
import type { TtsChunk } from '../types'

const span = (blockOrder: number, startOffset: number, endOffset: number, pageIndex = 2) =>
  ({ pageIndex, blockOrder, startOffset, endOffset })
const chunks: TtsChunk[] = [
  { id: 'blank', text: ' ' },
  { id: 'first', text: 'first', pdfSourceSpans: [span(9, 3, 7), span(4, 0, 1)] },
  { id: 'next', text: 'next', pdfSourceSpans: [span(4, 0, 5), span(0, 0, 3, 3)] },
]

describe('PDF source-to-chunk lookup', () => {
  it('uses page/item/source offsets and canonical nonempty playback order, including shared ligatures', () => {
    const resolver = createPdfChunkSourceResolver(chunks)
    expect(resolver.status).toBe('ready')
    if (resolver.status !== 'ready') return
    expect(resolver.resolve({ pageIndex: 2, blockOrder: 4, offset: 0 })).toEqual({ status: 'resolved', chunkIndex: 0, chunkId: 'first' })
    expect(resolver.resolve({ pageIndex: 2, blockOrder: 4, offset: 1 })).toEqual({ status: 'resolved', chunkIndex: 1, chunkId: 'next' })
    expect(resolver.resolve({ pageIndex: 3, blockOrder: 0, offset: 2 })).toEqual({ status: 'resolved', chunkIndex: 1, chunkId: 'next' })
    expect(resolver.resolve({ pageIndex: 2, blockOrder: 9, offset: 7 })).toEqual({ status: 'unavailable', reason: 'unmapped' })
    expect(resolver.resolve({ pageIndex: 1, blockOrder: 4, offset: 1 })).toEqual({ status: 'unavailable', reason: 'unmapped' })
    expect(resolver.resolve({ pageIndex: 2, blockOrder: 4, offset: -1 })).toEqual({ status: 'unavailable', reason: 'invalid-point' })
    chunks[1].pdfSourceSpans![0].startOffset = 100
    expect(resolver.resolve({ pageIndex: 2, blockOrder: 9, offset: 3 }).status).toBe('resolved')
  })

  it('rejects empty, incomplete, duplicate and malformed source metadata', () => {
    expect(createPdfChunkSourceResolver([])).toEqual({ status: 'unavailable', reason: 'not-ready' })
    expect(createPdfChunkSourceResolver([{ id: 'a', text: 'text' }])).toEqual({ status: 'unavailable', reason: 'unmapped' })
    const one: TtsChunk = { id: 'a', text: 'text', pdfSourceSpans: [span(0, 0, 1)] }
    expect(createPdfChunkSourceResolver([one, one])).toEqual({ status: 'unavailable', reason: 'invalid-source' })
    for (const invalid of [span(-1, 0, 1), span(0, 2, 1), span(0, 0, Infinity), span(0, 0, 1, 0.5)]) {
      expect(createPdfChunkSourceResolver([{ ...one, pdfSourceSpans: [invalid] }])).toEqual({ status: 'unavailable', reason: 'invalid-source' })
    }
  })
})
