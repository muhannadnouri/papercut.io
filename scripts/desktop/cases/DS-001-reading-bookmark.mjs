import {
  click,
  passagePosition,
  readAndBookmark,
  tab,
  title,
  visiblePassage,
} from '../smoke-reading-fixture.mjs'

/** Open from Library so only bookmark restoration can account for the passage being visible after restart. */
async function reopen(browser, progress) {
  progress('reopen from Library')
  await click(browser, tab('Library'))
  await click(browser, '//button[@aria-pressed and contains(., "Documents (")]')
  await browser.$('.document-filter-input').setValue(title)
  await click(browser, '//button[.//bdi[text()="' + title + '"]]')
  progress('check restored bookmark')
  await visiblePassage(browser)
  await click(browser, '//div[@role="status" and contains(., "Jumped to bookmark.")]//button[normalize-space(.)="Top"]')
  await browser.waitUntil(async () => {
    const position = await passagePosition(browser)
    return position && position.top > 2 * position.height
  }, { timeoutMsg: 'Top should return to the beginning of the document' })
}

// Each phase runs in a new app process against the same disposable profile.
export const phases = [readAndBookmark, reopen]
