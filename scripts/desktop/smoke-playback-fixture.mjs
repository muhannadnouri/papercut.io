import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { click } from './smoke-reading-fixture.mjs'

export const target = 'Amber beacons guide the pilot home.'

/** Read the player's rendered progress and the actual reader highlight after selection has been consumed. */
export function playbackSample(browser, state = 'playing') {
  return browser.execute(state => {
    const status = document.querySelector('.audio-floating-status.tts-status-' + state)
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
  }, state)
}

export async function startSelectedPassage(browser, progress, application, selectionText = target) {
  await selectBundlePassage(browser, progress, application, 'Harbor Signals', target, selectionText)
}

/** Import the saved edition, then snapshot the exact reader selection before the menu takes focus. */
export async function selectBundlePassage(browser, progress, application, title, paragraphText, selectionText = paragraphText) {
  progress('import ' + title)
  const file = fileURLToPath(new URL('../fixtures/desktop-smoke/' + title + '.papercut-audiobook', import.meta.url))
  execFileSync(application, [file], { timeout: 15000 })
  const paragraph = await browser.$(`//*[contains(@class,"document-viewer-shown")]//p[text()="${paragraphText}"]`)
  await paragraph.waitForDisplayed()
  const options = '.audio-controls .audio-menu-btn'
  await browser.$(options).waitForClickable()

  // Select using the WebView's native Selection API, keeping font metrics out of the fixture.
  const selected = await browser.execute((element, text) => {
    const range = document.createRange()
    const start = element.textContent.indexOf(text)
    range.setStart(element.firstChild, start)
    range.setEnd(element.firstChild, start + text.length)
    const selection = window.getSelection()
    selection.removeAllRanges()
    selection.addRange(range)
    return selection.toString()
  }, paragraph, selectionText)
  assert.equal(selected, selectionText)
  await click(browser, options)
  progress('play from selection: ' + selectionText)
  await click(browser, '//button[normalize-space(.)="Play from selection"]')
}

/** Capture the first playing sample: polling until it catches up would let a restart pass. */
export async function firstPlaying(browser, chunk) {
  let sample
  await browser.waitUntil(async () => {
    sample = await playbackSample(browser)
    return Number.isFinite(sample?.percent)
  }, { timeoutMsg: 'Expected the playing chunk and progress meter' })
  assert.equal(sample.chunk, chunk)
  return sample.percent
}

/** The fixture maps “guide” to 40% of chunk three. Confirm playback starts away from its boundary. */
export async function startMidChunk(browser, progress, application) {
  await startSelectedPassage(browser, progress, application, 'guide')
  const initial = await firstPlaying(browser, 'Chunk 3/3')
  assert.ok(initial >= 35 && initial <= 55, 'Expected a proportional start near 40%, saw ' + initial)
  return initial
}
