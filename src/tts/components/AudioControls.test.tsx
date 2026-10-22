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
    expect(document.activeElement).toBe(button('tts.playAnywhere.beginning'))
    expect(header.contains(document.querySelector('.audio-playback-dock'))).toBe(false)
    await act(async () => document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })))
    expect(document.activeElement).toBe(button('tts.controls.showMenu'))
    await act(async () => button('tts.controls.showMenu').click()); await act(async () => button('tts.playAnywhere.beginning').click()); expect(config.playAnywhere!.onPlayFromBeginning).toHaveBeenCalledOnce()
    await act(async () => app.render(<AudioControls {...config} ttsState={{ ...config.ttsState, status: 'paused', currentChunkIndex: 1 }} playAnywhere={{ ...config.playAnywhere!, browsing: true }} />))
    await act(async () => button('tts.controls.resume').click()); expect(config.onResume).toHaveBeenCalledOnce()
    await act(async () => button('tts.playAnywhere.return').click()); expect(config.playAnywhere!.onReturnToAudio).toHaveBeenCalledOnce()
    expect(config.playAnywhere!.onPlayFromHere).toHaveBeenCalledOnce()
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
