// @vitest-environment jsdom
import {
  afterEach,
  describe,
  expect,
  it,
  vi,
} from 'vitest'
import {
  createTextTargetPreparation,
  type TextTargetPreparationOptions,
} from './prepareTextTarget'
import { getUploadedPdfPageText } from '../../uploads/DocumentUploads'
import type { ViewerTextTargetResult } from '../../viewers/types'
import type { TtsChunk } from '../types'
import { pdfNarrationToReadableSegments } from '../../pdf/pdfTts'
import * as verification from './verifiedSource'
import { logTtsDiagnostic } from '../diagnostics/TtsDiagnostics'

vi.mock('../diagnostics/TtsDiagnostics', () => ({ logTtsDiagnostic: vi.fn() }))

vi.mock('../../uploads/DocumentUploads', async importOriginal => ({
  ...await importOriginal<typeof import('../../uploads/DocumentUploads')>(),
  getUploadedPdfPageText: vi.fn(),
}))
afterEach(() => {
  vi.restoreAllMocks()
  vi.mocked(getUploadedPdfPageText).mockReset()
})

const chunks: TtsChunk[] = [
  {
    id: 'a',
    text: 'First.',
    sourceSpan: {
      startSegmentIndex: 0,
      endSegmentIndex: 0,
      startOffset: 0,
      endOffset: 6,
    },
  },
  {
    id: 'b',
    text: 'Second.',
    sourceSpan: {
      startSegmentIndex: 1,
      endSegmentIndex: 1,
      startOffset: 0,
      endOffset: 7,
    },
  },
]
const texts = Object.freeze(['First.', 'Second.'])
function htmlCapture(segmentTexts: readonly string[] = texts): ViewerTextTargetResult {
  return {
    status: 'captured',
    point: {
      format: 'html',
      segmentIndex: 1,
      offset: 2,
    },
    segmentTexts,
    segmentLengths: segmentTexts.map(text => text.length),
    isCurrent: () => true,
  }
}
function options(extra: Partial<TextTargetPreparationOptions> = {}): TextTargetPreparationOptions {
  return {
    documentUrl: '/book.html',
    docContent: '<h1>First.</h1><p>Second.</p>',
    chunks,
    getChunks: vi.fn(async () => chunks),
    getPdfPageText: getUploadedPdfPageText,
    getPdfSegments: vi.fn(async () => []),
    ...extra,
  }
}
function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (error: Error) => void
  const promise = new Promise<T>((yes, no) => {
    resolve = yes
    reject = no
  })
  return {
    promise,
    resolve,
    reject,
  }
}

