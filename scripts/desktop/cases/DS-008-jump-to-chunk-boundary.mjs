import {
  playbackSample,
  startMidChunk,
  target,
} from '../smoke-playback-fixture.mjs'
import { click } from '../smoke-reading-fixture.mjs'

async function jumpToBoundary(browser, progress, { application }) {
  const initial = await startMidChunk(browser, progress, application)
  await click(browser, '.audio-floating-playback .audio-menu-btn')
  await click(browser, `button.audio-chunk-item[title="${target}"]`)
  // The current chunk was already playing. Require its clock to move back to the boundary.
  let boundary
  await browser.waitUntil(async () => {
    const sample = await playbackSample(browser)
    if (sample?.chunk === 'Chunk 3/3' && sample.highlight === target && sample.percent <= 10) {
      boundary = sample.percent
      return true
    }
    return false
  }, { timeoutMsg: 'Expected the same final chunk to restart at its boundary' })
  await browser.waitUntil(async () => {
    const sample = await playbackSample(browser)
    return sample?.chunk === 'Chunk 3/3' && sample.percent > boundary && sample.percent < initial
  }, { timeoutMsg: 'Expected playback to advance from the chunk boundary' })
  progress(`ordinary jump moved chunk three from ${initial}% to ${boundary}%`)
}

export const phases = [jumpToBoundary]
