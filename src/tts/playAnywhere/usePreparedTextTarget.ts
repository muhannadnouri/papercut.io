import {
  useCallback,
  useLayoutEffect,
  useRef,
} from 'react'
import type { ViewerTextTargetResult } from '../../viewers/types'
import { getUploadedPdfPageText } from '../../uploads/DocumentUploads'
import {
  createTextTargetPreparation,
  type TextTargetPreparationOptions,
  type PreparedTextTargetResult,
} from './prepareTextTarget'

/** Own validation caches for exactly one committed document/model/chunk revision. */
export function usePreparedTextTarget(options: Omit<TextTargetPreparationOptions, 'getPdfPageText'> & { modelId: string }) {
  const { documentUrl, docContent, chunks, getChunks, getPdfSegments, modelId } = options
  const current = useRef<ReturnType<typeof createTextTargetPreparation> | null>(null)
  useLayoutEffect(() => {
    const preparation = createTextTargetPreparation({
      documentUrl,
      docContent,
      chunks,
      getChunks,
      getPdfSegments,
      getPdfPageText: getUploadedPdfPageText,
    })
    current.current = preparation
    return () => {
      preparation.dispose()
      current.current = null
    }
  }, [documentUrl, docContent, chunks, getChunks, getPdfSegments, modelId])
  return useCallback((url: string, capture: ViewerTextTargetResult, signal?: AbortSignal): Promise<PreparedTextTargetResult> =>
    current.current?.resolve(url, capture, signal) ?? Promise.resolve({
      status: 'unavailable',
      reason: 'not-ready',
    }), [])
}
