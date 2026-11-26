// @vitest-environment jsdom
import {
  describe,
  expect,
  it,
  vi,
} from 'vitest'
import {
  readerSectionSelector,
  scrollToReaderElement,
  scrollToReaderRange,
} from './readerTarget'

describe('reader search-result section targets', () => {
  it('maps persisted section ordinals to generated reader markers', () => {
    expect(readerSectionSelector(12)).toBe('[data-papercut-section="12"]')
  })

  it('rejects values that cannot be persisted section ordinals', () => {
    expect(readerSectionSelector(-1)).toBeNull()
    expect(readerSectionSelector(1.5)).toBeNull()
    expect(readerSectionSelector(undefined)).toBeNull()
  })
})

it('reveals an enclosing disclosure without expanding the summary destination or its sibling', () => {
  const article = document.createElement('article')
  article.innerHTML = `<details><summary>Outer</summary><details><summary><h2>Inner</h2></summary>
    <p>Inside</p></details></details><details><summary>Other</summary></details>`
  const scroll = vi.spyOn(window, 'scrollTo').mockImplementation(() => {})
  try {
    scrollToReaderElement(article.querySelector('h2')!)
    expect(Array.from(article.querySelectorAll('details'), details => details.open)).toEqual([true, false, false])
  } finally {
    scroll.mockRestore()
  }
})

it('keeps an opening search hit and both toolbar rows clear of the status bar', () => {
  const header = document.createElement('header')
  header.style.scrollMarginTop = '62px'
  header.getBoundingClientRect = () => new DOMRect(0, 82, 390, 106)
  const range = document.createRange()
  const scroll = vi.spyOn(window, 'scrollTo').mockImplementation(() => {})
  try {
    range.getBoundingClientRect = () => new DOMRect(10, window.innerHeight / 2 + 42, 200, 24)
    scrollToReaderRange(range, header)
    expect(scroll).toHaveBeenLastCalledWith(expect.objectContaining({ top: 0 }))

    range.getBoundingClientRect = () => new DOMRect(10, window.innerHeight / 2 + 300, 200, 24)
    scrollToReaderRange(range, header)
    expect(scroll).toHaveBeenLastCalledWith(expect.objectContaining({ top: 300 }))
  } finally {
    scroll.mockRestore()
  }
})

it('preserves both toolbar rows for an opening section or hash and jumps normally to a later section', () => {
  const shell = document.createElement('div')
  shell.className = 'app-reader'
  shell.innerHTML = '<header class="doc-header" style="scroll-margin-top:62px"></header><section id="chapter"></section>'
  const section = shell.querySelector('section')!
  shell.querySelector('header')!.getBoundingClientRect = () => new DOMRect(0, 82, 390, 106)
  const scroll = vi.spyOn(window, 'scrollTo').mockImplementation(() => {})
  try {
    section.getBoundingClientRect = () => new DOMRect(0, 224, 390, 2000)
    scrollToReaderElement(section)
    expect(scroll).toHaveBeenLastCalledWith(expect.objectContaining({ top: 0 }))
    section.getBoundingClientRect = () => new DOMRect(0, 1224, 390, 2000)
    scrollToReaderElement(section)
    expect(scroll).toHaveBeenLastCalledWith(expect.objectContaining({ top: 1104 }))
  } finally {
    scroll.mockRestore()
  }
})
