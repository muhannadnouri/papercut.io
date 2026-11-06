import {
  invoke,
  isTauri,
} from '@tauri-apps/api/core'
import { forgetNativeAudio } from '../tts/playback/nativeMobileAudio'
import { isMobileUserAgent } from '../utils/platform'

// tesseract.js caches trained data here through idb-keyval. Nothing else uses IndexedDB.
const TESSERACT_CACHE_DATABASE = 'keyval-store'

export type AppResetBlocker = 'imports' | 'search' | 'audio' | 'transfer'

export interface AppResetSummary {
  documents: number
  audiobooks: number
  totalBytes: number
  /** Folders holding Papercut's data. Empty on mobile, where users can't browse them. */
  locations: string[]
  blockedBy: AppResetBlocker | null
}

interface AppResetOutcome {
  blockedBy: AppResetBlocker | null
  restarting: boolean
}

interface AppResetPending {
  failedPaths: string[]
}

let completedReset: AppResetPending | null = null

/** Advisory counts for the dialog. `app_reset_request` re-checks for active jobs. */
export function getAppResetSummary(): Promise<AppResetSummary> {
  return invoke<AppResetSummary>('app_reset_summary')
}

/**
 * Start the erase. Resolves with a blocker if a job must finish first. Otherwise
 * the app restarts or reloads, and {@link finishPendingAppReset} completes it.
 *
 * On desktop the process may exit before the reply arrives, so this promise may
 * never settle; callers must stay in their busy state rather than await a result.
 */
export async function eraseAppData(): Promise<AppResetBlocker | null> {
  if (isMobileUserAgent()) {
    // Players hold audiobook files open, and iOS keeps hard links until disposal.
    await forgetNativeAudio().catch((error) => {
      console.warn('Failed to release native audio before erasing', error)
    })
  }
  const outcome = await invoke<AppResetOutcome>('app_reset_request')
  if (outcome.blockedBy) {
    return outcome.blockedBy
  }
  if (!outcome.restarting) {
    window.location.reload()
  }
  return null
}

/** Best effort: this cache is rebuilt from bundled assets, so failure is harmless. */
function deleteDatabase(name: string): Promise<void> {
  return new Promise((resolve) => {
    if (typeof indexedDB === 'undefined') {
      resolve()
      return
    }
    const request = indexedDB.deleteDatabase(name)
    request.onsuccess = () => resolve()
    request.onerror = () => resolve()
    // Startup must not wait on a connection that may never close.
    request.onblocked = () => resolve()
  })
}

/**
 * Clear WebView storage for an erase that has finished natively. Runs at boot
 * rather than before reloading, since `pagehide` flushes can rewrite localStorage
 * during shutdown. Never rejects, because a failure here must not block launch.
 *
 * Clears everything rather than known keys: this origin belongs only to Papercut, and
 * keys use two prefixes (`papercut.` and `papercut:`), so a filter could silently miss
 * bookmarks. Must stay free of imports that touch storage, because it runs before them.
 */
export async function finishPendingAppReset(): Promise<void> {
  if (!isTauri()) {
    return
  }
  try {
    const pending = await invoke<AppResetPending | null>('app_reset_pending')
    if (!pending) {
      return
    }
    window.localStorage.clear()
    await deleteDatabase(TESSERACT_CACHE_DATABASE)
    await invoke('app_reset_acknowledge')
    completedReset = pending
  } catch (error) {
    console.error('Failed to finish erasing app data', error)
  }
}

/** The erase finished during this launch, if any, for the one-time notice. */
export function getCompletedAppReset(): AppResetPending | null {
  return completedReset
}
