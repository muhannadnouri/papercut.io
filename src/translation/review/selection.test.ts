// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { markedRuns, markState, rangeAllowed, selectionFromTextarea, selectionFromUtf16, selectionRange, toggleMark } from './selection'
import type { Mark, ScalarRange } from './types'

const range = (start: number, end: number): ScalarRange => ({ start, end })
const bold = (start: number, end: number): Mark => ({ kind: 'bold', range: range(start, end) })
const italic = (start: number, end: number): Mark => ({ kind: 'italic', range: range(start, end) })

function element(html: string): HTMLDivElement {
  const root = document.createElement('div')
  root.innerHTML = html
  document.body.append(root)
  return root
}

function select(start: Node, startOffset: number, end: Node, endOffset: number): Selection {
  const selection = window.getSelection()!
  selection.removeAllRanges()
  const selected = document.createRange()
  selected.setStart(start, startOffset)
  selected.setEnd(end, endOffset)
  selection.addRange(selected)
  return selection
}

afterEach(() => { window.getSelection()?.removeAllRanges(); document.body.replaceChildren(); vi.restoreAllMocks(); vi.unstubAllGlobals() })

describe('read-only English selection geometry', () => {
  it('maps split DOM runs and backward keyboard/touch selections to exact scalar offsets', () => {
    const root = element('A <strong>😀bo</strong><em>ld</em> Z')
    const start = root.querySelector('strong')!.firstChild!, end = root.querySelector('em')!.firstChild!
    const selection = select(start, 0, end, 2)
    expect(selectionRange(root, 'A 😀bold Z', selection)).toEqual(range(2, 7))
    selection.setBaseAndExtent(end, 2, start, 0)
    expect(selectionRange(root, 'A 😀bold Z', selection)).toEqual(range(2, 7))
    selection.selectAllChildren(root)
    expect(selectionRange(root, 'A 😀bold Z', selection)).toEqual(range(0, 9))
  })

  it('rejects collapsed, outside, cross-block and stale selections even when the text repeats', () => {
    const root = element('same'), outside = element('same')
    expect(selectionRange(root, 'same', null)).toBeNull()
    expect(selectionRange(root, 'same', select(root.firstChild!, 1, root.firstChild!, 1))).toBeNull()
    expect(selectionRange(root, 'same', select(outside.firstChild!, 0, outside.firstChild!, 4))).toBeNull()
    expect(selectionRange(root, 'same', select(root.firstChild!, 0, outside.firstChild!, 4))).toBeNull()
    expect(selectionRange(root, 'previous', select(root.firstChild!, 0, root.firstChild!, 4))).toBeNull()
  })

  it('rejects surrogate, combining-mark and emoji-family splits without rounding or changing English', () => {
    const root = element('😀é 👩‍👩‍👧‍👦')
    const text = root.textContent!, node = root.firstChild!
    expect(selectionRange(root, text, select(node, 0, node, 1))).toBeNull()
    expect(selectionRange(root, text, select(node, 2, node, 3))).toBeNull()
    expect(selectionRange(root, text, select(node, 5, node, 7))).toBeNull()
    expect(selectionRange(root, text, select(node, 2, node, 4))).toEqual(range(1, 3))
    expect(selectionRange(root, text, select(node, 5, node, text.length))).toEqual(range(4, 11))
  })

  it('admits complete Arabic graphemes and fails closed when the browser lacks grapheme segmentation', () => {
    const text = 'مَرْحَبًا'
    expect(rangeAllowed(text, range(0, 2), [])).toBe(true)
    expect(rangeAllowed(text, range(0, 1), [])).toBe(false)
    vi.stubGlobal('Intl', { ...Intl, Segmenter: undefined })
    expect(rangeAllowed(text, range(0, Array.from(text).length), [])).toBe(false)
    vi.unstubAllGlobals()
  })

  it('maps native read-only textarea keyboard offsets without rounding emoji, accents or backwards intervals', () => {
    const text = '😀é مَ'
    expect(selectionFromUtf16(text, 0, 2)).toEqual(range(0, 1))
    expect(selectionFromUtf16(text, 2, 4)).toEqual(range(1, 3))
    expect(selectionFromUtf16(text, 5, 7)).toEqual(range(4, 6))
    for (const [start, end] of [[0, 1], [2, 3], [5, 6], [4, 2], [-1, 2], [0, 999], [1.5, 4]]) {
      expect(selectionFromUtf16(text, start, end)).toBeNull()
    }
  })

  it('maps real textarea CRLF/CR normalization back to exact native scalar coordinates after both line endings', () => {
    const text = 'First.\r\nSecond.\r😀Third.'
    const textarea = document.createElement('textarea')
    textarea.readOnly = true
    textarea.value = text
    expect(textarea.value).toBe('First.\nSecond.\n😀Third.')
    textarea.setSelectionRange(7, 14)
    expect(selectionFromTextarea(text, textarea.selectionStart, textarea.selectionEnd)).toEqual(range(8, 15))
    textarea.setSelectionRange(15, 17)
    const emoji = selectionFromTextarea(text, textarea.selectionStart, textarea.selectionEnd)
    expect(emoji).toEqual(range(16, 17))
    expect(Array.from(text).slice(emoji!.start, emoji!.end).join('')).toBe('😀')
    expect(markedRuns(text, [{ kind: 'bold', range: emoji! }])).toEqual([
      { text: 'First.\r\nSecond.\r', bold: false, italic: false },
      { text: '😀', bold: true, italic: false },
      { text: 'Third.', bold: false, italic: false },
    ])
    expect(text).toBe('First.\r\nSecond.\r😀Third.')
  })

  it('selects an entire CRLF as one native grapheme and rejects normalized surrogate, accent and bounds cuts', () => {
    const text = 'A\r\n😀é\rB'
    expect(selectionFromTextarea(text, 1, 2)).toEqual(range(1, 3))
    expect(selectionFromTextarea(text, 6, 7)).toEqual(range(6, 7))
    expect(selectionFromTextarea(text, 2, 4)).toEqual(range(3, 4))
    for (const [start, end] of [[2, 3], [4, 5], [-1, 1], [2, 2], [4, 2], [0, 99], [0.5, 2]]) {
      expect(selectionFromTextarea(text, start, end)).toBeNull()
    }
    expect(selectionFromTextarea('A\nB', 2, 3)).toEqual(selectionFromUtf16('A\nB', 2, 3))
    expect(selectionFromTextarea('', 0, 1)).toBeNull()
  })

  it('rejects invalid ranges, literal intersections and enclosed zero-width attachment anchors', () => {
    const text = 'whole literal sentence'
    const locks = [range(6, 13), range(17, 17)]
    for (const candidate of [range(-1, 1), range(1, 1), range(5, 23), range(0, 22), range(0.5, 2), range(0, Infinity)]) {
      expect(rangeAllowed(text, candidate, locks)).toBe(false)
    }
    expect(rangeAllowed(text, range(0, 5), locks)).toBe(true)
    expect(rangeAllowed(text, range(13, 17), locks)).toBe(true)
    expect(rangeAllowed(text, range(17, 22), locks)).toBe(true)
  })
})

