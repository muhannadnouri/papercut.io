import assert from 'node:assert/strict'
import {
  click,
  openFromLibrary,
  pasteReadingFixture,
  searchForPassage,
  visiblePassage,
  bookmarkedWord,
} from '../smoke-reading-fixture.mjs'

/** Leave the reader at a known late passage without saving it, then reopen via Library. */
async function openWithoutBookmark(browser, progress) {
  await pasteReadingFixture(browser, progress)
  await click(browser, '.doc-header .back-button')
  progress('open the late search result without saving a bookmark')
  await searchForPassage(browser, bookmarkedWord)
  await visiblePassage(browser)
  await browser.$('button[aria-label="Save Bookmark"]').waitForDisplayed()
  await click(browser, '.doc-header .back-button')
  progress('reopen the unbookmarked document from Library')
  await openFromLibrary(browser)
  progress('check the first entry and absence of the bookmark notice after Library reopen')
  await visiblePassage(browser, 'Reading entry 1.')
  assert.equal(await browser.$('//div[@role="status" and contains(., "Jumped to bookmark.")]').isDisplayed(), false)
  progress('return to Library with Back')
  await click(browser, '.doc-header .back-button')
  await browser.$('.document-filter-input').waitForDisplayed()
  progress('unbookmarked book reopened at its first entry and Back worked')
}

export const phases = [openWithoutBookmark]