describe('source-only target preparation', () => {
  it('returns original prepared chunks before playback, caches per view/chunk revision and invalidates results on disposal', async () => {
    const verify = vi.spyOn(verification, 'htmlResolverSteps')
    const config = options()
    const preparation = createTextTargetPreparation(config)
    const first = await preparation.resolve('/book.html', htmlCapture())
    expect(first).toMatchObject({
      status: 'resolved',
      chunkIndex: 1,
      chunkId: 'b',
      documentUrl: '/book.html',
    })
    if (first.status !== 'resolved') {
      return
    }
    expect(first.chunks).toBe(chunks)
    expect(config.getChunks).not.toHaveBeenCalled()
    await preparation.resolve('/book.html', htmlCapture())
    expect(verify).toHaveBeenCalledTimes(1)
    await preparation.resolve('/book.html', htmlCapture([...texts]))
    expect(verify).toHaveBeenCalledTimes(2)
    expect(first.isCurrent()).toBe(true)
    preparation.dispose()
    expect(first.isCurrent()).toBe(false)
    expect(await preparation.resolve('/book.html', htmlCapture())).toMatchObject({ reason: 'stale' })
  })

  it('coalesces cold work, cancels replaced intent and uses the warm cache without timers', async () => {
    vi.useFakeTimers()
    const verify = vi.spyOn(verification, 'htmlResolverSteps')
    const preparation = createTextTargetPreparation(options())
    try {
      const controller = new AbortController()
      const old = preparation.resolve('/book.html', htmlCapture(), controller.signal)
      expect(verify).not.toHaveBeenCalled()
      controller.abort()
      const current = preparation.resolve('/book.html', htmlCapture())
      const shared = preparation.resolve('/book.html', htmlCapture())
      await vi.runAllTimersAsync()
      expect(await old).toMatchObject({ reason: 'stale' })
      expect(await current).toMatchObject({ chunkIndex: 1 })
      expect(await shared).toMatchObject({ chunkIndex: 1 })
      expect(verify).toHaveBeenCalledOnce()
      expect(vi.getTimerCount()).toBe(0)
      expect(await preparation.resolve('/book.html', htmlCapture())).toMatchObject({ chunkIndex: 1 })
      expect(vi.getTimerCount()).toBe(0)
      expect(verify).toHaveBeenCalledOnce()
    } finally {
      preparation.dispose()
      vi.useRealTimers()
    }
  })

  it.each(['dispose', 'view-change'])('cancels between cold batches without publishing partial state: %s', async change => {
    vi.useFakeTimers()
    const source = Array.from({ length: 1000 }, (_, i) => `Paragraph ${i} has narrated words.`)
    const saved = source.map((text, index) => ({
      id: String(index),
      text,
      sourceSpan: {
        startSegmentIndex: index,
        endSegmentIndex: index,
        startOffset: 0,
        endOffset: text.length,
      },
    }))
    const preparation = createTextTargetPreparation(options({
      chunks: saved,
      docContent: source.map(text => `<p>${text}</p>`).join(''),
    }))
    let current = true
    const capture = htmlCapture(source)
    if (capture.status === 'captured') {
      capture.isCurrent = () => current
    }
    try {
      const pending = preparation.resolve('/book.html', capture)
      await vi.advanceTimersToNextTimerAsync()
      expect(vi.getTimerCount()).toBe(1)
      if (change === 'dispose') {
        preparation.dispose()
      } else {
        current = false
      }
      await vi.runAllTimersAsync()
      expect(await pending).toMatchObject({ reason: 'stale' })
      expect(vi.getTimerCount()).toBe(0)
      if (change === 'view-change') {
        current = true
        const retry = preparation.resolve('/book.html', capture)
        await vi.runAllTimersAsync()
        expect(await retry).toMatchObject({ chunkIndex: 1 })
      }
    } finally {
      preparation.dispose()
      vi.useRealTimers()
    }
  })

  it('shares canonical chunk preparation while pending and permits retry after manifest failure', async () => {
    const config = options({ chunks: null })
    const preparation = createTextTargetPreparation(config)
    const [a, b] = await Promise.all([preparation.resolve('/book.html', htmlCapture()), preparation.resolve('/book.html', htmlCapture())])
    expect(a.status).toBe('resolved')
    expect(b.status).toBe('resolved')
    expect(config.getChunks).toHaveBeenCalledOnce()
    const retry = options({
      chunks: null,
      getChunks: vi.fn().mockRejectedValueOnce(new Error('manifest failed')).mockResolvedValue(chunks),
    })
    const second = createTextTargetPreparation(retry)
    await expect(second.resolve('/book.html', htmlCapture())).rejects.toThrow('manifest failed')
    expect((await second.resolve('/book.html', htmlCapture())).status).toBe('resolved')
  })

  it('requires live text identity, not equal lengths; rejects wrong documents, unavailable and stale views', async () => {
    const preparation = createTextTargetPreparation(options())
    expect(await preparation.resolve('/book.html', htmlCapture(['First.', 'Changed']))).toMatchObject({ reason: 'unmapped' })
    expect(await preparation.resolve('/different.html', htmlCapture())).toMatchObject({ reason: 'stale' })
    expect(await preparation.resolve('/book.html', {
      status: 'unavailable',
      reason: 'no-visible-text',
    })).toEqual({
      status: 'unavailable',
      reason: 'no-visible-text',
    })
    const capture = htmlCapture()
    if (capture.status === 'captured') {
      capture.isCurrent = () => false
    }
    expect(await preparation.resolve('/book.html', capture)).toMatchObject({ reason: 'stale' })
  })

  it('identifies source, chunk and point failures without logging private text or IDs', async () => {
    const preparation = createTextTargetPreparation(options())
    await preparation.resolve('/book.html', htmlCapture(['First.', 'Changed']))
    expect(logTtsDiagnostic).toHaveBeenLastCalledWith('[play-anywhere] HTML target rejected', {
      stage: 'source-identity',
      reason: 'unmapped',
      chunks: 2,
      sourceSegments: 2,
      viewSegments: 2,
    })
    // One stale span degrades to a localized gap; the rest of the book resolves.
    await createTextTargetPreparation(options({
      chunks: [{
        ...chunks[0],
        text: 'Changed',
      }, chunks[1]],
    })).resolve('/book.html', htmlCapture())
    expect(logTtsDiagnostic).toHaveBeenLastCalledWith('[play-anywhere] preparation completed', expect.anything())
    // With no verifiable span left, the whole book is rejected again.
    await createTextTargetPreparation(options({
      chunks: [{
        ...chunks[0],
        text: 'Changed',
      }, {
        ...chunks[1],
        text: 'Rewritten',
      }],
    })).resolve('/book.html', htmlCapture())
    expect(logTtsDiagnostic).toHaveBeenLastCalledWith('[play-anywhere] HTML target rejected', {
      stage: 'chunk-verification',
      reason: 'not-ready',
      chunks: 2,
      sourceSegments: 2,
      viewSegments: 2,
    })
    await preparation.resolve('/book.html', {
      ...htmlCapture(),
      point: {
        format: 'html',
        segmentIndex: 9,
        offset: 0,
      },
    } as ViewerTextTargetResult)
    expect(logTtsDiagnostic).toHaveBeenLastCalledWith('[play-anywhere] HTML target rejected', {
      stage: 'point-lookup',
      reason: 'invalid-point',
      chunks: 2,
      sourceSegments: 2,
      viewSegments: 2,
    })
    preparation.dispose()
  })

  it('recovers imported metadata without re-chunking or mutating authoritative IDs, text, hashes or spans', async () => {
    const imported = chunks.map(chunk => ({
      id: 'import-' + chunk.id,
      text: chunk.text,
      textHash: 'saved-' + chunk.id,
    }))
    const config = options({
      documentUrl: '/user-uploads/ab.html',
      chunks: imported,
    })
    const preparation = createTextTargetPreparation(config)
    const result = await preparation.resolve('/user-uploads/ab.html', htmlCapture())
    expect(result).toMatchObject({
      status: 'resolved',
      chunkIndex: 1,
      chunkId: 'import-b',
    })
    if (result.status === 'resolved') {
      expect(result.chunks).toBe(imported)
    }
    expect(imported.every(chunk => !('sourceSpan' in chunk))).toBe(true)
  })

  it.each(['dispose', 'view-change'])('rejects replacement during awaited preparation: %s', async change => {
    const pending = deferred<TtsChunk[]>()
    const config = options({
      chunks: null,
      getChunks: () => pending.promise,
    })
    const preparation = createTextTargetPreparation(config)
    let current = true
    const capture = htmlCapture()
    if (capture.status === 'captured') {
      capture.isCurrent = () => current
    }
    const result = preparation.resolve('/book.html', capture)
    if (change === 'dispose') {
      preparation.dispose()
    } else {
      current = false
    }
    pending.resolve(chunks)
    expect(await result).toMatchObject({ reason: 'stale' })
  })

  const pdfChunks: TtsChunk[] = [{
    id: 'pdf',
    text: 'office',
    sourceSpan: {
      startSegmentIndex: 0,
      endSegmentIndex: 0,
      startOffset: 0,
      endOffset: 6,
    },
    pdfSourceSpans: [{
      pageIndex: 0,
      blockOrder: 4,
      startOffset: 0,
      endOffset: 4,
    }],
  }]
  const segments = pdfNarrationToReadableSegments([{
    text: 'office',
    sourceRuns: [{
      pageIndex: 0,
      blockOrder: 4,
      startOffset: 0,
      endOffset: 6,
      sourceStartOffset: 0,
      sourceEndOffset: 4,
    }],
  }])
  const pdfCapture: ViewerTextTargetResult = {
    status: 'captured',
    point: {
      format: 'pdf',
      pageIndex: 0,
      blockOrder: 4,
      offset: 1,
    },
    sourceText: 'oﬃce',
    isCurrent: () => true,
  }
  const page = {
    schemaVersion: 1 as const,
    pageIndex: 0,
    width: 612,
    height: 792,
    blocks: [{
      order: 4,
      text: 'oﬃce\n',
      bounds: [0, 0, 50, 20] as [number, number, number, number],
      confidence: null,
    }],
  }

  it('verifies PDF original item text and native EOL while retaining nonlinear glyph mapping and bounded page cache', async () => {
    vi.mocked(getUploadedPdfPageText).mockResolvedValue(page)
    const verify = vi.spyOn(verification, 'pdfResolverSteps')
    const config = options({
      documentUrl: '/uploads/ab.pdf',
      chunks: pdfChunks,
      getPdfSegments: vi.fn(async () => segments),
    })
    const preparation = createTextTargetPreparation(config)
    const results = await Promise.all([preparation.resolve('/uploads/ab.pdf', pdfCapture), preparation.resolve('/uploads/ab.pdf', pdfCapture)])
    expect(results.every(result => result.status === 'resolved')).toBe(true)
    expect(results[0]).toMatchObject({
      status: 'resolved',
      chunkId: 'pdf',
      chunkIndex: 0,
    })
    await preparation.resolve('/uploads/ab.pdf', pdfCapture)
    expect(config.getPdfSegments).toHaveBeenCalledTimes(2)
    expect(verify).toHaveBeenCalledOnce()
    expect(getUploadedPdfPageText).toHaveBeenCalledOnce()
    expect(await preparation.resolve('/uploads/ab.pdf', {
      ...pdfCapture,
      sourceText: 'fake',
    })).toMatchObject({ reason: 'unmapped' })
    const otherPage = {
      ...page,
      pageIndex: 1,
    }
    vi.mocked(getUploadedPdfPageText).mockResolvedValue(otherPage)
    await preparation.resolve('/uploads/ab.pdf', {
      ...pdfCapture,
      point: {
        format: 'pdf',
        pageIndex: 1,
        blockOrder: 4,
        offset: 1,
      },
    })
    expect(getUploadedPdfPageText).toHaveBeenCalledTimes(2)
  })

  it('keeps OCR newlines exact and refuses duplicate blocks and out-of-bounds source spans', async () => {
    const config = options({
      documentUrl: '/uploads/ab.pdf',
      chunks: pdfChunks,
      getPdfSegments: async () => segments,
    })
    vi.mocked(getUploadedPdfPageText).mockResolvedValue({
      ...page,
      blocks: [{
        ...page.blocks[0],
        confidence: 0.9,
      }],
    })
    expect(await createTextTargetPreparation(config).resolve('/uploads/ab.pdf', pdfCapture)).toMatchObject({ reason: 'unmapped' })
    vi.mocked(getUploadedPdfPageText).mockResolvedValue({
      ...page,
      blocks: [page.blocks[0], page.blocks[0]],
    })
    expect(await createTextTargetPreparation(config).resolve('/uploads/ab.pdf', pdfCapture)).toMatchObject({ reason: 'unmapped' })
    const oversized = [{
      ...pdfChunks[0],
      pdfSourceSpans: [{
        pageIndex: 0,
        blockOrder: 4,
        startOffset: 0,
        endOffset: 9,
      }],
    }]
    expect(await createTextTargetPreparation({
      ...config,
      chunks: oversized,
    }).resolve('/uploads/ab.pdf', pdfCapture)).toMatchObject({ reason: 'invalid-source' })
  })

  it('rejects a replaced view during PDF narration preparation before requesting page text', async () => {
    const pending = deferred<typeof segments>()
    const preparation = createTextTargetPreparation(options({
      documentUrl: '/uploads/ab.pdf',
      chunks: pdfChunks,
      getPdfSegments: () => pending.promise,
    }))
    const result = preparation.resolve('/uploads/ab.pdf', pdfCapture)
    preparation.dispose()
    pending.resolve(segments)
    expect(await result).toMatchObject({ reason: 'stale' })
    expect(getUploadedPdfPageText).not.toHaveBeenCalled()
  })

  it('checks current view after PDF source and page awaits and permits retry after a page-read error', async () => {
    const pending = deferred<typeof page>()
    vi.mocked(getUploadedPdfPageText).mockReturnValue(pending.promise)
    const config = options({
      documentUrl: '/uploads/ab.pdf',
      chunks: pdfChunks,
      getPdfSegments: async () => segments,
    })
    const preparation = createTextTargetPreparation(config)
    const promise = preparation.resolve('/uploads/ab.pdf', pdfCapture)
    await vi.waitFor(() => expect(getUploadedPdfPageText).toHaveBeenCalled())
    preparation.dispose()
    pending.resolve(page)
    expect(await promise).toMatchObject({ reason: 'stale' })
    vi.mocked(getUploadedPdfPageText).mockRejectedValueOnce(new Error('read failed')).mockResolvedValue(page)
    const retry = createTextTargetPreparation(config)
    await expect(retry.resolve('/uploads/ab.pdf', pdfCapture)).rejects.toThrow('read failed')
    expect(await retry.resolve('/uploads/ab.pdf', pdfCapture)).toMatchObject({ status: 'resolved' })
  })
})
