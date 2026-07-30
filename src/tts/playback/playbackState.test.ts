import { describe, expect, it } from 'vitest'
import type { NativeAudiobookPlayback } from '../api/nativeTts'
import {
  findPlaybackChunk,
  isNativeMobilePlatform,
  isTransientPlaybackInterruption,
  normalizePlaybackRate,
  textPreview,
} from './playbackState'

const playback: NativeAudiobookPlayback = {
  title: 'Test Audiobook',
  audioUrl: 'audio.wav',
  audioDurationSec: 9,
  wavBytes: 100,
  chunks: [
    { index: 0, chunkId: 'one', startSec: 0, durationSec: 2 },
    { index: 1, chunkId: 'two', startSec: 2, durationSec: 3 },
    { index: 2, chunkId: 'three', startSec: 5, durationSec: 4 },
  ],
}

describe('playback state helpers', () => {
  it('finds chunk boundaries and clamps timestamps outside the audiobook', () => {
    expect(findPlaybackChunk(playback, -1)?.chunkId).toBe('one')
    expect(findPlaybackChunk(playback, 1.99)?.chunkId).toBe('one')
    expect(findPlaybackChunk(playback, 2)?.chunkId).toBe('two')
    expect(findPlaybackChunk(playback, 5)?.chunkId).toBe('three')
    expect(findPlaybackChunk(playback, 12)?.chunkId).toBe('three')
    expect(findPlaybackChunk({ ...playback, chunks: [] }, 0)).toBeNull()
  })

  it('normalizes playback rates to the supported range', () => {
    expect(normalizePlaybackRate(Number.NaN)).toBe(1)
    expect(normalizePlaybackRate(0)).toBe(1)
    expect(normalizePlaybackRate(0.1)).toBe(0.5)
    expect(normalizePlaybackRate(1.236)).toBe(1.24)
    expect(normalizePlaybackRate(4)).toBe(3)
  })

  it('classifies native platforms and transient browser interruptions', () => {
    expect(isNativeMobilePlatform('android')).toBe(true)
    expect(isNativeMobilePlatform('ios')).toBe(true)
    expect(isNativeMobilePlatform('linux')).toBe(false)

    const abort = new Error('play() was interrupted')
    abort.name = 'AbortError'
    expect(isTransientPlaybackInterruption(abort)).toBe(true)
    expect(isTransientPlaybackInterruption(new Error('decoder failed'))).toBe(false)
    expect(isTransientPlaybackInterruption('interrupted')).toBe(false)
  })

  it('produces compact diagnostic previews', () => {
    expect(textPreview('  one\n two  ')).toBe('one two')
    expect(textPreview('x'.repeat(120))).toHaveLength(98)
    expect(textPreview('x'.repeat(120)).endsWith('...')).toBe(true)
  })
})
