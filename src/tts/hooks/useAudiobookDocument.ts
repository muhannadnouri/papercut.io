import { useCallback, useEffect, useRef, useState } from 'react'
import {
  pdfNarrationToReadableSegments,
  pdfSourceSpansForChunk,
  type PdfReadableSegment,
} from '../../pdf/pdfTts'
import {
  getUploadedPdfNarrationSegments,
  isUploadedPdfDocumentUrl,
} from '../../uploads/DocumentUploads'
import {
  getImportedAudiobookMetadata,
  type NativeImportedAudiobookMetadata,
} from '../api/nativeTts'
import {
  chunksHaveDurableSourceSpans,
  countChunkSourceSpans,
  graftImportedSourceSpans,
} from '../alignment/importedSourceSpans'
import { logTtsDiagnostic } from '../diagnostics/TtsDiagnostics'
import { isUserUploadUrl } from '../storage/UserUploads'
import { SILMA_MODEL_ID, type TtsChunk } from '../types'
import {
  chunkAudiobookSaveHtmlWithSpans,
  chunkAudiobookSaveSegmentsWithSpans,
  SILMA_AUDIOBOOK_SAVE_CHUNK_PROFILE,
  type SpeechChunk,
} from '../utils/text'

type ImportedHighlightStatus = 'idle' | 'preparing' | 'ready' | 'unavailable'

interface AudiobookDocumentOptions {
  docContent: string
  loadHtmlDocument: (url: string) => Promise<string>
  modelId: string
  selectedDoc: string | null
  onImportedMetadata: (metadata: NativeImportedAudiobookMetadata) => void
}

/** Prepares stable save chunks and restores imported bundle metadata for the open document. */
export function useAudiobookDocument({
  docContent,
  loadHtmlDocument,
  modelId,
  selectedDoc,
  onImportedMetadata,
}: AudiobookDocumentOptions) {
  const [chunks, setChunks] = useState<TtsChunk[] | null>(null)
  const [importedHighlightStatus, setImportedHighlightStatus] = useState<ImportedHighlightStatus>('idle')
  const pdfSegmentsRef = useRef<{
    documentUrl: string
    promise: Promise<PdfReadableSegment[]>
  } | null>(null)

  const getPdfSegments = useCallback((documentUrl: string): Promise<PdfReadableSegment[]> => {
    if (pdfSegmentsRef.current?.documentUrl === documentUrl) {
      return pdfSegmentsRef.current.promise
    }

    const entry = {
      documentUrl,
      promise: getUploadedPdfNarrationSegments(documentUrl).then(pdfNarrationToReadableSegments),
    }
    pdfSegmentsRef.current = entry
    void entry.promise.catch(() => {
      if (pdfSegmentsRef.current === entry) pdfSegmentsRef.current = null
    })
    return entry.promise
  }, [])

  useEffect(() => {
    if (!selectedDoc) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setChunks(null)
      setImportedHighlightStatus('idle')
      return
    }

    let cancelled = false
    let cancelHighlightBuild: (() => void) | null = null
    if (isUploadedPdfDocumentUrl(selectedDoc)) {
      setChunks(null)
      setImportedHighlightStatus('idle')
      void getPdfSegments(selectedDoc)
        .then((segments) => {
          if (!cancelled) setChunks(audiobookSaveChunksFromSegments(segments, modelId))
        })
        .catch((error) => {
          if (cancelled) return
          logTtsDiagnostic('[tts] PDF narration preparation failed', {
            documentUrl: selectedDoc,
            error: error instanceof Error ? error.message : String(error),
          }, 'warn')
          setChunks(null)
        })
      return () => {
        cancelled = true
      }
    }

    if (!docContent) {
      setChunks(null)
      setImportedHighlightStatus('idle')
      return
    }

    if (isUserUploadUrl(selectedDoc)) {
      // Imported bundles play immediately from manifest chunks; old bundles receive DOM spans lazily.
      setChunks(null)
      setImportedHighlightStatus('preparing')
      void getImportedAudiobookMetadata(selectedDoc)
        .then((metadata) => {
          if (cancelled) return
          onImportedMetadata(metadata)
          setChunks(metadata.chunks)
          if (chunksHaveDurableSourceSpans(metadata.chunks)) {
            logTtsDiagnostic('[tts-highlight] imported durable source spans ready', {
              chunks: metadata.chunks.length,
              sourceSpans: countChunkSourceSpans(metadata.chunks),
              modelId: metadata.modelId,
              textPreprocessor: metadata.textPreprocessor,
              documentUrl: selectedDoc,
            })
            setImportedHighlightStatus('ready')
            return
          }
          cancelHighlightBuild = scheduleImportedHighlightBuild(() => {
            if (cancelled) return
            const rebuiltChunks = audiobookSaveChunksFromHtml(docContent, metadata.modelId)
            const graftedChunks = graftImportedSourceSpans(metadata.chunks, rebuiltChunks, {
              documentUrl: selectedDoc,
              modelId: metadata.modelId,
              textPreprocessor: metadata.textPreprocessor,
            })
            if (cancelled) return
            if (graftedChunks) {
              setChunks(graftedChunks)
              setImportedHighlightStatus('ready')
            } else {
              setImportedHighlightStatus('unavailable')
            }
          })
        })
        .catch(() => {
          if (cancelled) return
          setChunks(audiobookSaveChunksFromHtml(docContent, modelId))
          setImportedHighlightStatus('unavailable')
        })
      return () => {
        cancelled = true
        cancelHighlightBuild?.()
      }
    }

    setImportedHighlightStatus('ready')
    setChunks(audiobookSaveChunksFromHtml(docContent, modelId))
  }, [docContent, getPdfSegments, modelId, onImportedMetadata, selectedDoc])

  const getChunksForDocument = useCallback(async (
    documentUrl: string,
    requestedModelId = modelId,
  ): Promise<TtsChunk[]> => {
    if (isUserUploadUrl(documentUrl)) {
      return (await getImportedAudiobookMetadata(documentUrl)).chunks
    }
    if (isUploadedPdfDocumentUrl(documentUrl)) {
      return audiobookSaveChunksFromSegments(
        await getPdfSegments(documentUrl),
        requestedModelId,
      )
    }

    return audiobookSaveChunksFromHtml(await loadHtmlDocument(documentUrl), requestedModelId)
  }, [getPdfSegments, loadHtmlDocument, modelId])

  const getSelectedChunks = useCallback(async (): Promise<TtsChunk[]> => {
    if (!selectedDoc) return []
    return chunks ?? getChunksForDocument(selectedDoc)
  }, [chunks, getChunksForDocument, selectedDoc])

  const reset = useCallback(() => {
    pdfSegmentsRef.current = null
    setChunks(null)
    setImportedHighlightStatus('idle')
  }, [])

  return { chunks, getChunksForDocument, getSelectedChunks, importedHighlightStatus, reset }
}

