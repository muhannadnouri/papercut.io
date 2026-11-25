import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import {
  click,
  tab,
} from '../smoke-reading-fixture.mjs'

const file = fileURLToPath(new URL('../../fixtures/desktop-smoke/Estuary Survey.pdf', import.meta.url))
const pageNumber = 6
const passage = 'Copper buoys mark the inlet.'

async function openFromLibrary(browser) {
  await click(browser, tab('Library'))
  await click(browser, '//button[@aria-pressed and contains(., "Books (")]')
  await click(browser, 'button.library-book-open[aria-label="Estuary Survey"]')
}

/** Read geometry without scrolling. PDF.js can keep a text layer mounted outside the viewing area. */
function passagePosition(browser) {
  return browser.execute((number, text) => {
    const container = document.querySelector('.document-viewer-shown .pdf-viewer-container')
    const page = container?.querySelector(`.page[data-page-number="${number}"]`)
    const line = Array.from(page?.querySelectorAll('.textLayer span') ?? []).find(span => span.textContent === text)
    const input = document.querySelector('.pdf-page-input')
    const selectedPage = input ? Number(input.value) : null
    if (!line || !page.querySelector('canvas')) {
      return {
        pageNumber: selectedPage,
        rendered: false,
        visible: false,
        below: null,
      }
    }
    const bounds = line.getBoundingClientRect()
    const y = bounds.top + bounds.height / 2
    const hit = document.elementFromPoint(bounds.left + bounds.width / 2, y)
    return {
      below: y > container.getBoundingClientRect().bottom,
      visible: bounds.width > 0 && bounds.height > 0 && hit !== null && page.contains(hit),
      pageNumber: selectedPage,
      rendered: true,
      top: bounds.top,
    }
  }, pageNumber, passage)
}

async function checkDestination(browser) {
  let last
  try {
    await browser.waitUntil(async () => {
      const previous = last
      last = await passagePosition(browser)
      return last.pageNumber === pageNumber && last.visible && previous?.pageNumber === pageNumber &&
        previous.visible && previous.top === last.top
    }, { timeoutMsg: 'Expected page 6 and the copper-buoy passage settled in the PDF viewport' })
  } catch (error) {
    throw new Error(error.message + ' (last PDF observation: ' + JSON.stringify(last ?? null) + ')', { cause: error })
  }
}

async function savePdfBookmark(browser, progress, { application }) {
  progress('import the seven-page Estuary Survey PDF')
  execFileSync(application, [file], { timeout: 15000 })
  await openFromLibrary(browser)
  const input = await browser.$('.pdf-page-input')
  await input.waitForEnabled()
  assert.equal(await input.getValue(), '1')
  progress('navigate to page 6 and reveal the off-screen passage')
  await input.setValue(String(pageNumber))
  await browser.keys('Enter')
  await browser.waitUntil(async () => {
    const position = await passagePosition(browser)
    return position.pageNumber === pageNumber && position.below
  }, { timeoutMsg: 'Expected the passage below the viewport at the start of page 6' })
  const target = await browser.$(`//*[@data-page-number="${pageNumber}"]//span[text()="${passage}"]`)
  await browser.execute(line => line.scrollIntoView({ block: 'center' }), target)
  await checkDestination(browser)
  await click(browser, 'button[aria-label="Save Bookmark"]')
  await browser.$('button[aria-label="Bookmark Actions"]').waitForDisplayed()
}

async function reopenPdfBookmark(browser, progress) {
  progress('reopen Estuary Survey from Library in the new process')
  await openFromLibrary(browser)
  progress('verify restored page 6 and visible copper-buoy passage')
  await checkDestination(browser)
}

export const phases = [savePdfBookmark, reopenPdfBookmark]
