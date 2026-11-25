import assert from 'node:assert/strict'
import { setTimeout as delay } from 'node:timers/promises'
import {
  playbackSample,
  firstPlaying,
  startMidChunk,
  target,
} from '../smoke-playback-fixture.mjs'
import { click } from '../smoke-reading-fixture.mjs'

async function pauseAndResume(browser, progress, { application }) {
  const initial = await startMidChunk(browser, progress, application)
  await browser.waitUntil(async () => {
    const sample = await playbackSample(browser)
    return sample?.highlight === target && sample.percent >= initial + 15
  }, { timeoutMsg: 'Expected the highlighted final passage to advance beyond the selection offset' })
  await click(browser, 'button[aria-label="Pause audiobook"]')
  const resume = 'button[aria-label="Resume audiobook"]'
  await browser.$(resume).waitForClickable()
  const paused = await playbackSample(browser, 'paused')
  assert.equal(paused?.chunk, 'Chunk 3/3')
  assert.ok(paused.percent >= initial + 15)
  // A second of uninterrupted playback changes the 12-second fixture's meter by about eight points.
  await delay(1100)
  const held = await playbackSample(browser, 'paused')
  assert.equal(held?.percent, paused.percent, 'Expected the clock to stay steady while paused')
  await click(browser, resume)
  const resumed = await firstPlaying(browser, 'Chunk 3/3')
  progress(`selection ${initial}%, paused ${paused.percent}%, first resumed sample ${resumed}%`)
  assert.ok(resumed >= paused.percent - 2 && resumed <= paused.percent + 10,
    'Expected Resume to continue from the paused position')
  await browser.waitUntil(async () => {
    const sample = await playbackSample(browser)
    return sample?.chunk === 'Chunk 3/3' && sample.highlight === target && sample.percent > resumed
  }, { timeoutMsg: 'Expected resumed progress and the same highlighted passage' })
}

export const phases = [pauseAndResume]
