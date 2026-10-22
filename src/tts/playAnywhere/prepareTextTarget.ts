import type { ViewerTextTargetResult } from '../../viewers/types'
import { extractReadableSegmentsFromHtml } from '../alignment/readableSegments'
import type { TtsChunk } from '../types'
import { isUserUploadUrl } from '../storage/UserUploads'
import { isUploadedPdfDocumentUrl, type PdfPageTextLayer } from '../../uploads/DocumentUploads'
import type { PdfReadableSegment } from '../../pdf/pdfTts'
import type { ChunkTargetResult } from './sourceChunkResolver'
import { verifiedHtmlResolver, verifiedPdfResolver } from './verifiedSource'

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
  let htmlCache: { chunks: TtsChunk[]; texts: readonly string[]; resolver: ReturnType<typeof verifiedHtmlResolver> } | undefined
  let pdfCache: { chunks: TtsChunk[]; resolver: ReturnType<typeof verifiedPdfResolver> } | undefined
  let pageCache: { pageIndex: number; promise: Promise<PdfPageTextLayer> } | undefined
  const unavailable = (reason: 'stale' | 'not-ready' | 'unmapped' | 'invalid-point') => ({ status: 'unavailable', reason }) as const

  return {
    async resolve(documentUrl: string, capture: ViewerTextTargetResult): Promise<PreparedTextTargetResult> {
      if (!active || documentUrl !== options.documentUrl) return unavailable('stale')
      if (capture.status === 'unavailable') return capture
      const isCurrent = () => active && capture.isCurrent()
      if (!isCurrent()) return unavailable('stale')
      if (!options.chunks && !chunkRequest) {
        const request = options.getChunks()
        chunkRequest = request
        void request.catch(() => { if (chunkRequest === request) chunkRequest = undefined })
      }
      const chunks = options.chunks ?? await chunkRequest!
      if (!isCurrent()) return unavailable('stale')
      let result: ChunkTargetResult
      if ('segmentTexts' in capture) {
        if (isUploadedPdfDocumentUrl(documentUrl)) return unavailable('unmapped')
        if (htmlCache?.chunks !== chunks || htmlCache.texts !== capture.segmentTexts) {
          expectedHtml ??= extractReadableSegmentsFromHtml(options.docContent).map(segment => segment.text)
          const matches = expectedHtml.length === capture.segmentTexts.length &&
            expectedHtml.every((text, index) => text === capture.segmentTexts[index])
          htmlCache = { chunks, texts: capture.segmentTexts, resolver: matches
            ? verifiedHtmlResolver(chunks, capture.segmentTexts, isUserUploadUrl(documentUrl)) : { status: 'unavailable', reason: 'unmapped' } }
        }
        result = htmlCache.resolver.status === 'ready' ? htmlCache.resolver.resolve(capture.point) : htmlCache.resolver
      } else {
        if (!isUploadedPdfDocumentUrl(documentUrl)) return unavailable('unmapped')
        const point = capture.point
        if (![point.pageIndex, point.blockOrder, point.offset].every(value => Number.isSafeInteger(value) && value >= 0) ||
          point.offset >= capture.sourceText.length) return unavailable('invalid-point')
        if (pdfCache?.chunks !== chunks) {
          const segments = await options.getPdfSegments(documentUrl)
          if (!isCurrent()) return unavailable('stale')
          if (pdfCache?.chunks !== chunks) pdfCache = { chunks, resolver: verifiedPdfResolver(chunks, segments) }
        }
        if (pdfCache.resolver.status !== 'ready') return pdfCache.resolver
        if (pageCache?.pageIndex !== point.pageIndex) {
          const request = { pageIndex: point.pageIndex, promise: options.getPdfPageText(documentUrl, point.pageIndex) }
          pageCache = request
          void request.promise.catch(() => { if (pageCache === request) pageCache = undefined })
        }
        const page = await pageCache.promise
        if (!isCurrent()) return unavailable('stale')
        const blocks = page.blocks.filter(block => block.order === point.blockOrder)
        const block = blocks[0]
        // Native import appends hasEOL as a newline; PDF.js's live item excludes that suffix.
        if (page.pageIndex !== point.pageIndex || blocks.length !== 1 || !block ||
          (block.text !== capture.sourceText && !(block.confidence === null && block.text === capture.sourceText + '\n'))) {
          return unavailable('unmapped')
        }
        result = pdfCache.resolver.resolve(point, block.text.length)
      }
      if (!isCurrent()) return unavailable('stale')
      return result.status === 'resolved' ? { ...result, chunks, documentUrl, isCurrent } : result
    },
    dispose() { active = false; chunkRequest = undefined; htmlCache = undefined; pdfCache = undefined; pageCache = undefined },
  }
}
