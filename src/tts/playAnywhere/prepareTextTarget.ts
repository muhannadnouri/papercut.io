import type { ViewerTextTargetResult } from '../../viewers/types'
import { extractReadableSegmentsFromHtml } from '../alignment/readableSegments'
import type { TtsChunk } from '../types'
import { isUserUploadUrl } from '../storage/UserUploads'
import {
  isUploadedPdfDocumentUrl,
  type PdfPageTextLayer,
} from '../../uploads/DocumentUploads'
import type { PdfReadableSegment } from '../../pdf/pdfTts'
import type { ChunkTargetResult } from './sourceChunkResolver'
import {
  htmlResolverSteps,
  pdfResolverSteps,
  type verifiedHtmlResolver,
  type verifiedPdfResolver,
} from './verifiedSource'
import { runPreparationWork } from './preparationWork'
import { logTtsDiagnostic } from '../diagnostics/TtsDiagnostics'

export interface TextTargetPreparationOptions {
  documentUrl: string | null
  docContent: string
  chunks: TtsChunk[] | null
  getChunks: () => Promise<TtsChunk[]>
  getPdfSegments: (documentUrl: string) => Promise<PdfReadableSegment[]>
  getPdfPageText: (documentUrl: string, pageIndex: number) => Promise<PdfPageTextLayer>
}

export type PreparedTextTargetResult =
  | (Extract<ChunkTargetResult, { status: 'resolved' }> & {
    documentUrl: string
    chunks: readonly TtsChunk[]
    isCurrent: () => boolean
  })
  | Extract<ChunkTargetResult, { status: 'unavailable' }>
  | Extract<ViewerTextTargetResult, { status: 'unavailable' }>

