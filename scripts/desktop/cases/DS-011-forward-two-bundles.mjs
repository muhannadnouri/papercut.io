import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import {
  click,
  tab,
} from '../smoke-reading-fixture.mjs'

const books = [
  ['Harbor Signals', 'A ferry crosses the outer harbor.'],
  ['Lantern Field Notes', 'The blue lantern marks the northern footbridge.'],
]

/** Deliver both paths in one invocation so the native argument list and import queue are exercised together. */
async function forwardTwoBundles(browser, progress, { application, appPid, pid }) {
  progress('forward Harbor Signals and Lantern Field Notes in one second launch')
  const paths = books.map(([title]) => fileURLToPath(new URL('../../fixtures/desktop-smoke/' + title + '.papercut-audiobook', import.meta.url)))
  execFileSync(application, paths, { timeout: 15000 })
  const paragraph = text => '//*[contains(@class,"document-viewer-shown")]//p[text()="' + text + '"]'
  progress('wait for the second bundle, Lantern Field Notes, to finish opening')
  await browser.$(paragraph(books[1][1])).waitForDisplayed()
  await click(browser, '.doc-header .back-button')
  await click(browser, tab('Library'))
  await click(browser, '//button[@aria-pressed and contains(., "Documents (")]')
  for (const [title, passage] of books) {
    progress('verify one Library entry and reopen ' + title)
    await browser.$('.document-filter-input').setValue(title)
    const row = '//button[.//bdi[text()="' + title + '"]]'
    await browser.$(row).waitForDisplayed()
    assert.equal((await browser.$$(row)).length, 1, 'Expected one Library entry for ' + title)
    await click(browser, row)
    await browser.$(paragraph(passage)).waitForDisplayed()
    await click(browser, '.doc-header .back-button')
  }
  assert.equal(appPid(), pid, 'Expected both books in the original app process')
}

export const phases = [forwardTwoBundles]
