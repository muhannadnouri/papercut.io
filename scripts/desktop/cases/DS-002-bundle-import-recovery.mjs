import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const fixture = name => fileURLToPath(new URL('../../fixtures/desktop-smoke/' + name, import.meta.url))
const library = '//button[@role="tab" and .//span[normalize-space(.)="Library"]]'
const sourceText = 'A copper compass rests beside the map.'
const sourceParagraph = '//*[contains(@class,"document-viewer-shown")]//p[normalize-space(.)="' + sourceText + '"]'
const alert = '.audiobook-action-toast[role="alert"]'

/** Deliver through the OS entry point so both attempts exercise the same native importer. */
function deliver(application, name) {
  execFileSync(application, [fixture(name)], { timeout: 15000 })
}

async function click(browser, selector) {
  const control = await browser.$(selector)
  await control.waitForClickable()
  await control.click()
}

/** A marker specific to the current surface must stay visible while the error is up and after dismissal. */
async function dismissInvalidBundle(browser, progress, application, surface, marker) {
  progress('deliver invalid bundle in ' + surface)
  await browser.$(marker).waitForDisplayed()
  deliver(application, 'Invalid.papercut-audiobook')
  const notice = await browser.$(alert)
  await notice.waitForDisplayed()
  assert.equal((await browser.$$(alert)).length, 1)
  assert.match(await notice.getText(), /not a (valid or supported|current) Papercut audiobook bundle/i)
  assert.ok(await browser.$(marker).isDisplayed(), surface + ' should remain visible with the error')
  progress('dismiss invalid bundle error in ' + surface)
  await click(browser, alert + ' button[aria-label="Dismiss audiobook notice"]')
  await notice.waitForDisplayed({ reverse: true })
  assert.ok(await browser.$(marker).isDisplayed(), surface + ' should remain visible after dismissal')
}

async function recoverFromInvalidBundle(browser, progress, { application, pid, appPid }) {
  await click(browser, library)
  await dismissInvalidBundle(browser, progress, application, 'Library', '.document-import-btn')

  progress('open a distinct document for the reader error')
  await click(browser, '.document-import-btn')
  await click(browser, '[role="menuitem"][aria-label^="Paste Text"]')
  await browser.$('.paste-text-dialog input').setValue('Compass Notes')
  await browser.$('.paste-text-dialog textarea').setValue(sourceText)
  await click(browser, '.paste-text-dialog button[type="submit"]')
  await dismissInvalidBundle(browser, progress, application, 'reader', sourceParagraph)

  progress('forward valid bundle after reader dismissal')
  deliver(application, 'Lantern Field Notes.papercut-audiobook')
  await browser.$('//*[contains(@class,"document-viewer-shown")]//p[normalize-space(.)="The blue lantern marks the northern footbridge."]').waitForDisplayed()
  assert.equal(appPid(), pid, 'Expected the original app to display the recovered passage')
  progress('confirmed valid passage in original app PID ' + pid)

  progress('check for one imported title in Library')
  await click(browser, '.doc-header .back-button')
  await click(browser, library)
  await click(browser, '//button[@aria-pressed and contains(., "Documents (")]')
  await browser.$('.document-filter-input').setValue('Lantern Field Notes')
  const result = '//button[.//bdi[text()="Lantern Field Notes"]]'
  await browser.$(result).waitForDisplayed()
  assert.equal((await browser.$$(result)).length, 1)
}

export const phases = [recoverFromInvalidBundle]
