import { useCallback, useEffect, useState } from 'react'
import {
  deleteTranslatedDocument,
  getTranslationModelStatus,
  installTranslationModel,
  cancelTranslationJob,
  listenTranslationModelInstallProgress,
  listenTranslationProgress,
  getTranslationCapabilities,
  listTranslatedDocuments,
  startTranslationJob,
  type TranslatedDocumentInfo,
  type TranslationCapabilities,
  type TranslationDeleteResult,
  type TranslationJobProgress,
  type TranslationModelInstallProgress,
  type TranslationModelInstallResult,
  type TranslationModelStatus,
  type TranslationStartRequest,
  type TranslationStartResult,
} from '../api/nativeTranslation'
import { translationErrorMessage, translationLibraryRefreshError } from '../utils/errors'

interface TranslationStartState {
  cancelling: boolean
  checking: boolean
  jobId: string
  progress: TranslationJobProgress | null
  result: TranslationStartResult | null
  message: string
}

interface TranslationModelInstallState {
  installingModelId: string
  progress: TranslationModelInstallProgress | null
  result: TranslationModelInstallResult | null
  message: string
}

interface TranslationManagerState {
  capabilities: TranslationCapabilities | null
  deleteState: TranslationDeleteResult | null
  error: string
  loading: boolean
  modelInstallState: TranslationModelInstallState
  modelStatuses: Record<string, TranslationModelStatus>
  startState: TranslationStartState
  translatedDocuments: TranslatedDocumentInfo[]
  onDeleteTranslatedDocument: (id: string) => Promise<void>
  onInstallTranslationModel: (modelId: string) => Promise<void>
  onCancelTranslation: () => Promise<void>
  onStartTranslationPreflight: (request: TranslationStartRequest) => Promise<void>
  refresh: () => Promise<void>
}

interface TranslationManagerOptions {
  enabled: boolean
  onDocumentLibraryChanged?: (changedDocumentUrl?: string) => Promise<void>
}

