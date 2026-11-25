import assert from 'node:assert/strict'
import {
  firstPlaying,
  playbackSample,
  selectBundlePassage,
} from '../smoke-playback-fixture.mjs'
import { click } from '../smoke-reading-fixture.mjs'

/** The colophon exists only in the source. Playing from the beginning must still use the valid saved audio. */
async function recoverFromUnmappedSelection(browser, progress, { application }) {
  const colophon = 'Printed by the harbor office.'
  await selectBundlePassage(browser, progress, application, 'Harbor Office Notes', colophon)
  progress('verify unmatched passage feedback and idle playback')
  await browser.$('//*[@role="status" and contains(., "Audio could not be matched to this passage.")]').waitForDisplayed()
  // Observe a short idle window after feedback, before issuing the explicit recovery command.
  const stayedIdle = await browser.execute(() => new Promise(done => {
    const until = performance.now() + 500
    function observe() {
      if (!document.querySelector('.audio-compact-row .audio-primary-btn:enabled') ||
          document.querySelector('.audio-floating-status') || CSS.highlights.get('tts-current')?.size) {
        done(false)
      } else if (performance.now() >= until) {
        done(true)
      } else {
        requestAnimationFrame(observe)
      }
    }
    observe()
  }))
  assert.ok(stayedIdle, 'Unmapped selection must leave playback idle with no active highlight')

  progress('choose Play from beginning and verify advancing playback of the first passage')
  await click(browser, '.audio-controls .audio-menu-btn')
  await click(browser, '//button[normalize-space(.)="Play from beginning"]')
  const initial = await firstPlaying(browser, 'Chunk 1/2')
  assert.ok(initial <= 10, 'Expected playback near the start, saw ' + initial)
  await browser.waitUntil(async () => {
    const sample = await playbackSample(browser)
    return sample?.chunk === 'Chunk 1/2' && sample.highlight === 'The harbor office opens at dawn.' && sample.percent > initial
  }, { timeoutMsg: 'Expected the first narrated passage to highlight and advance after explicit recovery' })
}

export const phases = [recoverFromUnmappedSelection]