/** Source-only boundary: no player, audio loading, settings, native playback or DOM mutations. */
export function createTextTargetPreparation(options: TextTargetPreparationOptions) {
  let active = true
  let chunkRequest: Promise<TtsChunk[]> | undefined
  let expectedHtml: string[] | undefined
  type HtmlCache = { chunks: TtsChunk[];
    texts: readonly string[];
    sourceMatches: boolean;
    resolver: ReturnType<typeof verifiedHtmlResolver> }
  type PdfCache = { chunks: TtsChunk[];
    resolver: ReturnType<typeof verifiedPdfResolver> }
  let htmlCache: HtmlCache | undefined
  let pdfCache: PdfCache | undefined
  const lifetime = new AbortController()
  function start<T>(steps: Generator<void, T>, current: () => boolean, signal?: AbortSignal) {
    const controller = new AbortController()
    const valid = () => active && current() && !controller.signal.aborted
    return {
      current: valid,
      cancel: () => controller.abort(),
      promise: runPreparationWork(steps, valid, [lifetime.signal, signal, controller.signal]),
    }
  }
  let htmlPending: { chunks: TtsChunk[];
    texts: readonly string[];
    work: ReturnType<typeof start<HtmlCache>> } | undefined
  let pdfPending: { chunks: TtsChunk[];
    work: ReturnType<typeof start<PdfCache>> } | undefined
  function* prepareHtml(chunks: TtsChunk[], texts: readonly string[]): Generator<void, HtmlCache> {
    // shortcut: HTML extraction is atomic; revisit if packaged cold timings still stall input.
    expectedHtml ??= extractReadableSegmentsFromHtml(options.docContent).map(segment => segment.text)
    yield
    let matches = expectedHtml.length === texts.length
    for (let index = 0; matches && index < expectedHtml.length; index++) {
      matches = expectedHtml[index] === texts[index]
      yield
    }
    return {
      chunks,
      texts,
      sourceMatches: matches,
      resolver: matches
        ? yield* htmlResolverSteps(chunks, texts, isUserUploadUrl(options.documentUrl!)) : {
          status: 'unavailable',
          reason: 'unmapped',
        },
    }
  }
  function* preparePdf(chunks: TtsChunk[], segments: PdfReadableSegment[]): Generator<void, PdfCache> {
    return {
      chunks,
      resolver: yield* pdfResolverSteps(chunks, segments),
    }
  }
  let pageCache: { pageIndex: number;
    promise: Promise<PdfPageTextLayer> } | undefined
  const unavailable = (reason: 'stale' | 'not-ready' | 'unmapped' | 'invalid-point') => ({
    status: 'unavailable',
    reason,
  }) as const

  return {
    async resolve(documentUrl: string, capture: ViewerTextTargetResult, signal?: AbortSignal): Promise<PreparedTextTargetResult> {
      if (!active || documentUrl !== options.documentUrl) {
        return unavailable('stale')
      }
      if (capture.status === 'unavailable') {
        return capture
      }
      const isCurrent = () => active && !signal?.aborted && capture.isCurrent()
      if (!isCurrent()) {
        return unavailable('stale')
      }
      try {
        if (!options.chunks && !chunkRequest) {
          const request = options.getChunks()
          chunkRequest = request
          void request.catch(() => {
            if (chunkRequest === request) {
              chunkRequest = undefined
            }
          })
        }
        const chunks = options.chunks ?? await chunkRequest!
        if (!isCurrent()) {
          return unavailable('stale')
        }
        let result: ChunkTargetResult
        if ('segmentTexts' in capture) {
          if (isUploadedPdfDocumentUrl(documentUrl)) {
            return unavailable('unmapped')
          }
          if (htmlCache?.chunks !== chunks || htmlCache.texts !== capture.segmentTexts) {
            if (htmlPending && (htmlPending.chunks !== chunks || htmlPending.texts !== capture.segmentTexts || !htmlPending.work.current())) {
              htmlPending.work.cancel(); htmlPending = undefined
            }
            if (!htmlPending) {
              const work = start(prepareHtml(chunks, capture.segmentTexts), isCurrent, signal)
              const pending = {
                chunks,
                texts: capture.segmentTexts,
                work,
              }
              htmlPending = pending
              work.promise = work.promise.then(cache => {
                if (!work.current()) {
                  throw new DOMException('Preparation cancelled', 'AbortError')
                }
                htmlCache = cache; return cache
              }).finally(() => {
                if (htmlPending === pending) {
                  htmlPending = undefined
                }
              })
            }
            await htmlPending.work.promise
            if (!isCurrent()) {
              return unavailable('stale')
            }
          }
          const cache = htmlCache!
          result = cache.resolver.status === 'ready' ? cache.resolver.resolve(capture.point) : cache.resolver
          if (result.status === 'unavailable') {
            logTtsDiagnostic('[play-anywhere] HTML target rejected', {
              stage: !cache.sourceMatches ? 'source-identity' : cache.resolver.status !== 'ready' ? 'chunk-verification' : 'point-lookup',
              reason: result.reason,
              chunks: chunks.length,
              sourceSegments: expectedHtml!.length,
              viewSegments: capture.segmentTexts.length,
            })
          }
        } else {
          if (!isUploadedPdfDocumentUrl(documentUrl)) {
            return unavailable('unmapped')
          }
          const point = capture.point
          if (![point.pageIndex, point.blockOrder, point.offset].every(value => Number.isSafeInteger(value) && value >= 0) ||
            point.offset >= capture.sourceText.length) {
            return unavailable('invalid-point')
          }
          if (pdfCache?.chunks !== chunks) {
            const segments = await options.getPdfSegments(documentUrl)
            if (!isCurrent()) {
              return unavailable('stale')
            }
            if (pdfCache?.chunks !== chunks) {
              if (pdfPending && (pdfPending.chunks !== chunks || !pdfPending.work.current())) {
                pdfPending.work.cancel(); pdfPending = undefined
              }
              if (!pdfPending) {
                const work = start(preparePdf(chunks, segments), isCurrent, signal)
                const pending = {
                  chunks,
                  work,
                }; pdfPending = pending
                work.promise = work.promise.then(cache => {
                  if (!work.current()) {
                    throw new DOMException('Preparation cancelled', 'AbortError')
                  }
                  pdfCache = cache; return cache
                }).finally(() => {
                  if (pdfPending === pending) {
                    pdfPending = undefined
                  }
                })
              }
              await pdfPending.work.promise
              if (!isCurrent()) {
                return unavailable('stale')
              }
            }
          }
          const cache = pdfCache!
          if (cache.resolver.status !== 'ready') {
            return cache.resolver
          }
          if (pageCache?.pageIndex !== point.pageIndex) {
            const request = {
              pageIndex: point.pageIndex,
              promise: options.getPdfPageText(documentUrl, point.pageIndex),
            }
            pageCache = request
            void request.promise.catch(() => {
              if (pageCache === request) {
                pageCache = undefined
              }
            })
          }
          const page = await pageCache.promise
          if (!isCurrent()) {
            return unavailable('stale')
          }
          const blocks = page.blocks.filter(block => block.order === point.blockOrder)
          const block = blocks[0]
          // Native import appends hasEOL as a newline; PDF.js's live item excludes that suffix.
          if (page.pageIndex !== point.pageIndex || blocks.length !== 1 || !block ||
            (block.text !== capture.sourceText && !(block.confidence === null && block.text === capture.sourceText + '\n'))) {
            return unavailable('unmapped')
          }
          result = cache.resolver.resolve(point, block.text.length)
        }
        if (!isCurrent()) {
          return unavailable('stale')
        }
        return result.status === 'resolved' ? {
          ...result,
          chunks,
          documentUrl,
          isCurrent,
        } : result
      } catch (error) {
        if (!isCurrent() || error instanceof DOMException && error.name === 'AbortError') {
          return unavailable('stale')
        }
        throw error
      }
    },
    dispose() {
      active = false; lifetime.abort(); htmlPending = undefined; pdfPending = undefined; chunkRequest = undefined; htmlCache = undefined; pdfCache = undefined; pageCache = undefined
    },
  }
}