export function useTranslationManager({
  enabled,
  onDocumentLibraryChanged,
}: TranslationManagerOptions): TranslationManagerState {
  const [capabilities, setCapabilities] = useState<TranslationCapabilities | null>(null)
  const [translatedDocuments, setTranslatedDocuments] = useState<TranslatedDocumentInfo[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [deleteState, setDeleteState] = useState<TranslationDeleteResult | null>(null)
  const [modelStatuses, setModelStatuses] = useState<Record<string, TranslationModelStatus>>({})
  const [modelInstallState, setModelInstallState] = useState<TranslationModelInstallState>({
    installingModelId: '',
    progress: null,
    result: null,
    message: '',
  })
  const [startState, setStartState] = useState<TranslationStartState>({
    cancelling: false,
    checking: false,
    jobId: '',
    progress: null,
    result: null,
    message: '',
  })

  const refresh = useCallback(async () => {
    setLoading(true)
    setError('')
    try {
      const [nextCapabilities, nextDocuments] = await Promise.all([
        getTranslationCapabilities(),
        listTranslatedDocuments(),
      ])
      const statusEntries = await Promise.all(
        nextCapabilities.models.map(async (model) => [model.id, await getTranslationModelStatus(model.id)] as const),
      )
      setCapabilities(nextCapabilities)
      setTranslatedDocuments(nextDocuments)
      setModelStatuses(Object.fromEntries(statusEntries))
    } catch (err) {
      setError(translationErrorMessage(err))
    } finally {
      setLoading(false)
    }
  }, [])

  const refreshGeneratedDocuments = useCallback(async (changedDocumentUrl?: string) => {
    await refresh()
    if (!onDocumentLibraryChanged) return
    try {
      await onDocumentLibraryChanged(changedDocumentUrl)
    } catch (err) {
      setError(translationLibraryRefreshError(err))
    }
  }, [onDocumentLibraryChanged, refresh])

  useEffect(() => {
    if (!enabled) return
    void refresh()
  }, [enabled, refresh])

  useEffect(() => {
    if (!enabled) return
    let disposed = false
    let unlisten: (() => void) | null = null
    void listenTranslationModelInstallProgress((progress) => {
      if (disposed) return
      setModelInstallState((current) => ({
        ...current,
        installingModelId: progress.status === 'installed' ? '' : progress.modelId,
        progress,
        message: progress.message,
      }))
      setModelStatuses((current) => {
        const existing = current[progress.modelId]
        if (!existing) return current
        return {
          ...current,
          [progress.modelId]: {
            ...existing,
            installing: progress.status !== 'installed',
            installed: progress.status === 'installed' ? true : existing.installed,
            installedBytes: progress.status === 'installed' ? progress.totalBytes : existing.installedBytes,
          },
        }
      })
    }).then((cleanup) => {
      if (disposed) cleanup()
      else unlisten = cleanup
    }).catch((err) => {
      if (!disposed) setError(translationErrorMessage(err))
    })
    return () => {
      disposed = true
      if (unlisten) unlisten()
    }
  }, [enabled])

  useEffect(() => {
    if (!enabled) return
    let disposed = false
    let unlisten: (() => void) | null = null
    void listenTranslationProgress((progress) => {
      if (disposed) return
      setStartState((current) => {
        if (current.jobId && progress.jobId !== current.jobId) return current
        return {
          ...current,
          progress,
          message: progress.message,
        }
      })
    }).then((cleanup) => {
      if (disposed) cleanup()
      else unlisten = cleanup
    }).catch((err) => {
      if (!disposed) setError(translationErrorMessage(err))
    })
    return () => {
      disposed = true
      if (unlisten) unlisten()
    }
  }, [enabled])

  const onDeleteTranslatedDocument = useCallback(async (id: string) => {
    setDeleteState(null)
    const deletedDocumentUrl = translatedDocuments.find((doc) => doc.id === id)?.documentUrl
    try {
      const result = await deleteTranslatedDocument(id)
      setDeleteState(result)
      await refreshGeneratedDocuments(deletedDocumentUrl)
    } catch (err) {
      setDeleteState({
        id,
        deleted: false,
        bytesFreed: 0,
        message: translationErrorMessage(err),
      })
    }
  }, [refreshGeneratedDocuments, translatedDocuments])

  const onInstallTranslationModel = useCallback(async (modelId: string) => {
    setModelInstallState({
      installingModelId: modelId,
      progress: null,
      result: null,
      message: 'Preparing translation model download',
    })
    try {
      const result = await installTranslationModel(modelId)
      setModelInstallState({
        installingModelId: '',
        progress: {
          modelId,
          status: 'installed',
          message: 'Translation model installed',
          downloadedBytes: result.bytes,
          totalBytes: result.bytes,
          percent: 100,
        },
        result,
        message: 'Translation model installed',
      })
      await refresh()
    } catch (err) {
      setModelInstallState({
        installingModelId: '',
        progress: null,
        result: null,
        message: translationErrorMessage(err),
      })
    }
  }, [refresh])

  const onStartTranslationPreflight = useCallback(async (request: TranslationStartRequest) => {
    const jobId = createTranslationJobId()
    setStartState({
      cancelling: false,
      checking: true,
      jobId,
      progress: null,
      result: null,
      message: 'Preparing translation job',
    })
    try {
      const result = await startTranslationJob({ ...request, jobId })
      setStartState((current) => ({
        cancelling: false,
        checking: false,
        jobId: result.jobId,
        progress: current.jobId === result.jobId ? current.progress : null,
        result,
        message: result.message,
      }))
      await refreshGeneratedDocuments()
    } catch (err) {
      setStartState((current) => ({
        cancelling: false,
        checking: false,
        jobId,
        progress: current.jobId === jobId ? current.progress : null,
        result: null,
        message: translationErrorMessage(err),
      }))
    }
  }, [refreshGeneratedDocuments])

  const onCancelTranslation = useCallback(async () => {
    const jobId = startState.jobId
    if (!jobId || !startState.checking) return
    setStartState((current) => ({
      ...current,
      cancelling: true,
      message: 'Cancelling translation job',
    }))
    try {
      await cancelTranslationJob(jobId)
    } catch (err) {
      setStartState((current) => ({
        ...current,
        cancelling: false,
        message: translationErrorMessage(err),
      }))
    }
  }, [startState.checking, startState.jobId])

  return {
    capabilities,
    deleteState,
    error,
    loading,
    modelInstallState,
    modelStatuses,
    startState,
    translatedDocuments,
    onDeleteTranslatedDocument,
    onInstallTranslationModel,
    onCancelTranslation,
    onStartTranslationPreflight,
    refresh,
  }
}

function createTranslationJobId(): string {
  const cryptoValue = globalThis.crypto?.randomUUID?.()
  if (cryptoValue) return cryptoValue
  return 'translation-' + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2)
}
