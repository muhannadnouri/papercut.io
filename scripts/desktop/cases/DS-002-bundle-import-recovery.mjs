import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const bundle = fileURLToPath(new URL('../../fixtures/desktop-smoke/Lantern Field Notes.papercut-audiobook', import.meta.url))
const passage = 'The blue lantern marks the northern footbridge.'

/** The forwarding process exits while the original WebDriver session shows the imported passage. */
async function openValidBundle(browser, progress, { application, pid, appPid }) {
  progress('forward valid bundle to running app')
  execFileSync(application, [bundle], { timeout: 15000 })
  const paragraph = await browser.$('.document-viewer-shown .document-html-content p')
  await paragraph.waitForDisplayed()
  assert.equal(await paragraph.getText(), passage)
  assert.equal(appPid(), pid, 'Expected the original app to display the delivered bundle')
  progress('confirmed valid passage in original app PID ' + pid)
}

export const phases = [openValidBundle]
