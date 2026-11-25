import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { click } from '../smoke-reading-fixture.mjs'

const file = fileURLToPath(new URL('../../fixtures/desktop-smoke/Harbor Signals.papercut-audiobook', import.meta.url))
const target = 'Amber beacons guide the pilot home.'

/** Read the player's rendered progress and the actual reader highlight after selection has been consumed. */
function playbackSample(browser) {
  return browser.execute(() => {
    const status = document.querySelector('.audio-floating-status.tts-status-playing')
    if (!status) {
      return null
    }
    const range = Array.from(CSS.highlights.get('tts-current') ?? [])[0]
    return {
      chunk: status.querySelector('span')?.textContent,
      percent: Number(/^Current chunk (\d+)% complete$/.exec(status.querySelector('.tts-meter')?.getAttribute('aria-label'))?.[1]),
      clock: status.innerText,
      highlight: range?.toString(),
    }
  })
}

async function playFromSelection(browser, progress, { application }) {
  progress('import the three-chunk Harbor Signals audiobook')
  execFileSync(application, [file], { timeout: 15000 })
  const paragraph = await browser.$(`//*[contains(@class,"document-viewer-shown")]//p[text()="${target}"]`)
  await paragraph.waitForDisplayed()
  const options = '.audio-controls .audio-menu-btn'
  await browser.$(options).waitForClickable()

  // Select using the WebView's native Selection API, keeping font metrics out of the fixture.
  const selected = await browser.execute(element => {
    const range = document.createRange()
    range.selectNodeContents(element)
    const selection = window.getSelection()
    selection.removeAllRanges()
    selection.addRange(range)
    return selection.toString()
  }, paragraph)
  assert.equal(selected, target)
  await click(browser, options)
  progress('play from selection in the final passage')
  await click(browser, '//button[normalize-space(.)="Play from selection"]')
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
