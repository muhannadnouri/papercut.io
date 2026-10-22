// @vitest-environment jsdom
import { act, type ComponentProps } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, expect, it, vi } from 'vitest'
import { AudioControls } from './AudioControls'
vi.mock('react-i18next', async importOriginal => ({ ...await importOriginal<typeof import('react-i18next')>(), useTranslation: () => ({ t: (key: string) => key }) }))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
afterEach(() => document.body.replaceChildren())
function props(): ComponentProps<typeof AudioControls> {
  const noop = () => {}
  return { audiobookState: { status: 'idle', complete: true, cachedChunks: 2, totalChunks: 2, message: '', audioDurationSec: 0, wavBytes: 0, appliedThreadCount: null },
    canPlayAudiobook: true, canSaveAudiobook: false, canSkipBackward: false, canSkipForward: true, modelSetupLabel: null, saveInProgress: false,
    onManageSave: noop, onPause: vi.fn(), onRead: vi.fn(), onResume: vi.fn(), onSelectSavedAudiobook: noop, onJumpToChunk: noop, onPlaybackRateChange: noop, onOpenAudioSetup: noop, onSave: noop, onSkipBackward: noop, onSkipForward: noop, onStop: noop, onWordHighlightEnabledChange: noop,
    playbackRate: 1, savedAudiobooks: [], selectedAudiobookId: null, wordHighlightEnabled: true,
    ttsState: { status: 'idle', message: '', chunksGenerated: 2, chunksPlayed: 0, chunksTotal: 2, currentText: '', currentChunkIndex: null, pendingChunkIndex: null, currentChunkId: null, currentChunkProgress: 0, currentChunkTime: 0, currentChunkDuration: 0, chunkSummaries: [], chunks: [] },
    playAnywhere: { browsing: false, pending: false, feedback: null, onPlayFromHere: vi.fn(), onPlayFromBeginning: vi.fn(), onReturnToAudio: vi.fn() } }
}
it('uses explicit viewport/beginning actions, keeps Resume while browsing, and restores menu focus', async () => {
  const config = props(), shell = document.createElement('div'); shell.className = 'app-reader'
  const header = document.createElement('header'); shell.append(header); document.body.append(shell); const app = createRoot(header)
  const button = (name: string) => [...document.querySelectorAll('button')].find(b => b.getAttribute('aria-label') === name || b.textContent === name)!
  try {
    await act(async () => app.render(<AudioControls {...config} />))
    await act(async () => button('tts.playAnywhere.here').click()); expect(config.playAnywhere!.onPlayFromHere).toHaveBeenCalledOnce(); expect(config.onRead).not.toHaveBeenCalled()
    await act(async () => button('tts.controls.showMenu').click())
    expect(document.activeElement).toBe(document.querySelector('.audio-passage-here'))
    expect(header.contains(document.querySelector('.audio-playback-dock'))).toBe(false)
    await act(async () => document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })))
    expect(document.activeElement).toBe(button('tts.controls.showMenu'))
    await act(async () => button('tts.controls.showMenu').click()); await act(async () => button('tts.playAnywhere.beginning').click()); expect(config.playAnywhere!.onPlayFromBeginning).toHaveBeenCalledOnce()
    await act(async () => app.render(<AudioControls {...config} ttsState={{ ...config.ttsState, status: 'paused', currentChunkIndex: 1 }} playAnywhere={{ ...config.playAnywhere!, browsing: true }} />))
    await act(async () => button('tts.controls.resume').click()); expect(config.onResume).toHaveBeenCalledOnce()
    await act(async () => button('tts.playAnywhere.return').click()); expect(config.playAnywhere!.onReturnToAudio).toHaveBeenCalledOnce()
    expect(config.playAnywhere!.onPlayFromHere).toHaveBeenCalledOnce()
    expect(button('tts.playAnywhere.return').closest('.audio-floating-playback')).not.toBeNull()
    expect(document.querySelector('.audio-browse-actions')).toBeNull()
    expect(button('tts.playAnywhere.return').querySelector('svg')).not.toBeNull()
    button('tts.playAnywhere.return').focus()
    await act(async () => app.render(<AudioControls {...config} ttsState={{ ...config.ttsState, status: 'paused', currentChunkIndex: 1 }} />))
    expect(document.activeElement).toBe(button('tts.controls.resume'))
    await act(async () => app.render(<AudioControls {...config} playAnywhere={{ ...config.playAnywhere!, feedback: 'unmapped' }} />))
    expect(document.querySelector('[role="status"]')?.textContent).toBe('tts.playAnywhere.unmapped')
    await act(async () => app.render(<AudioControls {...config} ttsState={{ ...config.ttsState, status: 'error', message: 'Seek failed' }} />))
    expect(document.querySelector('[role="status"]')?.textContent).toBe('Seek failed')
    await act(async () => app.render(<AudioControls {...config} playAnywhere={undefined} />))
    await act(async () => button('tts.controls.playSaved').click())
    expect(config.onRead).toHaveBeenCalledOnce()
  } finally { await act(async () => app.unmount()) }
})

