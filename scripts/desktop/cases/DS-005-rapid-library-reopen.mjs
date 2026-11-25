import assert from 'node:assert/strict'
import {
  checkRestoredPassageAndTop,
  click,
  openFromLibrary,
  readAndBookmark,
  visiblePassage,
} from '../smoke-reading-fixture.mjs'

/** Bound both Library opens in one process. The hook test controls when the old timeout fires. */
async function rapidReopen(browser, progress, { pid, appPid }) {
  await readAndBookmark(browser, progress)
  await click(browser, '.doc-header .back-button')
  progress('open bookmarked document twice from Library')
  const started = performance.now()
  await openFromLibrary(browser)
  await visiblePassage(browser)
  await click(browser, '.doc-header .back-button')
  await openFromLibrary(browser)
  const elapsed = Math.ceil(performance.now() - started)
  assert.ok(elapsed < 20000, `Expected two Library opens within 20 seconds, took ${elapsed} ms`)
  progress(`verify restored passage after second open within ${elapsed} ms`)
  await checkRestoredPassageAndTop(browser)
  assert.equal(appPid(), pid, 'Expected both Library opens in the original process')
}

export const phases = [rapidReopen]
