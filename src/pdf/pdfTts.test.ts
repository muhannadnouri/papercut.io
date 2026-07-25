import { describe, expect, it } from 'vitest'
import {
  AUDIOBOOK_SAVE_CHUNK_PROFILE,
  chunkAudiobookSaveSegmentsWithSpans,
} from '../tts/utils/text'
import {
  pdfNarrationToReadableSegments,
  pdfSourceSpansForChunk,
} from './pdfTts'
import { mergePdfTtsHighlightRects } from './pdfTtsHighlight'

describe('pdfNarrationToReadableSegments', () => {
  it('keeps reconstructed text and source runs in stable order', () => {
    expect(pdfNarrationToReadableSegments([
      {
        text: 'First paragraph.',
        sourceRuns: [{
          pageIndex: 0,
          blockOrder: 2,
          startOffset: 0,
          endOffset: 16,
          sourceStartOffset: 0,
          sourceEndOffset: 16,
        }],
      },
      { text: '  ', sourceRuns: [] },
      { text: 'Second paragraph.', sourceRuns: [] },
    ])).toEqual([
      {
        text: 'First paragraph.',
        kind: 'paragraph',
        sourceRuns: [{
          pageIndex: 0,
          blockOrder: 2,
          startOffset: 0,
          endOffset: 16,
          sourceStartOffset: 0,
          sourceEndOffset: 16,
        }],
      },
      { text: 'Second paragraph.', kind: 'paragraph', sourceRuns: [] },
    ])
  })

  it('keeps reconstructed prose within audiobook save limits', () => {
    const text = `${'A long reconstructed PDF sentence keeps every source word intact, '.repeat(14)}then ends.`
    const chunks = chunkAudiobookSaveSegmentsWithSpans(
      pdfNarrationToReadableSegments([{ text, sourceRuns: [] }]),
    )

    expect(chunks.length).toBeGreaterThan(1)
    expect(chunks.every((chunk) => chunk.text.length <= AUDIOBOOK_SAVE_CHUNK_PROFILE.maxChunkLength)).toBe(true)
    expect(chunks.map((chunk) => chunk.text).join(' ')).toBe(text)
  })

  it('maps a chunk across PDF text items and segment boundaries', () => {
    const segments = pdfNarrationToReadableSegments([
      {
        text: 'First line',
        sourceRuns: [{
          pageIndex: 0,
          blockOrder: 3,
          startOffset: 0,
          endOffset: 10,
          sourceStartOffset: 0,
          sourceEndOffset: 10,
        }],
      },
      {
        text: 'final file',
        sourceRuns: [
          {
            pageIndex: 1,
            blockOrder: 0,
            startOffset: 0,
            endOffset: 6,
            sourceStartOffset: 0,
            sourceEndOffset: 6,
          },
          {
            pageIndex: 1,
            blockOrder: 1,
            startOffset: 6,
            endOffset: 8,
            sourceStartOffset: 0,
            sourceEndOffset: 1,
          },
          {
            pageIndex: 1,
            blockOrder: 1,
            startOffset: 8,
            endOffset: 10,
            sourceStartOffset: 1,
            sourceEndOffset: 3,
          },
        ],
      },
    ])

    expect(pdfSourceSpansForChunk(segments, {
      startSegmentIndex: 0,
      startOffset: 6,
      endSegmentIndex: 1,
      endOffset: 9,
    })).toEqual([
      { pageIndex: 0, blockOrder: 3, startOffset: 6, endOffset: 10 },
      { pageIndex: 1, blockOrder: 0, startOffset: 0, endOffset: 6 },
      { pageIndex: 1, blockOrder: 1, startOffset: 0, endOffset: 1 },
      { pageIndex: 1, blockOrder: 1, startOffset: 1, endOffset: 2 },
    ])
  })
})

describe('mergePdfTtsHighlightRects', () => {
  it('bridges word spaces without joining lines or columns', () => {
    expect(mergePdfTtsHighlightRects([
      { top: 10, right: 30, bottom: 20, left: 10 },
      { top: 10, right: 54, bottom: 20, left: 34 },
      { top: 24, right: 30, bottom: 34, left: 10 },
      { top: 10, right: 230, bottom: 20, left: 210 },
    ])).toEqual([
      { top: 10, right: 54, bottom: 20, left: 10 },
      { top: 10, right: 230, bottom: 20, left: 210 },
      { top: 24, right: 30, bottom: 34, left: 10 },
    ])
  })
})
