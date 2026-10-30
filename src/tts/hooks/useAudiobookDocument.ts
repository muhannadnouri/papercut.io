import {
  useCallback,
  useEffect,
  useRef,
  useState,
} from 'react'
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
import {
  SILMA_MODEL_ID,
  type TtsChunk,
} from '../types'
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
  const [prepared, setPrepared] = useState<{
    documentUrl: string | null;
    docContent: string;
    modelId: string;
    chunks: TtsChunk[];
    source: 'canonical' | 'unverified-import'
  } | null>(null)
  const chunks = prepared?.documentUrl === selectedDoc && prepared.docContent === docContent && prepared.modelId === modelId
    ? prepared.chunks : null
  const canonicalChunks = prepared?.source === 'canonical' ? chunks : null
  const setChunks = useCallback((value: TtsChunk[] | null, source: 'canonical' | 'unverified-import' = 'canonical') => {
    setPrepared(value ? {
      documentUrl: selectedDoc,
      docContent,
      modelId,
      chunks: value,
      source,
    } : null)
  }, [selectedDoc, docContent, modelId])
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
      if (pdfSegmentsRef.current === entry) {
        pdfSegmentsRef.current = null
      }
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
          if (!cancelled) {
            setChunks(audiobookSaveChunksFromSegments(segments, modelId))
          }
        })
        .catch((error) => {
          if (cancelled) {
            return
          }
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
          if (cancelled) {
            return
          }
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
            if (cancelled) {
              return
            }
            const rebuiltChunks = audiobookSaveChunksFromHtml(docContent, metadata.modelId)
            const graftedChunks = graftImportedSourceSpans(metadata.chunks, rebuiltChunks, {
              documentUrl: selectedDoc,
              modelId: metadata.modelId,
              textPreprocessor: metadata.textPreprocessor,
            })
            if (cancelled) {
              return
            }
            if (graftedChunks) {
              setChunks(graftedChunks)
              setImportedHighlightStatus('ready')
            } else {
              setImportedHighlightStatus('unavailable')
            }
          })
        })
        .catch(() => {
          if (cancelled) {
            return
          }
          setChunks(audiobookSaveChunksFromHtml(docContent, modelId), 'unverified-import')
          setImportedHighlightStatus('unavailable')
        })
      return () => {
        cancelled = true
        cancelHighlightBuild?.()
      }
    }

    setImportedHighlightStatus('ready')
    setChunks(audiobookSaveChunksFromHtml(docContent, modelId))
  }, [docContent, getPdfSegments, modelId, onImportedMetadata, selectedDoc, setChunks])

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
    if (!selectedDoc) {
      return []
    }
    return chunks ?? getChunksForDocument(selectedDoc)
  }, [chunks, getChunksForDocument, selectedDoc])

  const getCanonicalChunks = useCallback(async (): Promise<TtsChunk[]> => {
    if (canonicalChunks) {
      return canonicalChunks
    }
    // A failed import read may leave display fallback chunks; target preparation must retry the manifest.
    if (selectedDoc && isUserUploadUrl(selectedDoc)) {
      return getChunksForDocument(selectedDoc)
    }
    return getSelectedChunks()
  }, [canonicalChunks, getChunksForDocument, getSelectedChunks, selectedDoc])

  const reset = useCallback(() => {
    pdfSegmentsRef.current = null
    setPrepared(null)
    setImportedHighlightStatus('idle')
  }, [])

  return {
    chunks,
    canonicalChunks,
    getCanonicalChunks,
    getChunksForDocument,
    getSelectedChunks,
    getPdfSegments,
    importedHighlightStatus,
    reset,
  }
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
  return warnMissingSourceSpans(
    buildRuntimeChunks(chunkAudiobookSaveHtmlWithSpans(html, profile), 'save-c'),
    'html',
  )
}

function audiobookSaveChunksFromSegments(
  segments: PdfReadableSegment[],
  modelId: string,
): TtsChunk[] {
  const profile = modelId === SILMA_MODEL_ID ? SILMA_AUDIOBOOK_SAVE_CHUNK_PROFILE : undefined
  const sourceChunks = chunkAudiobookSaveSegmentsWithSpans(segments, profile)
  return warnMissingSourceSpans(buildRuntimeChunks(sourceChunks, 'save-c').map((chunk, index) => ({
    ...chunk,
    pdfSourceSpans: pdfSourceSpansForChunk(segments, sourceChunks[index].sourceSpan),
  })), 'pdf')
}

/** Spanless chunks narrate fine but cannot route play-from-here; say so while
 * the book is being saved instead of failing silently at playback time. */
function warnMissingSourceSpans(chunks: TtsChunk[], source: 'html' | 'pdf'): TtsChunk[] {
  const missing = chunks.filter(chunk => chunk.text.trim() && !chunk.sourceSpan).length
  if (missing > 0) {
    logTtsDiagnostic('[tts-save] chunks without source spans degrade play-from-here', {
      chunks: chunks.length,
      missing,
      source,
    }, 'warn')
  }
  return chunks
}

function buildRuntimeChunks(sourceChunks: SpeechChunk[], prefix: string): TtsChunk[] {
  return sourceChunks.map((chunk, index) => ({
    id: prefix + String(index + 1).padStart(5, '0'),
    text: chunk.text,
    sourceSpan: chunk.sourceSpan,
  }))
}
