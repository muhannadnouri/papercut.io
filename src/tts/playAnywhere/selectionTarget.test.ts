// @vitest-environment jsdom
import {
  afterEach,
  expect,
  it,
  vi,
} from 'vitest'
import { createHtmlTextTargetApi } from './htmlTextTarget'
import {
  bindSelectionIntent,
  readerSelection,
  selectedTextStart,
} from './selectionTarget'
afterEach(() => {
  document.getSelection()?.removeAllRanges()
  document.body.replaceChildren()
})
function setup() {
  const root = document.createElement('article')
  root.innerHTML = '<p>First 😀 <b>chosen phrase</b> tail.</p><p>Next paragraph.</p><p><a href="#ref"><sup>[1]</sup></a> narrated.</p>'
  document.body.append(root)
  const api = createHtmlTextTargetApi(root, () => null)
  return {
    root,
    ...api,
  }
}
function select(start: Node, from: number, end: Node = start, to = from + 1, backwards = false) {
  document.getSelection()!.setBaseAndExtent(backwards ? end : start, backwards ? to : from, backwards ? start : end, backwards ? from : to)
}
it.each([false, true])('maps a multi-node selection in source order without viewport geometry (backward=%s)', backwards => {
  const { root, api, dispose } = setup()
  try {
    const original = root.innerHTML
    const start = root.querySelector('b')!.firstChild!
    const end = root.querySelectorAll('p')[1].firstChild!
    select(start, 2, end, 4, backwards)
    const capture = api.captureSelection!()
    expect(capture.status).toBe('captured')
    if (capture.status !== 'captured') {
      throw new Error('Expected selection capture')
    }
    expect(capture.point).toEqual({
      format: 'html',
      segmentIndex: 0,
      offset: 'First 😀 ch'.length,
    })
    expect(capture.isCurrent()).toBe(true)
    expect(root.innerHTML).toBe(original)
  } finally {
    dispose()
  }
})
it('handles element boundaries, end-of-text boundaries and whitespace only within the selected range', () => {
  const { root, api, dispose } = setup()
  try {
    const paragraph = root.querySelector('p')!
    const bold = root.querySelector('b')!
    const tail = bold.nextSibling!
    select(paragraph, 1, paragraph, 2)
    expect(api.captureSelection!()).toMatchObject({
      status: 'captured',
      point: {
        segmentIndex: 0,
        offset: 'First 😀 '.length,
      },
    })
    select(bold.firstChild!, bold.textContent!.length, tail, 5)
    expect(api.captureSelection!()).toMatchObject({
      status: 'captured',
      point: {
        segmentIndex: 0,
        offset: 'First 😀 chosen phrase '.length,
      },
    })
    select(tail, 0, tail, 1)
    expect(api.captureSelection!()).toMatchObject({
      status: 'unavailable',
      reason: 'unmapped',
    })
    select(root, 1, root, 2)
    expect(api.captureSelection!()).toMatchObject({
      status: 'captured',
      point: {
        segmentIndex: 1,
        offset: 0,
      },
    })
  } finally {
    dispose()
  }
})
it('rejects collapsed/cross-reader selection, hidden text and excluded references without selecting other narration', () => {
  const { root, api, dispose } = setup()
  try {
    const text = root.querySelector('b')!.firstChild!
    select(text, 0, text, 0)
    expect(api.captureSelection!()).toMatchObject({ reason: 'no-selection' })
    const outside = document.createElement('p')
    outside.textContent = 'Outside'
    document.body.append(outside)
    select(text, 1, outside.firstChild!, 3)
    expect(readerSelection(root)).toBeNull()
    expect(api.captureSelection!()).toMatchObject({ reason: 'no-selection' })
    const reference = root.querySelector('sup')!.firstChild!
    select(reference, 0, reference, 3)
    expect(api.captureSelection!()).toMatchObject({ reason: 'unmapped' })
    root.querySelector('b')!.setAttribute('hidden', '')
    select(text, 0, text, 3)
    expect(api.captureSelection!()).toMatchObject({ reason: 'unmapped' })
  } finally {
    dispose()
  }
})
it('keeps a focus-safe snapshot but rejects changed live selection, replaced text and disposed views', () => {
  const { root, api, dispose } = setup()
  const text = root.querySelector('b')!.firstChild!
  select(text, 1, text, 4)
  const capture = api.captureSelection!()
  if (capture.status !== 'captured') {
    throw new Error('Expected selection')
  }
  const button = document.createElement('button')
  button.className = 'audio-controls'
  document.body.append(button)
  button.focus()
  document.getSelection()!.removeAllRanges()
  expect(capture.isCurrent()).toBe(true)
  select(text, 3, text, 5)
  expect(capture.isCurrent()).toBe(false)
  select(text, 1, text, 4)
  expect(capture.isCurrent()).toBe(true)
  button.blur()
  document.getSelection()!.removeAllRanges()
  expect(capture.isCurrent()).toBe(true)
  select(text, 1, text, 4)
  root.querySelector('b')!.textContent = 'replacement'
  expect(capture.isCurrent()).toBe(false)
  dispose()
  expect(api.captureSelection!()).toMatchObject({ reason: 'stale' })
})
it('finds the first selected character without crossing an element end boundary', () => {
  const root = document.createElement('div')
  root.innerHTML = '<p>   </p><p>Next</p>'
  document.body.append(root)
  const range = document.createRange()
  range.setStart(root, 0)
  range.setEnd(root, 1)
  expect(selectedTextStart(range)).toBeNull()
  range.setEnd(root, 2)
  expect(selectedTextStart(range)?.node.data).toBe('Next')
})

it('snapshots before keyboard focus moves, invalidates new reader input and removes listeners', () => {
  const shell = document.createElement('div')
  shell.innerHTML = '<main class="document-view"><p>Reader</p></main><button class="audio-controls">Menu</button>'
  document.body.append(shell)
  const actions = {
    snapshot: vi.fn(),
    invalidate: vi.fn(),
  }
  const dispose = bindSelectionIntent(shell, actions)
  document.body.dispatchEvent(new KeyboardEvent('keydown', {
    key: 'Tab',
    bubbles: true,
  }))
  expect(actions.snapshot).toHaveBeenCalledOnce()
  shell.querySelector('button')!.dispatchEvent(new KeyboardEvent('keydown', {
    key: 'Enter',
    bubbles: true,
  }))
  expect(actions.invalidate).not.toHaveBeenCalled()
  shell.querySelector('p')!.dispatchEvent(new Event('pointerdown', { bubbles: true }))
  expect(actions.invalidate).toHaveBeenCalledOnce()
  document.body.dispatchEvent(new KeyboardEvent('keydown', {
    key: 'ArrowRight',
    shiftKey: true,
    bubbles: true,
  }))
  expect(actions.invalidate).toHaveBeenCalledTimes(2)
  dispose()
  document.body.dispatchEvent(new KeyboardEvent('keydown', {
    key: 'Tab',
    bubbles: true,
  }))
  expect(actions.snapshot).toHaveBeenCalledOnce()
})