describe('bounded bold and italic edits', () => {
  it('distinguishes union coverage, partial coverage and independent kinds', () => {
    const marks = [bold(0, 2), bold(2, 4), italic(1, 3)]
    expect(markState(marks, range(0, 4), 'bold')).toBe(true)
    expect(markState(marks, range(0, 4), 'italic')).toBe('mixed')
    expect(markState(marks, range(4, 6), 'bold')).toBe(false)
    expect(markState([bold(0, 1), bold(2, 4)], range(0, 4), 'bold')).toBe('mixed')
  })

  it('fills mixed coverage and splits removed marks while retaining nested italic and immutable input', () => {
    const marks = [bold(0, 2), italic(1, 5)]
    const original = structuredClone(marks)
    const added = toggleMark(marks, range(1, 4), 'bold', [])
    expect(added).toEqual([bold(0, 4), italic(1, 5)])
    const removed = toggleMark(added, range(1, 3), 'bold', [])
    expect(removed).toEqual([bold(0, 1), italic(1, 5), bold(3, 4)])
    expect(marks).toEqual(original)
    expect(added).toEqual([bold(0, 4), italic(1, 5)])
  })

  it('never merges adjacent edits across a locked anchor or formats protected content', () => {
    expect(toggleMark([bold(0, 2)], range(2, 4), 'bold', [range(2, 2)])).toEqual([bold(0, 2), bold(2, 4)])
    expect(() => toggleMark([], range(1, 3), 'bold', [range(2, 2)])).toThrow('Invalid formatting range')
    expect(() => toggleMark([], range(0, 5), 'italic', [range(1, 3)])).toThrow('Invalid formatting range')
  })

  it('allows the native 128-mark budget but rejects additions or splits beyond it', () => {
    const marks = Array.from({ length: 128 }, (_, index) => bold(index * 2, index * 2 + 1))
    expect(toggleMark(marks.slice(0, 127), range(254, 255), 'bold', [])).toHaveLength(128)
    expect(() => toggleMark(marks, range(256, 257), 'italic', [])).toThrow('128-mark limit')
    const split = [bold(0, 10), ...marks.slice(1).map(mark => bold(mark.range.start + 10, mark.range.end + 10))]
    expect(() => toggleMark(split, range(3, 5), 'bold', [])).toThrow('128-mark limit')
    expect(marks).toHaveLength(128)
  })

  it('renders nested, crossing B/I as safe exact text including whitespace, emoji and Arabic', () => {
    const text = ' A😀é\nمَ <unsafe> '
    const runs = markedRuns(text, [bold(1, 5), italic(3, 9)])
    expect(runs.map(run => run.text).join('')).toBe(text)
    expect(runs).toEqual([
      { text: ' ', bold: false, italic: false },
      { text: 'A😀', bold: true, italic: false },
      { text: 'é', bold: true, italic: true },
      { text: '\nمَ ', bold: false, italic: true },
      { text: '<unsafe> ', bold: false, italic: false },
    ])
    expect(markedRuns('', [])).toEqual([])
    expect(() => markedRuns(text, [bold(0, 999)])).toThrow('Invalid formatting range')
  })
})
