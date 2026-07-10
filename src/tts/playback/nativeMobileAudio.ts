import type {
  NativeAudioSetSourcePayload,
  NativeAudioState,
} from 'tauri-plugin-native-audio-api'

let initializePromise: Promise<NativeAudioState> | null = null
let lifecyclePromise: Promise<void> = Promise.resolve()

async function loadPlugin() {
  return import('tauri-plugin-native-audio-api')
}

// Serialize every plugin command. Platform players mutate one shared session, and
// overlapping play/seek/pause calls can otherwise resolve out of order.
function runLifecycleOperation<T>(operation: () => Promise<T>): Promise<T> {
  const result = lifecyclePromise.catch(() => {}).then(operation)
  lifecyclePromise = result.then(() => undefined, () => undefined)
  return result
}

// Share one in-flight/successful initialization across callers, but allow a later
// playback attempt to recover if native setup fails. The identity check prevents
// an older rejected attempt from clearing a newer initialization after disposal.
export function initializeNativeAudio(): Promise<NativeAudioState> {
  if (initializePromise) return initializePromise

  const attempt = runLifecycleOperation(async () => {
    const plugin = await loadPlugin()
    return plugin.initialize()
  })
  initializePromise = attempt
  void attempt.catch(() => {
    if (initializePromise === attempt) {
      initializePromise = null
    }
  })
  return attempt
}

export async function setNativeAudioSource(payload: NativeAudioSetSourcePayload): Promise<NativeAudioState> {
  await initializeNativeAudio()
  return runLifecycleOperation(async () => {
    const plugin = await loadPlugin()
    return plugin.setSource(payload)
  })
}

export function playNativeAudio(): Promise<NativeAudioState> {
  return runLifecycleOperation(async () => {
    const plugin = await loadPlugin()
    return plugin.play()
  })
}

export function pauseNativeAudio(): Promise<NativeAudioState> {
  return runLifecycleOperation(async () => {
    const plugin = await loadPlugin()
    return plugin.pause()
  })
}

export function seekNativeAudio(position: number): Promise<NativeAudioState> {
  return runLifecycleOperation(async () => {
    const plugin = await loadPlugin()
    return plugin.seekTo(position)
  })
}

export function setNativeAudioRate(rate: number): Promise<NativeAudioState> {
  return runLifecycleOperation(async () => {
    const plugin = await loadPlugin()
    return plugin.setRate(rate)
  })
}

export function getNativeAudioState(): Promise<NativeAudioState> {
  return runLifecycleOperation(async () => {
    const plugin = await loadPlugin()
    return plugin.getState()
  })
}

// Plugin has no non-destructive Stop API. Pause + seek(0) preserves reusable
// source/session while giving app controls expected stopped behavior.
export function stopNativeAudio(): Promise<void> {
  return runLifecycleOperation(async () => {
    const plugin = await loadPlugin()
    await plugin.pause()
    await plugin.seekTo(0)
  })
}

// Full disposal is teardown-only; normal Stop must preserve background session setup.
export function disposeNativeAudio(): Promise<void> {
  initializePromise = null
  return runLifecycleOperation(async () => {
    const plugin = await loadPlugin()
    await plugin.dispose()
  })
}

export type { NativeAudioState }
