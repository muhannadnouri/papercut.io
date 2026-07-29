import { useCallback, useEffect, useRef, useState } from 'react'
import i18n from '../../i18n'
import {
  getNativeTtsCapabilities,
  getNativeTtsModelStatus,
  installNativeTtsModel,
  listenNativeTtsModelInstallProgress,
  probeNativeSilmaSidecar,
  type NativeTtsCapabilities,
  type NativeTtsModelInstallProgress,
  type NativeTtsModelStatus,
} from '../api/nativeTts'
import { logTtsDiagnostic } from '../diagnostics/TtsDiagnostics'
import { FALLBACK_TTS_MODELS } from '../models'
import { nativeTtsErrorDetail, nativeTtsErrorMessage } from '../utils/errors'

interface UseTtsModelRuntimeOptions {
  initialModelId: string
  preload: () => void
}

function summarizeTtsModelStatus(status: NativeTtsModelStatus | null): Record<string, unknown> {
  if (!status) return {}
  return {
    modelId: status.modelId,
    installed: status.installed,
    installing: status.installing,
    installSupported: status.installSupported,
    runtimeInstalled: status.runtimeInstalled,
    archiveBytes: status.archiveBytes,
    installedBytes: status.installedBytes,
    modelDir: status.modelDir ?? '',
    runtimeDir: status.runtimeDir ?? '',
    message: status.message,
    runtimeMessage: status.runtimeMessage,
  }
}

