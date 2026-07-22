import { afterEach, describe, expect, it, vi } from 'vitest'
import { BrowserAudioCache } from './browserAudioCache'

describe('BrowserAudioCache', () => {
  afterEach(() => vi.restoreAllMocks())

  it('keeps a bounded window and revokes every discarded URL', () => {
    const revoke = vi.spyOn(URL, 'revokeObjectURL')
    const cache = new BrowserAudioCache()
    for (let index = 0; index < 8; index += 1) {
      cache.enqueue({ index, chunkId: String(index), text: String(index), wav: new ArrayBuffer(1) })
    }

    cache.prune(4, 0, 8)

    expect(cache.has(0)).toBe(true)
    expect(cache.has(1)).toBe(false)
    expect(cache.has(2)).toBe(true)
    expect(cache.generatedCount).toBe(8)
    expect(revoke).toHaveBeenCalledTimes(1)

    cache.clear()
    expect(cache.generatedCount).toBe(0)
    expect(revoke).toHaveBeenCalledTimes(8)
  })
})
