import { useCallback, useEffect, useRef, useState } from 'react'
import i18n from '../../i18n'
import {
  getNativeSavedAudiobookChunk,
  getNativeTtsCapabilities,
  prepareNativeAudiobookPlayback,
  type NativeAudiobookPlayback,
  resetNativeTtsCapabilities,
} from '../api/nativeTts'
import { logTtsDiagnostic, summarizeTtsCapabilities } from '../diagnostics/TtsDiagnostics'
import {
  disposeNativeAudio,
  getNativeAudioState,
  initializeNativeAudio,
  pauseNativeAudio,
  playNativeAudio,
  seekNativeAudio,
  setNativeAudioRate,
  setNativeAudioSource,
  stopNativeAudio,
  type NativeAudioState,
} from '../playback/nativeMobileAudio'
import type { TtsOptions, TtsChunk } from '../types'

type TtsStatus = 'idle' | 'loading' | 'playing' | 'paused' | 'error'

interface QueuedAudio {
  index: number
  chunkId: string
  text: string
  url: string
}

interface LoadedAudio {
  index: number
  chunkId: string
  text: string
  wav: ArrayBuffer
}

interface PreloadTarget {
  anchorIndex: number
  jobId: number
  navigationIntent: number
}

export interface TtsChunkSummary {
  index: number
  chunkId: string
  textPreview: string
}

export interface TtsPlayerState {
  status: TtsStatus
  message: string
  progress?: number
  chunksGenerated: number
  chunksPlayed: number
  chunksTotal: number
  currentText: string
  currentChunkIndex: number | null
  pendingChunkIndex: number | null
  currentChunkId: string | null
  currentChunkProgress: number
  currentChunkTime: number
  currentChunkDuration: number
  chunkSummaries: TtsChunkSummary[]
  chunks: TtsChunk[]
}

const MOBILE_PROGRESS_UPDATE_MS = 250
const DEFAULT_PLAYBACK_RATE = 1

const EMPTY_PLAYBACK_STATE = {
  currentText: '',
  currentChunkIndex: null,
  pendingChunkIndex: null,
  currentChunkId: null,
  currentChunkProgress: 0,
  currentChunkTime: 0,
  currentChunkDuration: 0,
} satisfies Pick<
  TtsPlayerState,
  | 'currentText'
  | 'currentChunkIndex'
  | 'pendingChunkIndex'
  | 'currentChunkId'
  | 'currentChunkProgress'
  | 'currentChunkTime'
  | 'currentChunkDuration'
>

