import assert from 'node:assert/strict'
import {
  playbackSample,
  startSelectedPassage,
  target,
} from '../smoke-playback-fixture.mjs'

async function playFromSelection(browser, progress, { application }) {
  await startSelectedPassage(browser, progress, application)
  let first
  await browser.waitUntil(async () => {
    first = await playbackSample(browser)
    return Boolean(first)
  }, { timeoutMsg: 'Expected saved audio playback to start' })
  progress('first playing chunk: ' + first.chunk)
  assert.equal(first.chunk, 'Chunk 3/3', 'Expected playback to start in the selected final chunk')

  let baseline
  let last
  await browser.waitUntil(async () => {
    last = await playbackSample(browser)
    if (last?.chunk !== 'Chunk 3/3' || last.highlight !== target) {
      return false
    }
    if (!last.clock.includes('/ 0:12') || !Number.isFinite(last.percent)) {
      return false
    }
    baseline ??= last.percent
    return last.percent > baseline
  }, { timeoutMsg: 'Expected advancing progress with the final passage highlighted' })
  progress(`confirmed final passage highlighted while progress advanced from ${baseline}% to ${last.percent}%`)
}

export const phases = [playFromSelection]