it('snapshots on menu pointerdown before focus and exposes a keyboard-focusable explicit selection action', async () => {
  const config = props(), element = document.createElement('div'); document.body.append(element); const app = createRoot(element)
  const snapshot = vi.fn(() => true), play = vi.fn(), close = vi.fn()
  try {
    await act(async () => app.render(<AudioControls {...config} playAnywhere={{ ...config.playAnywhere!, onSnapshotSelection: snapshot, onPlayFromSelection: play, onSelectionMenuClose: close }} />))
    const trigger = document.querySelector<HTMLButtonElement>('[aria-expanded="false"]')!
    await act(async () => trigger.dispatchEvent(new Event('pointerdown', { bubbles: true })))
    expect(snapshot).toHaveBeenCalledOnce()
    trigger.focus(); await act(async () => trigger.click())
    const action = [...document.querySelectorAll('button')].find(b => b.textContent === 'tts.playAnywhere.selection')!
    expect(document.activeElement?.textContent).toBe('tts.playAnywhere.here')
    action.focus(); expect(document.activeElement).toBe(action)
    await act(async () => action.click()); expect(play).toHaveBeenCalledOnce(); expect(close).toHaveBeenCalledOnce()
    expect(document.activeElement).toBe(trigger)
    await act(async () => app.render(<AudioControls {...config} playAnywhere={{ ...config.playAnywhere!, feedback: 'no-selection' }} />))
    expect(document.querySelector('[role="status"]')?.textContent).toBe('tts.playAnywhere.noSelection')
  } finally { await act(async () => app.unmount()) }
})

it('keeps passage commands in the popover, reports missing selection and closes before viewport capture', async () => {
  const config = props(), element = document.createElement('div'); document.body.append(element); const app = createRoot(element)
  const play = vi.fn(), here = vi.fn(() => expect(document.querySelector('.audio-chunk-menu')).toBeNull())
  try {
    await act(async () => app.render(<AudioControls {...config} ttsState={{ ...config.ttsState, status: 'playing' }}
      playAnywhere={{ ...config.playAnywhere!, onPlayFromHere: here, onSnapshotSelection: () => false, onPlayFromSelection: play }} />))
    expect(document.querySelector('.audio-passage-actions')).toBeNull()
    const trigger = document.querySelector<HTMLButtonElement>('[aria-expanded="false"]')!
    await act(async () => trigger.click())
    const commands = [...document.querySelectorAll<HTMLButtonElement>('.audio-passage-command')]
    expect(commands.map(b => b.textContent)).toEqual(['tts.playAnywhere.here', 'tts.playAnywhere.selection', 'tts.playAnywhere.beginning'])
    expect(commands.every(b => b.querySelector('svg')?.getAttribute('aria-hidden') === 'true')).toBe(true)
    expect(commands[1].disabled).toBe(true)
    expect(document.getElementById(commands[1].getAttribute('aria-describedby')!)?.textContent).toBe('tts.playAnywhere.noSelection')
    await act(async () => commands[1].click()); expect(play).not.toHaveBeenCalled()
    await act(async () => commands[0].click()); expect(here).toHaveBeenCalledOnce()
    expect(document.activeElement).toBe(trigger)
  } finally { await act(async () => app.unmount()) }
})