export function useTtsPlayer(playbackRate = DEFAULT_PLAYBACK_RATE) {
  const [state, setState] = useState<TtsPlayerState>({
    status: 'idle',
    message: '',
    chunksGenerated: 0,
    chunksPlayed: 0,
    chunksTotal: 0,
    ...EMPTY_PLAYBACK_STATE,
    chunkSummaries: [],
    chunks: [],
  })

  const audioRef = useRef<HTMLAudioElement | null>(null)
  const audioByIndexRef = useRef(new Map<number, QueuedAudio>())
  const loadedIndexesRef = useRef(new Set<number>())
  const loadingByIndexRef = useRef(new Map<number, Promise<LoadedAudio | null>>())
  const chunksRef = useRef<TtsChunk[]>([])
  const optionsRef = useRef<TtsOptions | null>(null)
  const jobIdRef = useRef(0)
  const nextPlayIndexRef = useRef(0)
  const currentPlayingIndexRef = useRef<number | null>(null)
  const totalChunksRef = useRef(0)
  const pausedRef = useRef(false)
  const playingRef = useRef(false)
  const playbackAttemptRef = useRef(0)
  const navigationIntentRef = useRef(0)
  const navigationInFlightRef = useRef(false)
  const navigationFrameRef = useRef<number | null>(null)
  const queuedTargetIndexRef = useRef<number | null>(null)
  const pendingTargetIndexRef = useRef<number | null>(null)
  const preloadTargetRef = useRef<PreloadTarget | null>(null)
  const preloadInFlightRef = useRef(false)
  const mobileModeRef = useRef(false)
  const nativeMobilePlatformRef = useRef(false)
  const mobilePlaybackRef = useRef<NativeAudiobookPlayback | null>(null)
  const mobileQueuedIndexRef = useRef<number | null>(null)
  const mobilePendingIndexRef = useRef<number | null>(null)
  const mobileNavigationInFlightRef = useRef(false)
  const mobileNavigationFrameRef = useRef<number | null>(null)
  const mobilePollTimerRef = useRef<number | null>(null)
  const mobilePollGenerationRef = useRef(0)
  const mobileForegroundReadyRef = useRef(false)
  const mobileForegroundSyncRef = useRef<Promise<void> | null>(null)
  const nativeAudioInitializedRef = useRef(false)
  const playbackRateRef = useRef(playbackRate)

  // Increment generation whenever polling stops. Late async responses then fail
  // their generation fence and cannot overwrite newer playback/navigation state.
  const stopMobilePolling = useCallback(() => {
    if (mobilePollTimerRef.current !== null) {
      window.clearTimeout(mobilePollTimerRef.current)
      mobilePollTimerRef.current = null
    }
    mobilePollGenerationRef.current += 1
  }, [])

  const resetMobileForegroundSync = useCallback(() => {
    stopMobilePolling()
    mobileForegroundReadyRef.current = false
    mobileForegroundSyncRef.current = null
  }, [stopMobilePolling])

  const revokeAudioUrls = useCallback(() => {
    for (const item of audioByIndexRef.current.values()) {
      URL.revokeObjectURL(item.url)
    }
    audioByIndexRef.current.clear()
    loadedIndexesRef.current.clear()
    loadingByIndexRef.current.clear()
  }, [])

  const finishPlayback = useCallback(() => {
    playingRef.current = false
    currentPlayingIndexRef.current = null
    navigationInFlightRef.current = false
    queuedTargetIndexRef.current = null
    pendingTargetIndexRef.current = null
    preloadTargetRef.current = null
    mobileQueuedIndexRef.current = null
    mobilePendingIndexRef.current = null
    setState((prev) => ({
      ...prev,
      status: 'idle',
      message: '',
      ...EMPTY_PLAYBACK_STATE,
    }))
  }, [])

  // Native global timeline is source of truth on mobile. Convert it back into
  // existing chunk-local UI/highlight fields through binary-searched boundaries.
  const updateNativePlaybackState = useCallback((nativeState: NativeAudioState) => {
    if (!mobileModeRef.current) return
    if (nativeState.status === 'ended') {
      resetMobileForegroundSync()
      mobilePlaybackRef.current = null
      mobileModeRef.current = false
      void stopNativeAudio().catch((err: unknown) => {
        logTtsDiagnostic('[tts-playback] native reset failed', { error: errorMessage(err) }, 'warn')
      })
      finishPlayback()
      return
    }
    if (nativeState.status === 'error') {
      pausedRef.current = true
      playingRef.current = false
      setState((prev) => ({
        ...prev,
        status: 'error',
        message: nativeState.error || i18n.t('tts.status.nativePlaybackFailed'),
      }))
      return
    }

    const playback = mobilePlaybackRef.current
    if (!playback) return
    const timing = findPlaybackChunk(playback, nativeState.currentTime)
    if (!timing) return
    const chunk = chunksRef.current[timing.index]
    if (!chunk) return

    const localTime = Math.min(
      Math.max(nativeState.currentTime - timing.startSec, 0),
      timing.durationSec,
    )
    currentPlayingIndexRef.current = timing.index
    nextPlayIndexRef.current = Math.min(timing.index + 1, totalChunksRef.current)
    playingRef.current = nativeState.isPlaying
    if (nativeState.status === 'idle' && !mobileNavigationInFlightRef.current) {
      pausedRef.current = true
    }
    if (nativeState.isPlaying) pausedRef.current = false
    setState((prev) => ({
      ...prev,
      status: nativeState.isPlaying
        ? 'playing'
        : (nativeState.status === 'idle' ? 'paused' : 'loading'),
      message: nativeState.buffering ? i18n.t('tts.status.buffering') : '',
      chunksGenerated: totalChunksRef.current,
      chunksPlayed: timing.index,
      currentText: chunk.text,
      currentChunkIndex: timing.index,
      pendingChunkIndex: null,
      currentChunkId: chunk.id,
      currentChunkProgress: timing.durationSec > 0 ? Math.min(localTime / timing.durationSec, 1) : 0,
      currentChunkTime: localTime,
      currentChunkDuration: timing.durationSec,
    }))
  }, [finishPlayback, resetMobileForegroundSync])

  // One non-overlapping foreground poll replaces plugin high-frequency events.
  // Polling pauses while hidden and while seek worker owns player state.
  const startMobilePolling = useCallback(() => {
    stopMobilePolling()
    const generation = mobilePollGenerationRef.current
    const schedule = () => {
      if (
        generation !== mobilePollGenerationRef.current ||
        !mobileModeRef.current ||
        document.visibilityState !== 'visible'
      ) return
      mobilePollTimerRef.current = window.setTimeout(() => {
        mobilePollTimerRef.current = null
        void poll()
      }, MOBILE_PROGRESS_UPDATE_MS)
    }
    async function poll() {
      try {
        if (!mobileNavigationInFlightRef.current) {
          const nativeState = await getNativeAudioState()
          if (
            generation !== mobilePollGenerationRef.current ||
            !mobileModeRef.current ||
            document.visibilityState !== 'visible' ||
            mobileNavigationInFlightRef.current
          ) return
          updateNativePlaybackState(nativeState)
        }
      } catch (err) {
        if (generation === mobilePollGenerationRef.current) {
          logTtsDiagnostic(
            '[tts-playback] native state poll failed',
            { error: errorMessage(err) },
            'warn',
          )
        }
      } finally {
        schedule()
      }
    }
    schedule()
  }, [stopMobilePolling, updateNativePlaybackState])

  // After lock/background, read native state once before any UI command. Concurrent
  // callers share one promise so several taps cannot launch duplicate bridge calls.
  const syncMobileForegroundState = useCallback((): Promise<void> => {
    if (!mobileModeRef.current || document.visibilityState !== 'visible') {
      return Promise.resolve()
    }
    if (mobileForegroundReadyRef.current) {
      startMobilePolling()
      return Promise.resolve()
    }
    const existing = mobileForegroundSyncRef.current
    if (existing) return existing

    stopMobilePolling()
    const generation = mobilePollGenerationRef.current
    const task: Promise<void> = (async () => {
      const nativeState = await getNativeAudioState()
      if (
        generation !== mobilePollGenerationRef.current ||
        document.visibilityState !== 'visible' ||
        !mobileModeRef.current
      ) return

      if (!mobileNavigationInFlightRef.current) mobilePendingIndexRef.current = null
      updateNativePlaybackState(nativeState)
      mobileForegroundReadyRef.current = true
      startMobilePolling()
    })()
    mobileForegroundSyncRef.current = task
    const clearTask = () => {
      if (mobileForegroundSyncRef.current === task) mobileForegroundSyncRef.current = null
    }
    task.then(clearTask, clearTask)
    return task
  }, [startMobilePolling, stopMobilePolling, updateNativePlaybackState])

  const playIndex = useCallback((index: number) => {
    const audio = audioRef.current
    if (!audio || pausedRef.current) return false

    if (index >= totalChunksRef.current) {
      nextPlayIndexRef.current = totalChunksRef.current
      finishPlayback()
      return true
    }

    const item = audioByIndexRef.current.get(index)
    if (!item) {
      nextPlayIndexRef.current = index
      playingRef.current = false
      setState((prev) => ({
        ...prev,
        status: 'loading',
        message: i18n.t('tts.status.loadingChunk', {
          current: index + 1,
          total: totalChunksRef.current,
        }),
      }))
      return false
    }

    const playJobId = jobIdRef.current
    const playAttemptId = playbackAttemptRef.current + 1
    playbackAttemptRef.current = playAttemptId

    nextPlayIndexRef.current = index + 1
    currentPlayingIndexRef.current = index
    playingRef.current = true
    audio.src = item.url
    audio.currentTime = 0
    audio.playbackRate = playbackRateRef.current

    setState((prev) => ({
      ...prev,
      status: 'playing',
      message: '',
      chunksPlayed: index,
      currentText: item.text,
      currentChunkIndex: item.index,
      pendingChunkIndex: null,
      currentChunkId: item.chunkId,
      currentChunkProgress: 0,
      currentChunkTime: 0,
      currentChunkDuration: Number.isFinite(audio.duration) ? audio.duration : 0,
    }))

    audio.play().catch((err: unknown) => {
      // Rapid skip/jump actions intentionally replace the audio source. Browsers
      // reject the superseded play() promise; ignore it unless it is still the
      // newest playback attempt for the current job and chunk.
      if (jobIdRef.current !== playJobId) return
      if (playbackAttemptRef.current !== playAttemptId) return
      if (currentPlayingIndexRef.current !== index) return
      if (pausedRef.current) return

      if (isTransientPlaybackInterruption(err)) {
        playingRef.current = false
        setState((prev) => ({
          ...prev,
          status: 'loading',
          message: i18n.t('tts.status.switchingChunk'),
        }))
        return
      }

      pausedRef.current = true
      playingRef.current = false
      setState((prev) => ({
        ...prev,
        status: 'error',
        message: err instanceof Error ? err.message : String(err),
      }))
    })

    return true
  }, [finishPlayback])

  const pruneAudioWindow = useCallback((anchorIndex: number) => {
    const min = Math.max(anchorIndex - 2, 0)
    const max = Math.min(anchorIndex + 4, Math.max(totalChunksRef.current - 1, 0))

    for (const item of audioByIndexRef.current.values()) {
      if (item.index >= min && item.index <= max) continue
      if (item.index === currentPlayingIndexRef.current) continue
      URL.revokeObjectURL(item.url)
      audioByIndexRef.current.delete(item.index)
    }
  }, [])

  const enqueueAudio = useCallback((item: QueuedAudio) => {
    const previous = audioByIndexRef.current.get(item.index)
    if (previous && previous.url !== item.url) URL.revokeObjectURL(previous.url)

    audioByIndexRef.current.set(item.index, item)
    loadedIndexesRef.current.add(item.index)
    setState((prev) => ({
      ...prev,
      chunksGenerated: loadedIndexesRef.current.size,
    }))
  }, [])

  const loadChunk = useCallback(async (
    index: number,
    jobId: number,
    shouldAccept: () => boolean,
  ): Promise<boolean> => {
    const chunks = chunksRef.current
    const options = optionsRef.current
    if (audioByIndexRef.current.has(index)) return true
    if (!options || index < 0 || index >= chunks.length) return false

    let promise = loadingByIndexRef.current.get(index)
    if (!promise) {
      const chunk = chunks[index]
      if (!options.documentUrl) throw new Error(i18n.t('tts.status.playbackNeedsDocument'))

      promise = getNativeSavedAudiobookChunk(options.documentUrl, chunk, index, options).then((nativeSaved) => {
        if (!nativeSaved) return null
        return {
          index,
          chunkId: chunk.id,
          text: chunk.text,
          wav: nativeSaved.wav,
        }
      })
      loadingByIndexRef.current.set(index, promise)
      const clearLoading = () => {
        if (loadingByIndexRef.current.get(index) === promise) {
          loadingByIndexRef.current.delete(index)
        }
      }
      void promise.then(clearLoading, clearLoading)
    }

    const loaded = await promise
    if (jobIdRef.current !== jobId || !shouldAccept()) return false
    if (!loaded) {
      throw new Error(i18n.t('tts.status.missingSavedChunk'))
    }
    if (audioByIndexRef.current.has(index)) return true

    enqueueAudio({
      index: loaded.index,
      chunkId: loaded.chunkId,
      text: loaded.text,
      url: URL.createObjectURL(new Blob([loaded.wav], { type: 'audio/wav' })),
    })
    return true
  }, [enqueueAudio])

  // Single speculative desktop worker. New anchor replaces queued target; stale work
  // is rejected by job/navigation ids and never commits obsolete audio.
  const runPreloadWorker = useCallback(async () => {
    if (preloadInFlightRef.current) return
    preloadInFlightRef.current = true

    try {
      while (preloadTargetRef.current) {
        const target = preloadTargetRef.current
        preloadTargetRef.current = null

        for (let offset = 1; offset <= 2; offset++) {
          const index = target.anchorIndex + offset
          const isCurrentTarget = () => (
            jobIdRef.current === target.jobId &&
            navigationIntentRef.current === target.navigationIntent
          )
          if (index >= totalChunksRef.current || !isCurrentTarget()) break
          try {
            await loadChunk(index, target.jobId, isCurrentTarget)
          } catch {
            break
          }
        }
      }
    } finally {
      preloadInFlightRef.current = false
    }
  }, [loadChunk])

  const preloadAround = useCallback((
    anchorIndex: number,
    jobId: number,
    navigationIntent: number,
  ) => {
    preloadTargetRef.current = { anchorIndex, jobId, navigationIntent }
    void runPreloadWorker()
  }, [runPreloadWorker])

  const clampChunkIndex = useCallback((index: number) => Math.min(
    Math.max(index, 0),
    Math.max(totalChunksRef.current - 1, 0),
  ), [])

  // Serialize mobile seeks and apply latest-target-wins. Intermediate queued targets
  // are replaced before commit, keeping rapid skips bounded to one bridge command.
  const runMobileNavigationWorker = useCallback(async () => {
    if (mobileNavigationInFlightRef.current) return
    mobileNavigationInFlightRef.current = true
    try {
      while (mobileQueuedIndexRef.current !== null && mobileModeRef.current) {
        const targetIndex = mobileQueuedIndexRef.current
        mobileQueuedIndexRef.current = null
        const timing = mobilePlaybackRef.current?.chunks[targetIndex]
        if (!timing) {
          mobilePendingIndexRef.current = null
          continue
        }

        mobilePendingIndexRef.current = targetIndex
        setState((prev) => ({
          ...prev,
          status: 'loading',
          message: i18n.t('tts.status.seekingChunk', {
            current: targetIndex + 1,
            total: totalChunksRef.current,
          }),
          pendingChunkIndex: targetIndex,
        }))
        const nativeState = await seekNativeAudio(timing.startSec)
        if (!mobileModeRef.current) return
        if (mobileQueuedIndexRef.current !== null) continue
        updateNativePlaybackState(nativeState)
        mobilePendingIndexRef.current = null
      }
    } finally {
      mobilePendingIndexRef.current = null
      mobileNavigationInFlightRef.current = false
      if (mobileModeRef.current) {
        setState((prev) => prev.pendingChunkIndex === null
          ? prev
          : { ...prev, pendingChunkIndex: null })
      }
    }
  }, [updateNativePlaybackState])

  // Coalesce taps within one animation frame before starting serialized seek worker.
  const seekMobileToChunk = useCallback((index: number) => {
    if (!mobileModeRef.current || totalChunksRef.current === 0) return
    mobileQueuedIndexRef.current = clampChunkIndex(index)
    if (mobileNavigationInFlightRef.current || mobileNavigationFrameRef.current !== null) return
    mobileNavigationFrameRef.current = window.requestAnimationFrame(() => {
      mobileNavigationFrameRef.current = null
      void runMobileNavigationWorker().catch((err: unknown) => {
        pausedRef.current = true
        playingRef.current = false
        mobileQueuedIndexRef.current = null
        mobilePendingIndexRef.current = null
        setState((prev) => ({
          ...prev,
          status: 'error',
          message: errorMessage(err),
          pendingChunkIndex: null,
        }))
      })
    })
  }, [clampChunkIndex, runMobileNavigationWorker])

  // Android WebView can deliver touch skips faster than audio.pause(), src
  // changes, and audio.play() promises settle. Keep one foreground loader and
  // commit a target only if it is still the newest navigation intent.
  const runNavigationWorker = useCallback(async () => {
    if (navigationInFlightRef.current || totalChunksRef.current === 0) return
    navigationInFlightRef.current = true

    try {
      while (queuedTargetIndexRef.current !== null && totalChunksRef.current > 0) {
        const targetIndex = queuedTargetIndexRef.current
        queuedTargetIndexRef.current = null

        const audio = audioRef.current
        if (!audio) return

        const jobId = jobIdRef.current
        const navigationIntent = navigationIntentRef.current
        pendingTargetIndexRef.current = targetIndex

        pausedRef.current = false
        playingRef.current = false
        playbackAttemptRef.current += 1
        audio.pause()
        nextPlayIndexRef.current = targetIndex

        setState((prev) => ({
          ...prev,
          status: 'loading',
          message: i18n.t('tts.status.loadingChunk', {
            current: targetIndex + 1,
            total: totalChunksRef.current,
          }),
          pendingChunkIndex: targetIndex,
          currentChunkProgress: 0,
          currentChunkTime: 0,
          currentChunkDuration: 0,
        }))

        const isCurrentTarget = () => (
          jobIdRef.current === jobId &&
          navigationIntentRef.current === navigationIntent &&
          !pausedRef.current
        )
        const loaded = await loadChunk(targetIndex, jobId, isCurrentTarget)
        if (!loaded || !isCurrentTarget()) continue

        pruneAudioWindow(targetIndex)
        const didStart = playIndex(targetIndex)
        if (navigationIntentRef.current === navigationIntent) {
          pendingTargetIndexRef.current = didStart ? null : targetIndex
        }
        if (didStart) preloadAround(targetIndex, jobId, navigationIntent)
      }
    } finally {
      navigationInFlightRef.current = false
    }
  }, [loadChunk, playIndex, preloadAround, pruneAudioWindow])

  const startPlaybackAt = useCallback((index: number) => {
    if (totalChunksRef.current === 0) return
    queuedTargetIndexRef.current = clampChunkIndex(index)
    navigationIntentRef.current += 1
    preloadTargetRef.current = null

    if (navigationInFlightRef.current || navigationFrameRef.current !== null) return
    navigationFrameRef.current = window.requestAnimationFrame(() => {
      navigationFrameRef.current = null
      void runNavigationWorker().catch((err: unknown) => {
        pausedRef.current = false
        playingRef.current = false
        setState((prev) => ({
          ...prev,
          status: 'error',
          message: err instanceof Error ? err.message : String(err),
        }))
      })
    })
  }, [clampChunkIndex, runNavigationWorker])

  useEffect(() => {
    playbackRateRef.current = normalizePlaybackRate(playbackRate)
    if (audioRef.current) {
      audioRef.current.playbackRate = playbackRateRef.current
    }
    if (mobileModeRef.current && nativeAudioInitializedRef.current) {
      void setNativeAudioRate(playbackRateRef.current).catch((err: unknown) => {
        logTtsDiagnostic('[tts-playback] native rate update failed', { error: errorMessage(err) }, 'warn')
      })
    }
  }, [playbackRate])

  useEffect(() => {
    const audio = new Audio()
    audio.preload = 'auto'
    audio.playbackRate = playbackRateRef.current
    audioRef.current = audio

    const updateProgress = () => {
      const duration = Number.isFinite(audio.duration) ? audio.duration : 0
      const currentTime = Number.isFinite(audio.currentTime) ? audio.currentTime : 0
      setState((prev) => ({
        ...prev,
        currentChunkTime: currentTime,
        currentChunkDuration: duration,
        currentChunkProgress: duration > 0 ? Math.min(currentTime / duration, 1) : 0,
      }))
    }

    const handleEnded = () => {
      const finishedIndex = currentPlayingIndexRef.current
      setState((prev) => ({
        ...prev,
        chunksPlayed: finishedIndex === null
          ? prev.chunksPlayed + 1
          : Math.max(prev.chunksPlayed, finishedIndex + 1),
        currentChunkProgress: 1,
      }))
      const nextIndex = (finishedIndex ?? nextPlayIndexRef.current - 1) + 1
      if (nextIndex >= totalChunksRef.current) {
        finishPlayback()
        return
      }
      void startPlaybackAt(nextIndex)
    }

    audio.addEventListener('timeupdate', updateProgress)
    audio.addEventListener('durationchange', updateProgress)
    audio.addEventListener('loadedmetadata', updateProgress)
    audio.addEventListener('ended', handleEnded)
    return () => {
      audio.pause()
      audio.removeEventListener('timeupdate', updateProgress)
      audio.removeEventListener('durationchange', updateProgress)
      audio.removeEventListener('loadedmetadata', updateProgress)
      audio.removeEventListener('ended', handleEnded)
      if (navigationFrameRef.current !== null) {
        window.cancelAnimationFrame(navigationFrameRef.current)
        navigationFrameRef.current = null
      }
      audioRef.current = null
      revokeAudioUrls()
    }
  }, [finishPlayback, revokeAudioUrls, startPlaybackAt])

  useEffect(() => {
    const handleVisibilityChange = () => {
      if (document.visibilityState !== 'visible') {
        resetMobileForegroundSync()
        return
      }
      void syncMobileForegroundState().catch((err: unknown) => {
        logTtsDiagnostic('[tts-playback] foreground sync failed', { error: errorMessage(err) }, 'warn')
      })
    }
    document.addEventListener('visibilitychange', handleVisibilityChange)
    return () => {
      document.removeEventListener('visibilitychange', handleVisibilityChange)
      resetMobileForegroundSync()
      if (mobileNavigationFrameRef.current !== null) {
        window.cancelAnimationFrame(mobileNavigationFrameRef.current)
        mobileNavigationFrameRef.current = null
      }
      if (nativeAudioInitializedRef.current) {
        nativeAudioInitializedRef.current = false
        void disposeNativeAudio().catch((err: unknown) => {
          logTtsDiagnostic('[tts-playback] native dispose failed', { error: errorMessage(err) }, 'warn')
        })
      }
    }
  }, [resetMobileForegroundSync, syncMobileForegroundState])

  const preload = useCallback(() => {
    setState((prev) => (prev.status === 'idle'
      ? {
        ...prev,
        status: 'loading',
        message: i18n.t('tts.status.checkingNative'),
        progress: undefined,
      }
      : prev))

    void getNativeTtsCapabilities().then((capabilities) => {
      logTtsDiagnostic('[tts-native] capabilities', summarizeTtsCapabilities(capabilities))
      nativeMobilePlatformRef.current = isNativeMobilePlatform(capabilities.platform)
      setState((prev) => ({
        ...prev,
        status: prev.status === 'loading' ? (capabilities.available ? 'idle' : 'error') : prev.status,
        message: prev.status === 'loading' ? (capabilities.available ? '' : capabilities.reason) : prev.message,
      }))
    })
  }, [])

  // Prepare/reuse native single track, load it once, then let platform session own
  // playback while React polls and maps global time back to chunks.
  const startNativePlayback = useCallback(async (
    chunks: TtsChunk[],
    options: TtsOptions,
    jobId: number,
  ) => {
    if (!options.documentUrl) {
      throw new Error(i18n.t('tts.status.playbackNeedsDocument'))
    }
    mobileModeRef.current = true
    setState((prev) => ({
      ...prev,
      status: 'loading',
      message: i18n.t('tts.status.preparingPlayback'),
    }))
    const prepareStarted = performance.now()
    const playback = await prepareNativeAudiobookPlayback(options.documentUrl, chunks, options)
    logTtsDiagnostic('[tts-playback] native preparation completed', {
      chunks: playback.chunks.length,
      elapsedMs: Math.round(performance.now() - prepareStarted),
    })
    if (jobIdRef.current !== jobId || !mobileModeRef.current) return

    mobilePlaybackRef.current = playback
    await initializeNativeAudio()
    nativeAudioInitializedRef.current = true
    if (jobIdRef.current !== jobId || !mobileModeRef.current) return

    const sourceStarted = performance.now()
    const sourceState = await setNativeAudioSource({
      src: playback.audioUrl,
      title: options.title || 'Papercut Audiobook',
      artist: 'Papercut',
    })
    logTtsDiagnostic('[tts-playback] native source loaded', {
      elapsedMs: Math.round(performance.now() - sourceStarted),
    })
    if (jobIdRef.current !== jobId || !mobileModeRef.current) return
    pausedRef.current = false
    updateNativePlaybackState(sourceState)
    await setNativeAudioRate(playbackRateRef.current)
    if (jobIdRef.current !== jobId || !mobileModeRef.current) return
    updateNativePlaybackState(await playNativeAudio())
    await syncMobileForegroundState()
  }, [syncMobileForegroundState, updateNativePlaybackState])

  const speak = useCallback((chunks: TtsChunk[], options: TtsOptions) => {
    const speakableChunks = chunks.filter((chunk) => chunk.text.trim())
    if (speakableChunks.length === 0) return

    const nextJobId = jobIdRef.current + 1
    jobIdRef.current = nextJobId
    nextPlayIndexRef.current = 0
    currentPlayingIndexRef.current = null
    totalChunksRef.current = speakableChunks.length
    pausedRef.current = false
    playingRef.current = false
    playbackAttemptRef.current += 1
    navigationIntentRef.current += 1
    if (navigationFrameRef.current !== null) {
      window.cancelAnimationFrame(navigationFrameRef.current)
      navigationFrameRef.current = null
    }
    if (mobileNavigationFrameRef.current !== null) {
      window.cancelAnimationFrame(mobileNavigationFrameRef.current)
      mobileNavigationFrameRef.current = null
    }
    queuedTargetIndexRef.current = null
    pendingTargetIndexRef.current = null
    preloadTargetRef.current = null
    chunksRef.current = speakableChunks
    optionsRef.current = options
    audioRef.current?.pause()
    resetMobileForegroundSync()
    mobilePlaybackRef.current = null
    mobileQueuedIndexRef.current = null
    mobilePendingIndexRef.current = null
    mobileNavigationInFlightRef.current = false
    const nativeCleanup = mobileModeRef.current && nativeAudioInitializedRef.current
      ? stopNativeAudio()
      : Promise.resolve()
    mobileModeRef.current = false
    revokeAudioUrls()

    setState((prev) => ({
      ...prev,
      status: 'loading',
      message: i18n.t('tts.status.checkingSaved'),
      progress: undefined,
      chunksGenerated: 0,
      chunksPlayed: 0,
      chunksTotal: speakableChunks.length,
      chunkSummaries: speakableChunks.map((chunk, index) => ({
        index,
        chunkId: chunk.id,
        textPreview: textPreview(chunk.text),
      })),
      chunks: speakableChunks,
      ...EMPTY_PLAYBACK_STATE,
    }))

    void nativeCleanup
      .then(() => getNativeTtsCapabilities())
      .then((capabilities) => {
        if (jobIdRef.current !== nextJobId) return
        nativeMobilePlatformRef.current = isNativeMobilePlatform(capabilities.platform)
        if (nativeMobilePlatformRef.current) {
          return startNativePlayback(speakableChunks, options, nextJobId)
        }
        startPlaybackAt(0)
      })
      .catch((err: unknown) => {
        if (jobIdRef.current !== nextJobId) return
        setState((prev) => ({
          ...prev,
          status: 'error',
          message: err instanceof Error ? err.message : String(err),
        }))
      })
  }, [resetMobileForegroundSync, revokeAudioUrls, startNativePlayback, startPlaybackAt])

  const pause = useCallback(() => {
    pausedRef.current = true
    playingRef.current = false
    playbackAttemptRef.current += 1
    if (mobileModeRef.current) {
      void syncMobileForegroundState()
        .then(() => pauseNativeAudio())
        .then(updateNativePlaybackState)
        .catch((err: unknown) => {
          setState((prev) => ({
            ...prev,
            status: 'error',
            message: err instanceof Error ? err.message : String(err),
          }))
        })
      setState((prev) => ({ ...prev, status: 'paused' }))
      return
    }
    audioRef.current?.pause()
    setState((prev) => ({ ...prev, status: 'paused' }))
  }, [syncMobileForegroundState, updateNativePlaybackState])

  const resume = useCallback(() => {
    pausedRef.current = false
    if (mobileModeRef.current) {
      void syncMobileForegroundState()
        .then(() => playNativeAudio())
        .then(updateNativePlaybackState)
        .catch((err: unknown) => {
          setState((prev) => ({
            ...prev,
            status: 'error',
            message: err instanceof Error ? err.message : String(err),
          }))
        })
      return
    }
    const audio = audioRef.current
    if (audio?.src && !audio.ended) {
      audio.play()
        .then(() => {
          playingRef.current = true
          setState((prev) => ({ ...prev, status: 'playing' }))
        })
        .catch((err: unknown) => {
          setState((prev) => ({
            ...prev,
            status: 'error',
            message: err instanceof Error ? err.message : String(err),
          }))
        })
      return
    }
    void startPlaybackAt(nextPlayIndexRef.current)
  }, [startPlaybackAt, syncMobileForegroundState, updateNativePlaybackState])

  // Mobile first synchronizes after backgrounding, then bases relative skip on
  // newest queued/pending/native index. Desktop uses its separate loader state.
  const skipByChunks = useCallback((delta: number) => {
    if (totalChunksRef.current === 0) return

    if (mobileModeRef.current) {
      void syncMobileForegroundState()
        .then(() => {
          if (!mobileModeRef.current || totalChunksRef.current === 0) return
          const activePendingIndex = mobileNavigationInFlightRef.current
            ? mobilePendingIndexRef.current
            : null
          const currentIndex = mobileQueuedIndexRef.current ??
            activePendingIndex ??
            currentPlayingIndexRef.current ??
            Math.max(nextPlayIndexRef.current - 1, 0)
          seekMobileToChunk(currentIndex + delta)
        })
        .catch((err: unknown) => {
          setState((prev) => ({
            ...prev,
            status: 'error',
            message: errorMessage(err),
          }))
        })
      return
    }

    const currentIndex = queuedTargetIndexRef.current ??
      pendingTargetIndexRef.current ??
      currentPlayingIndexRef.current ??
      Math.max(nextPlayIndexRef.current - 1, 0)
    pausedRef.current = false
    startPlaybackAt(currentIndex + delta)
  }, [seekMobileToChunk, startPlaybackAt, syncMobileForegroundState])

  const jumpToChunk = useCallback((index: number) => {
    if (totalChunksRef.current === 0) return
    if (mobileModeRef.current) {
      void syncMobileForegroundState()
        .then(() => {
          if (!mobileModeRef.current) return
          seekMobileToChunk(index)
        })
        .catch((err: unknown) => {
          setState((prev) => ({
            ...prev,
            status: 'error',
            message: errorMessage(err),
          }))
        })
      return
    }
    startPlaybackAt(index)
  }, [seekMobileToChunk, startPlaybackAt, syncMobileForegroundState])

  // Invalidate every async generation/queue before resetting UI. Native Stop is
  // non-destructive pause+seek(0); desktop also revokes bounded Blob URLs.
  const stop = useCallback(() => {
    jobIdRef.current += 1
    nextPlayIndexRef.current = 0
    currentPlayingIndexRef.current = null
    totalChunksRef.current = 0
    pausedRef.current = false
    playingRef.current = false
    playbackAttemptRef.current += 1
    navigationIntentRef.current += 1
    if (navigationFrameRef.current !== null) {
      window.cancelAnimationFrame(navigationFrameRef.current)
      navigationFrameRef.current = null
    }
    if (mobileNavigationFrameRef.current !== null) {
      window.cancelAnimationFrame(mobileNavigationFrameRef.current)
      mobileNavigationFrameRef.current = null
    }
    queuedTargetIndexRef.current = null
    pendingTargetIndexRef.current = null
    preloadTargetRef.current = null
    mobileQueuedIndexRef.current = null
    mobilePendingIndexRef.current = null
    mobileNavigationInFlightRef.current = false
    resetMobileForegroundSync()
    const shouldStopNativeAudio = mobileModeRef.current && nativeAudioInitializedRef.current
    mobileModeRef.current = false
    mobilePlaybackRef.current = null
    chunksRef.current = []
    optionsRef.current = null
    audioRef.current?.pause()
    if (shouldStopNativeAudio) {
      void stopNativeAudio().catch((err: unknown) => {
        logTtsDiagnostic('[tts-playback] native reset failed', { error: errorMessage(err) }, 'warn')
      })
    }
    revokeAudioUrls()
    setState((prev) => ({
      ...prev,
      status: 'idle',
      message: '',
      progress: undefined,
      chunksGenerated: 0,
      chunksPlayed: 0,
      chunksTotal: 0,
      chunkSummaries: [],
      chunks: [],
      ...EMPTY_PLAYBACK_STATE,
    }))
  }, [resetMobileForegroundSync, revokeAudioUrls])

  return {
    state,
    preload,
    speak,
    pause,
    resume,
    jumpToChunk,
    skipBackward: () => skipByChunks(-1),
    skipForward: () => skipByChunks(1),
    stop,
    resetNativeTtsCapabilities,
  }
}