/** Own native model discovery, installation, progress, and runtime tuning. */
export function useTtsModelRuntime({ initialModelId, preload }: UseTtsModelRuntimeOptions) {
  const [modelId, setModelIdState] = useState(initialModelId)
  const [capabilities, setCapabilities] = useState<NativeTtsCapabilities | null>(null)
  const [modelStatus, setModelStatus] = useState<NativeTtsModelStatus | null>(null)
  const [modelProgress, setModelProgress] = useState<NativeTtsModelInstallProgress | null>(null)
  const [threadCount, setThreadCount] = useState(1)
  const [silmaProbeRunning, setSilmaProbeRunning] = useState(false)
  const modelIdRef = useRef(modelId)

  // Clear model-specific state synchronously with selection so the UI cannot
  // briefly present the previous model as installed while the new status loads.
  const setModelId = useCallback((nextModelId: string) => {
    if (modelIdRef.current === nextModelId) return
    modelIdRef.current = nextModelId
    setModelStatus(null)
    setModelProgress(null)
    setModelIdState(nextModelId)
  }, [])

  // Initialize from the platform default at startup; later refreshes preserve
  // the user's current choice while clamping it to the detected maximum.
  const syncRuntimeSettings = useCallback(async (initializeThreadCount = false) => {
    const nextCapabilities = await getNativeTtsCapabilities()
    const maxThreadCount = Math.max(1, nextCapabilities.maxThreadCount)
    const defaultThreadCount = Math.min(
      maxThreadCount,
      Math.max(1, nextCapabilities.defaultThreadCount),
    )
    setCapabilities({ ...nextCapabilities, defaultThreadCount, maxThreadCount })
    setThreadCount((current) => initializeThreadCount
      ? defaultThreadCount
      : Math.min(maxThreadCount, Math.max(1, current)))
    return nextCapabilities
  }, [])

  const refreshModelStatus = useCallback(async () => {
    const status = await getNativeTtsModelStatus(modelId)
    if (modelIdRef.current === status.modelId) setModelStatus(status)
    return status
  }, [modelId])

  useEffect(() => {
    void syncRuntimeSettings(true)
  }, [syncRuntimeSettings])

  useEffect(() => {
    void refreshModelStatus()
    let cancelled = false
    let unlisten: (() => void) | null = null
    listenNativeTtsModelInstallProgress((progress) => {
      if (!cancelled && progress.modelId === modelId) setModelProgress(progress)
    }).then((value) => {
      if (cancelled) value()
      else unlisten = value
    })
    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [modelId, refreshModelStatus])

  const installModel = useCallback(async () => {
    const installingSilmaRuntime = modelStatus?.runtimeInstalled === false
    logTtsDiagnostic('[tts-native] model install started', {
      modelId,
      installingSilmaRuntime,
      ...summarizeTtsModelStatus(modelStatus),
    })
    setModelProgress({
      modelId,
      status: 'starting',
      message: installingSilmaRuntime
        ? i18n.t('tts.setup.installingSilma')
        : i18n.t('tts.setup.preparingDownload'),
      downloadedBytes: 0,
      totalBytes: modelStatus?.archiveBytes ?? 0,
      percent: 0,
    })
    try {
      const result = await installNativeTtsModel(modelId)
      const status = await refreshModelStatus()
      await syncRuntimeSettings()
      if (modelIdRef.current !== modelId) return
      logTtsDiagnostic('[tts-native] model install completed', {
        resultModelDir: result.modelDir,
        resultBytes: result.bytes,
        ...summarizeTtsModelStatus(status),
      }, status.installed && status.runtimeInstalled ? 'info' : 'warn')
      if (!status.installed || !status.runtimeInstalled) {
        setModelProgress(null)
        return
      }
      setModelProgress((previous) => ({
        modelId,
        status: 'installed',
        message: i18n.t('tts.setup.installed'),
        downloadedBytes: previous?.totalBytes ?? modelStatus?.archiveBytes ?? 0,
        totalBytes: previous?.totalBytes ?? modelStatus?.archiveBytes ?? 0,
        percent: 100,
      }))
      preload()
    } catch (error) {
      if (modelIdRef.current !== modelId) return
      logTtsDiagnostic('[tts-native] model install failed', {
        modelId,
        error: nativeTtsErrorDetail(error),
        ...summarizeTtsModelStatus(modelStatus),
      }, 'error')
      setModelProgress({
        modelId,
        status: 'error',
        message: nativeTtsErrorMessage(error),
        downloadedBytes: 0,
        totalBytes: modelStatus?.archiveBytes ?? 0,
        percent: 0,
      })
      void refreshModelStatus()
    }
  }, [modelId, modelStatus, preload, refreshModelStatus, syncRuntimeSettings])

  const probeSilmaSidecar = useCallback(async () => {
    if (silmaProbeRunning) return
    setSilmaProbeRunning(true)
    try {
      const result = await probeNativeSilmaSidecar()
      logTtsDiagnostic('[tts-native] SILMA sidecar probe passed', { ...result })
    } catch (error) {
      logTtsDiagnostic('[tts-native] SILMA sidecar probe failed', {
        error: nativeTtsErrorDetail(error),
      }, 'error')
    } finally {
      setSilmaProbeRunning(false)
    }
  }, [silmaProbeRunning])

  useEffect(() => {
    if (window.requestIdleCallback) {
      const handle = window.requestIdleCallback(() => preload(), { timeout: 4000 })
      return () => window.cancelIdleCallback(handle)
    }
    const timeout = window.setTimeout(() => preload(), 1500)
    return () => window.clearTimeout(timeout)
  }, [preload])

  const maxThreadCount = capabilities?.maxThreadCount ?? 1
  const changeThreadCount = useCallback((nextThreadCount: number) => {
    setThreadCount(Math.min(maxThreadCount, Math.max(1, nextThreadCount)))
  }, [maxThreadCount])

  return {
    defaultThreadCount: capabilities?.defaultThreadCount ?? 1,
    installModel,
    maxThreadCount,
    modelId,
    modelProgress,
    models: capabilities?.models.length ? capabilities.models : FALLBACK_TTS_MODELS,
    modelStatus,
    onThreadCountChange: changeThreadCount,
    probeSilmaSidecar,
    setModelId,
    silmaProbeRunning,
    threadCount,
  }
}
