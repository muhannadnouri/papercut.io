import {
  checkRestoredPassageAndTop,
  openFromLibrary,
  readAndBookmark,
} from '../smoke-reading-fixture.mjs'

/** Open from Library so only bookmark restoration can account for the passage being visible after restart. */
async function reopen(browser, progress) {
  progress('reopen from Library')
  await openFromLibrary(browser)
  progress('check restored bookmark')
  await checkRestoredPassageAndTop(browser)
}

// Each phase runs in a new app process against the same disposable profile.
export const phases = [readAndBookmark, reopen]