function isNativeMobilePlatform(platform: string): boolean {
  return platform === 'android' || platform === 'ios'
}

function findPlaybackChunk(
  playback: NativeAudiobookPlayback,
  currentTime: number,
): NativeAudiobookPlayback['chunks'][number] | null {
  const chunks = playback.chunks
  if (chunks.length === 0) return null

  let low = 0
  let high = chunks.length - 1
  while (low <= high) {
    const middle = Math.floor((low + high) / 2)
    const chunk = chunks[middle]
    const nextStart = chunks[middle + 1]?.startSec ?? playback.audioDurationSec
    if (currentTime < chunk.startSec) {
      high = middle - 1
    } else if (currentTime >= nextStart && middle < chunks.length - 1) {
      low = middle + 1
    } else {
      return chunk
    }
  }

  return currentTime < chunks[0].startSec ? chunks[0] : chunks[chunks.length - 1]
}

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err)
}

function normalizePlaybackRate(rate: number): number {
  if (!Number.isFinite(rate) || rate <= 0) return DEFAULT_PLAYBACK_RATE
  return Number(Math.min(3, Math.max(0.5, rate)).toFixed(2))
}

function isTransientPlaybackInterruption(err: unknown): boolean {
  // Mobile browsers reject play() with AbortError/interruption messages when a
  // user quickly replaces the audio source. Those are navigation noise, not a
  // real TTS failure, so they should not collapse the floating controls.
  if (!(err instanceof Error)) return false
  const name = err.name.toLowerCase()
  const message = err.message.toLowerCase()
  return name === 'aborterror' ||
    message.includes('interrupted') ||
    message.includes('new load request') ||
    message.includes('pause')
}

function textPreview(text: string): string {
  const normalized = text.replace(/\s+/g, ' ').trim()
  if (normalized.length <= 96) return normalized
  return normalized.slice(0, 95).trimEnd() + '...'
}
