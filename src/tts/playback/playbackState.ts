import type { NativeAudiobookPlayback } from '../api/nativeTts'

export const DEFAULT_PLAYBACK_RATE = 1

/** Find the chunk owning a playback timestamp without scanning long audiobooks. */
export function findPlaybackChunk(
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

export function normalizePlaybackRate(rate: number): number {
  if (!Number.isFinite(rate) || rate <= 0) return DEFAULT_PLAYBACK_RATE
  return Number(Math.min(3, Math.max(0.5, rate)).toFixed(2))
}

export function isNativeMobilePlatform(platform: string): boolean {
  return platform === 'android' || platform === 'ios'
}

/** Ignore browser play interruptions caused by quickly replacing an audio source. */
export function isTransientPlaybackInterruption(err: unknown): boolean {
  if (!(err instanceof Error)) return false
  const name = err.name.toLowerCase()
  const message = err.message.toLowerCase()
  return name === 'aborterror' ||
    message.includes('interrupted') ||
    message.includes('new load request') ||
    message.includes('pause')
}

export function textPreview(text: string): string {
  const normalized = text.replace(/\s+/g, ' ').trim()
  if (normalized.length <= 96) return normalized
  return normalized.slice(0, 95).trimEnd() + '...'
}
