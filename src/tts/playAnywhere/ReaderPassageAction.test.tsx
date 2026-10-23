// @vitest-environment jsdom
import {
  act,
} from 'react'
import { createRoot } from 'react-dom/client'
import {
  afterEach,
  expect,
  it,
  vi,
} from 'vitest'
import { ReaderPassageAction } from './ReaderPassageAction'
import { bindSelectionIntent } from './selectionTarget'

vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
afterEach(() => document.body.replaceChildren())

it('uses the source-order selection through control focus, then returns to viewport after new reader input', async () => {
  const shell = document.createElement('div')
  shell.className = 'app-reader'
  shell.innerHTML = '<main class="document-view"><p>Opening words and later words.</p></main><aside></aside>'
  document.body.append(shell)
  const app = createRoot(shell.querySelector('aside')!)
  const snapshot = vi.fn(() => false)
  const here = vi.fn()
  const selected = vi.fn()
  const invalidate = vi.fn()
  const dispose = bindSelectionIntent(shell, { snapshot, invalidate })
  try {
    await act(async () => app.render(<ReaderPassageAction hasPlayed={false} pending={false}
      onSnapshotSelection={snapshot} onPlayFromHere={here} onPlayFromSelection={selected} />))
    const button = shell.querySelector('button')!
    expect(button.textContent).toBe('')
    expect(button.getAttribute('aria-label')).toBe('tts.playAnywhere.here')
    expect(button.classList.contains('reader-passage-action-initial')).toBe(true)
    const text = shell.querySelector('p')!.firstChild!
    const selection = document.getSelection()!
    // Backward drag still advertises the document-order range start.
    selection.setBaseAndExtent(text, 19, text, 8)
    await act(async () => document.dispatchEvent(new Event('selectionchange')))
    expect(button.textContent).toBe('')
    expect(button.getAttribute('aria-label')).toBe('tts.playAnywhere.selection')
    await act(async () => app.render(<ReaderPassageAction hasPlayed pending={false}
      onSnapshotSelection={snapshot} onPlayFromHere={here} onPlayFromSelection={selected} />))
    expect(button.textContent).toBe('tts.playAnywhere.selection')
    snapshot.mockReturnValue(true)
    await act(async () => button.dispatchEvent(new Event('pointerdown', { bubbles: true })))
    expect(invalidate).not.toHaveBeenCalled()
    button.focus()
    selection.removeAllRanges()
    await act(async () => document.dispatchEvent(new Event('selectionchange')))
    expect(button.textContent).toBe('tts.playAnywhere.selection')
    snapshot.mockReturnValue(false)
    await act(async () => button.click())
    expect(selected).toHaveBeenCalledOnce()
    expect(here).not.toHaveBeenCalled()
    button.blur()
    await act(async () => document.dispatchEvent(new Event('selectionchange')))
    await act(async () => button.click())
    expect(here).toHaveBeenCalledOnce()
    await act(async () => app.render(<ReaderPassageAction hasPlayed pending
      onSnapshotSelection={snapshot} onPlayFromHere={here} onPlayFromSelection={selected} />))
    expect(button.disabled).toBe(true)
    expect(button.getAttribute('aria-busy')).toBe('true')
    expect(button.querySelector('.spinner')).not.toBeNull()
  } finally {
    dispose()
    await act(async () => app.unmount())
  }
})

it('moves focused idle shortcut to the surviving player when playback starts', async () => {
  const shell = document.createElement('div')
  shell.className = 'app-reader'
  shell.innerHTML = '<main class="document-view"><p>Reader text.</p></main><aside></aside>'
  document.body.append(shell)
  const app = createRoot(shell.querySelector('aside')!)
  const noop = () => {}
  try {
    await act(async () => app.render(<ReaderPassageAction hasPlayed pending={false}
      onSnapshotSelection={() => false} onPlayFromHere={noop} onPlayFromSelection={noop} />))
    shell.querySelector('button')!.focus()
    await act(async () => app.render(<div className="audio-floating-playback">
      <button className="audio-primary-btn">Pause</button>
    </div>))
    expect(document.activeElement?.textContent).toBe('Pause')
  } finally {
    await act(async () => app.unmount())
  }
})
