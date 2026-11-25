import assert from 'node:assert/strict'
import {
  bookmarkedWord,
  click,
  otherWord,
  passagePosition,
  readAndBookmark,
  searchForPassage,
  visiblePassage,
} from '../smoke-reading-fixture.mjs'

/** Read the rendered paragraph so a settings value changing without a reader update cannot pass. */
function paragraphFontSize(browser) {
  return browser.execute(() => parseFloat(getComputedStyle(document.querySelector('.document-viewer-shown .document-html-content p')).fontSize))
}

async function returnAfterResize(browser, progress) {
  await readAndBookmark(browser, progress)
  progress('open search result B: ' + otherWord)
  await click(browser, '.doc-header .back-button')
  await searchForPassage(browser, otherWord)
  await visiblePassage(browser, otherWord)

  progress('increase the reader font size')
  const before = await paragraphFontSize(browser)
  await click(browser, 'button[aria-label="Reading appearance"]')
  await click(browser, 'button[aria-label="Increase Font Size"]')
  await browser.keys('Escape')
  await browser.$('[role="dialog"][aria-label="Reader settings"]').waitForDisplayed({ reverse: true })
  await browser.waitUntil(async () => (await paragraphFontSize(browser)) > before,
    { timeoutMsg: 'Expected larger rendered paragraph text' })

  // The appearance button is in the header. Page down if returning focus there hid the floating bookmark control.
  const actions = 'button[aria-label="Bookmark Actions"]'
  if (!await browser.$(actions).isDisplayed()) {
    await browser.keys('PageDown')
    await browser.$(actions).waitForDisplayed()
  }
  await visiblePassage(browser, otherWord)
  // A is below B. Returning to the document opening or staying at B must both fail.
  const position = await passagePosition(browser, bookmarkedWord)
  assert.ok(position && position.top > position.height, 'Expected bookmarked passage A below the viewport before Return')
  progress('return to bookmarked passage A: ' + bookmarkedWord)
  await click(browser, actions)
  await click(browser, '//*[@role="menuitem" and normalize-space(.)="Jump to Bookmark"]')
  await visiblePassage(browser, bookmarkedWord)
  const after = await paragraphFontSize(browser)
  assert.ok(after > before, 'Expected the larger font to persist after Return')
  progress(`confirmed ${bookmarkedWord} in viewport after font change from ${before}px to ${after}px`)
}

export const phases = [returnAfterResize]
