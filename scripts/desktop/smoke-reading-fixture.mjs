import assert from 'node:assert/strict'

export const title = 'Northern Reading Log'
export const bookmarkedWord = 'Amberquartz'
export const otherWord = 'Silverwillow'
const paragraphs = Array.from({ length: 80 }, (_, i) => 'Reading entry ' + (i + 1) + '. The library opens early.')
paragraphs[18] = otherWord + ' grew beside the trail.'
paragraphs[55] = bookmarkedWord + ' was found along the riverbank.'
const story = [title, ...paragraphs].join('\n\n')

export async function click(browser, selector) {
  const element = await browser.$(selector)
  await element.waitForClickable()
  await element.click()
}
export const tab = name => '//button[@role="tab" and .//span[normalize-space(.)="' + name + '"]]'

/** Measure the word in place. A scrolling WebDriver element lookup here could move the reader and conceal a navigation regression. */
export async function passagePosition(browser, target = bookmarkedWord) {
  return browser.execute((title, target) => {
    const body = document.querySelector('.document-viewer-shown .document-html-content')
    if (!body || !body.textContent.includes(title)) {
      return null
    }
    const walker = document.createTreeWalker(body, NodeFilter.SHOW_TEXT)
    while (walker.nextNode()) {
      const node = walker.currentNode
      const start = node.textContent.indexOf(target)
      if (start === -1) {
        continue
      }
      const range = document.createRange()
      range.setStart(node, start)
      range.setEnd(node, start + target.length)
      const rect = range.getBoundingClientRect()
      // Hit testing also excludes text hidden by a sticky header or clipped by the reader.
      const hit = document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2)
      if (rect.width <= 0) {
        return null
      }
      return {
        visible: hit !== null && node.parentElement.contains(hit),
        top: rect.top,
        height: innerHeight,
      }
    }
    return null
  }, title, target)
}

/** Smooth scrolling can carry the viewport past a word. Sample twice before accepting its final position. */
export async function visiblePassage(browser, target = bookmarkedWord) {
  let previousTop
  await browser.waitUntil(async () => {
    const position = await passagePosition(browser, target)
    const settled = position?.visible && position.top === previousTop
    previousTop = position?.visible ? position.top : undefined
    return Boolean(settled)
  }, {
    timeoutMsg: 'Expected the target word in the visible reader passage: ' + target,
  })
}

export async function searchForPassage(browser, target) {
  await click(browser, tab('Search'))
  await click(browser, '//label[.//input[@name="search-mode"] and contains(., "All Words")]')
  await browser.$('.search-input').setValue(target)
  await browser.keys('Enter')
  await click(browser, '//button[.//bdi[text()="' + title + '"] and contains(@class,"result-card-primary") and .//*[contains(@class,"result-excerpt") and contains(., "' + target + '")]]')
}

/** Establish that the target starts off screen before exercising search. */
export async function pasteReadingFixture(browser, progress) {
  progress('paste text through Library')
  await click(browser, tab('Library'))
  await click(browser, '.document-import-btn')
  await click(browser, '[role="menuitem"][aria-label^="Paste Text"]')
  await browser.$('.paste-text-dialog input').setValue(title)
  const textarea = await browser.$('.paste-text-dialog textarea')
  await textarea.click()
  // WebKit drops literal newlines in WebDriver text input. Enter produces real paragraph breaks.
  await browser.elementSendKeys(textarea.elementId, story.replaceAll('\n', '\uE007'))
  assert.equal(await textarea.getValue(), story, 'Expected the complete multi-paragraph fixture')
  await click(browser, '.paste-text-dialog button[type="submit"]')
  await browser.waitUntil(async () => {
    const position = await passagePosition(browser)
    return position && position.top > 2 * position.height
  }, { timeoutMsg: 'Fixture target should begin multiple screens below the opening' })
}

export async function readAndBookmark(browser, progress) {
  await pasteReadingFixture(browser, progress)
  await click(browser, '.doc-header .back-button')
  progress('search the imported document')
  await searchForPassage(browser, bookmarkedWord)
  progress('check search destination')
  await visiblePassage(browser)
  progress('save bookmark')
  await click(browser, 'button[aria-label="Save Bookmark"]')
  await browser.$('button[aria-label="Bookmark Actions"]').waitForDisplayed()
}

export async function openFromLibrary(browser) {
  await click(browser, tab('Library'))
  await click(browser, '//button[@aria-pressed and contains(., "Documents (")]')
  await browser.$('.document-filter-input').setValue(title)
  await click(browser, '//button[.//bdi[text()="' + title + '"]]')
}

export async function checkRestoredPassageAndTop(browser) {
  await visiblePassage(browser)
  await click(browser, '//div[@role="status" and contains(., "Jumped to bookmark.")]//button[normalize-space(.)="Top"]')
  await browser.waitUntil(async () => {
    const position = await passagePosition(browser)
    return position && position.top > 2 * position.height
  }, { timeoutMsg: 'Top should return to the beginning of the document' })
}