/** Defers legacy span reconstruction so manifest-backed playback becomes available first. */
function scheduleImportedHighlightBuild(task: () => void): () => void {
  if (window.requestIdleCallback) {
    const handle = window.requestIdleCallback(task, { timeout: 1500 })
    return () => window.cancelIdleCallback(handle)
  }

  const handle = window.setTimeout(task, 0)
  return () => window.clearTimeout(handle)
}

function audiobookSaveChunksFromHtml(html: string, modelId: string): TtsChunk[] {
  const profile = modelId === SILMA_MODEL_ID ? SILMA_AUDIOBOOK_SAVE_CHUNK_PROFILE : undefined
  return buildRuntimeChunks(chunkAudiobookSaveHtmlWithSpans(html, profile), 'save-c')
}

function audiobookSaveChunksFromSegments(
  segments: PdfReadableSegment[],
  modelId: string,
): TtsChunk[] {
  const profile = modelId === SILMA_MODEL_ID ? SILMA_AUDIOBOOK_SAVE_CHUNK_PROFILE : undefined
  const sourceChunks = chunkAudiobookSaveSegmentsWithSpans(segments, profile)
  return buildRuntimeChunks(sourceChunks, 'save-c').map((chunk, index) => ({
    ...chunk,
    pdfSourceSpans: pdfSourceSpansForChunk(segments, sourceChunks[index].sourceSpan),
  }))
}

function buildRuntimeChunks(sourceChunks: SpeechChunk[], prefix: string): TtsChunk[] {
  return sourceChunks.map((chunk, index) => ({
    id: prefix + String(index + 1).padStart(5, '0'),
    text: chunk.text,
    sourceSpan: chunk.sourceSpan,
  }))
}
