import assert from 'node:assert/strict'
import {
  firstPlaying,
  playbackSample,
  startMidChunk,
} from '../smoke-playback-fixture.mjs'
import { click } from '../smoke-reading-fixture.mjs'

async function stopAndRestart(browser, progress, { application }) {
  await startMidChunk(browser, progress, application)
  await click(browser, 'button[aria-label="Stop audiobook"]')
  await click(browser, '.audio-controls .audio-menu-btn')
  await click(browser, '//button[normalize-space(.)="Play from beginning"]')
  const first = await firstPlaying(browser, 'Chunk 1/3')
  assert.ok(first <= 10, 'Expected the new playback to start near zero, saw ' + first)
  await browser.waitUntil(async () => {
    const sample = await playbackSample(browser)
    return sample?.chunk === 'Chunk 1/3' && sample.clock.includes('/ 0:18') &&
      sample.highlight === 'A ferry crosses the outer harbor.' && sample.percent > first
  }, { timeoutMsg: 'Expected advancing playback and highlighting in the opening passage' })
  progress(`after Stop, fresh playback started at ${first}% in the first chunk`)
}

export const phases = [stopAndRestart]
