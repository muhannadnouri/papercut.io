import type { Mark, MarkKind, ScalarRange } from './types'

const MAX_MARKS = 128

/** Browser offsets count UTF-16 units; native marks count scalars and must end on complete graphemes. */
function graphemeBoundaries(text: string): Set<number> {
  if (typeof Intl.Segmenter !== 'function') return new Set()
  const boundaries = new Set([0])
  let scalar = 0
  for (const { segment } of new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(text)) {
    scalar += Array.from(segment).length
    boundaries.add(scalar)
  }
  return boundaries
}

/** Zero-width locks are attachment anchors: touching one is safe, enclosing one is not. */
export function rangeAllowed(text: string, range: ScalarRange, locked: ScalarRange[]): boolean {
  if (!Number.isSafeInteger(range.start) || !Number.isSafeInteger(range.end) || range.start >= range.end) return false
  const boundaries = graphemeBoundaries(text)
  return boundaries.has(range.start) && boundaries.has(range.end) && !locked.some(lock => lock.start === lock.end
    ? range.start < lock.start && lock.start < range.end
    : range.start < lock.end && lock.start < range.end)
}

/** Read-only textarea selectionStart/End use UTF-16 too; reject partial Unicode clusters instead of widening. */
export function selectionFromUtf16(text: string, start: number, end: number): ScalarRange | null {
  if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start < 0 || start >= end || end > text.length) return null
  const before = text.slice(0, start), through = text.slice(0, end)
  const result = { start: Array.from(before).length, end: Array.from(through).length }
  const scalars = Array.from(text)
  if (scalars.slice(0, result.start).join('') !== before || scalars.slice(0, result.end).join('') !== through) return null
  return rangeAllowed(text, result, []) ? result : null
}

/** Textareas expose LF-normalized offsets; map CRLF/CR back to immutable native text before validating graphemes. */
export function selectionFromTextarea(text: string, start: number, end: number): ScalarRange | null {
  if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start < 0 || start >= end) return null
  const originalOffsets = [0]
  for (let offset = 0; offset < text.length;) {
    offset += text[offset] === '\r' && text[offset + 1] === '\n' ? 2 : 1
    originalOffsets.push(offset)
  }
  if (end >= originalOffsets.length) return null
  return selectionFromUtf16(text, originalOffsets[start], originalOffsets[end])
}

/** Ordered DOM ranges also cover backward keyboard/touch selections; exact text rejects stale or foreign nodes. */
export function selectionRange(root: HTMLElement, text: string, selection: Selection | null): ScalarRange | null {
  if (!selection || selection.rangeCount !== 1 || root.textContent !== text) return null
  const range = selection.getRangeAt(0)
  if (range.collapsed || !root.contains(range.startContainer) || !root.contains(range.endContainer)) return null
  const prefix = root.ownerDocument.createRange()
  prefix.selectNodeContents(root)
  prefix.setEnd(range.startContainer, range.startOffset)
  const before = prefix.toString()
  prefix.setEnd(range.endContainer, range.endOffset)
  const through = prefix.toString()
  if (!text.startsWith(before) || !text.startsWith(through) || text.slice(before.length, through.length) !== range.toString()) return null
  return selectionFromUtf16(text, before.length, through.length)
}

/** Union coverage, rather than a single overlapping mark, determines the toolbar's pressed/mixed state. */
export function markState(marks: Mark[], range: ScalarRange, kind: MarkKind): boolean | 'mixed' {
  const overlaps = marks.filter(mark => mark.kind === kind && mark.range.start < range.end && range.start < mark.range.end)
    .map(mark => mark.range).sort((a, b) => a.start - b.start)
  if (!overlaps.length) return false
  let covered = range.start
  for (const mark of overlaps) {
    if (mark.start > covered) return 'mixed'
    covered = Math.max(covered, mark.end)
    if (covered >= range.end) return true
  }
  return 'mixed'
}

/** Removing a partial mark splits it; adding emphasis never joins across protected zero-width anchors. */
export function toggleMark(marks: Mark[], range: ScalarRange, kind: MarkKind, locked: ScalarRange[]): Mark[] {
  // Text/grapheme admission belongs to rangeAllowed and native validation; this helper owns interval geometry only.
  const valid = (candidate: ScalarRange) => Number.isSafeInteger(candidate.start) && Number.isSafeInteger(candidate.end)
    && candidate.start >= 0 && candidate.start < candidate.end && !locked.some(lock => lock.start === lock.end
      ? candidate.start < lock.start && lock.start < candidate.end
      : candidate.start < lock.end && lock.start < candidate.end)
  if (marks.length > MAX_MARKS || !valid(range) || marks.some(mark => !valid(mark.range))) throw new Error('Invalid formatting range')
  let next: Mark[]
  if (markState(marks, range, kind) === true) {
    next = marks.flatMap(mark => {
      if (mark.kind !== kind || mark.range.end <= range.start || mark.range.start >= range.end) return [{ ...mark, range: { ...mark.range } }]
      const pieces: Mark[] = []
      if (mark.range.start < range.start) pieces.push({ kind, range: { start: mark.range.start, end: range.start } })
      if (range.end < mark.range.end) pieces.push({ kind, range: { start: range.end, end: mark.range.end } })
      return pieces
    })
  } else {
    next = [...marks.map(mark => ({ ...mark, range: { ...mark.range } })), { kind, range: { ...range } }]
  }
  next.sort((a, b) => a.kind.localeCompare(b.kind) || a.range.start - b.range.start || a.range.end - b.range.end)
  const merged: Mark[] = []
  for (const mark of next) {
    const previous = merged[merged.length - 1]
    if (previous?.kind === mark.kind && mark.range.start < previous.range.end) previous.range.end = Math.max(previous.range.end, mark.range.end)
    else merged.push(mark)
  }
  if (merged.length > MAX_MARKS) throw new Error('Formatting exceeds the 128-mark limit')
  return merged.sort((a, b) => a.range.start - b.range.start || a.range.end - b.range.end || a.kind.localeCompare(b.kind))
}

/** Render only text and semantic emphasis; even nested/overlapping marks cannot introduce HTML or change English. */
export function markedRuns(text: string, marks: Mark[]): Array<{ text: string; bold: boolean; italic: boolean }> {
  const scalars = Array.from(text)
  if (marks.some(mark => !Number.isSafeInteger(mark.range.start) || !Number.isSafeInteger(mark.range.end)
    || mark.range.start < 0 || mark.range.start >= mark.range.end || mark.range.end > scalars.length)) throw new Error('Invalid formatting range')
  const cuts = [...new Set([0, scalars.length, ...marks.flatMap(mark => [mark.range.start, mark.range.end])])].sort((a, b) => a - b)
  const runs: Array<{ text: string; bold: boolean; italic: boolean }> = []
  for (let index = 1; index < cuts.length; index++) {
    const start = cuts[index - 1], end = cuts[index]
    const active = marks.filter(mark => mark.range.start <= start && end <= mark.range.end)
    const bold = active.some(mark => mark.kind === 'bold'), italic = active.some(mark => mark.kind === 'italic')
    const previous = runs[runs.length - 1]
    const slice = scalars.slice(start, end).join('')
    if (previous?.bold === bold && previous.italic === italic) previous.text += slice
    else runs.push({ text: slice, bold, italic })
  }
  return runs
}
