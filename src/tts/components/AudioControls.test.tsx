// @vitest-environment jsdom
import {
  act,
  type ComponentProps,
} from 'react'
import { createRoot } from 'react-dom/client'
import {
  afterEach,
  expect,
  it,
  vi,
} from 'vitest'
import { AudioControls } from './AudioControls'
vi.mock('react-i18next', async importOriginal => ({
  ...await importOriginal<typeof import('react-i18next')>(),
  useTranslation: () => ({ t: (key: string) => key }),
}))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
afterEach(() => document.body.replaceChildren())

it('keeps playback speed reachable through Options without the optional feature', async () => {
  const config = props()
  const element = document.createElement('div')
  document.body.append(element)
  const app = createRoot(element)
  const rate = vi.fn()
  try {
    await act(async () => app.render(<AudioControls {...config} playAnywhere={undefined}
      onPlaybackRateChange={rate} ttsState={{
        ...config.ttsState,
        status: 'paused',
      }} />))
    await act(async () => document.querySelector<HTMLButtonElement>('.audio-menu-btn')!.click())
    expect(document.activeElement).toBe(document.querySelector('[role="switch"]'))
    const speed = document.querySelector<HTMLButtonElement>('.audio-menu-rate')!
    expect(speed.textContent).toBe('1x')
    expect(speed.getAttribute('aria-label')).toBe('tts.controls.playbackSpeed')
    await act(async () => speed.click())
    expect(rate).toHaveBeenCalledWith(1.25)
    expect(document.querySelector('.audio-chunk-menu')).not.toBeNull()
    expect(document.querySelector('.audio-passage-actions')).toBeNull()
  } finally {
    await act(async () => app.unmount())
  }
})
it('exposes Word Highlight as a stable named switch with the existing immediate callback', async () => {
  const config = props()
  const element = document.createElement('div')
  document.body.append(element)
  const app = createRoot(element)
  const change = vi.fn()
  try {
    const render = (enabled: boolean) => app.render(<AudioControls {...config}
      wordHighlightEnabled={enabled} onWordHighlightEnabledChange={change} />)
    await act(async () => render(true))
    await act(async () => document.querySelector<HTMLButtonElement>('[aria-expanded="false"]')!.click())
    const input = document.querySelector<HTMLInputElement>('[role="switch"]')!
    expect(input.type).toBe('checkbox')
    expect(input.checked).toBe(true)
    expect(input.getAttribute('aria-label')).toBe('tts.controls.wordHighlight')
    await act(async () => input.closest('label')!.click())
    expect(change).toHaveBeenLastCalledWith(false)
    await act(async () => render(false))
    expect(input.checked).toBe(false)
    expect(input.getAttribute('aria-label')).toBe('tts.controls.wordHighlight')
    input.focus()
    expect(document.activeElement).toBe(input)
    await act(async () => input.click())
    expect(change).toHaveBeenLastCalledWith(true)
    expect(document.querySelector('.audio-chunk-menu')).not.toBeNull()
    expect(input.closest('button')).toBeNull()
  } finally {
    await act(async () => app.unmount())
  }
})
function props(): ComponentProps<typeof AudioControls> {
  const noop = () => {}
  return {
    audiobookState: {
      status: 'idle',
      complete: true,
      cachedChunks: 2,
      totalChunks: 2,
      message: '',
      audioDurationSec: 0,
      wavBytes: 0,
      appliedThreadCount: null,
    },
    canPlayAudiobook: true,
    canSaveAudiobook: false,
    canSkipBackward: false,
    canSkipForward: true,
    modelSetupLabel: null,
    saveInProgress: false,
    onManageSave: noop,
    onPause: vi.fn(),
    onRead: vi.fn(),
    onResume: vi.fn(),
    onSelectSavedAudiobook: noop,
    onJumpToChunk: noop,
    onPlaybackRateChange: noop,
    onOpenAudioSetup: noop,
    onSave: noop,
    onSkipBackward: noop,
    onSkipForward: noop,
    onStop: noop,
    onWordHighlightEnabledChange: noop,
    playbackRate: 1,
    savedAudiobooks: [],
    selectedAudiobookId: null,
    wordHighlightEnabled: true,
    ttsState: {
      status: 'idle',
      message: '',
      chunksGenerated: 2,
      chunksPlayed: 0,
      chunksTotal: 2,
      currentText: '',
      currentChunkIndex: null,
      pendingChunkIndex: null,
      currentChunkId: null,
      currentChunkProgress: 0,
      currentChunkTime: 0,
      currentChunkDuration: 0,
      chunkSummaries: [],
      chunks: [],
    },
    playAnywhere: {
      browsing: false,
      pending: false,
      feedback: null,
      onPlayFromHere: vi.fn(),
      onPlayFromBeginning: vi.fn(),
      onReturnToAudio: vi.fn(),
    },
  }
}
it('uses explicit viewport/beginning actions, keeps Resume while browsing, and restores menu focus', async () => {
  const config = props(); const shell = document.createElement('div'); shell.className = 'app-reader'
  const header = document.createElement('header'); shell.append(header); document.body.append(shell); const app = createRoot(header)
  const button = (name: string) => [...document.querySelectorAll('button')].find(b => b.getAttribute('aria-label') === name || b.textContent === name)!
  try {
    await act(async () => app.render(<AudioControls {...config} />))
    await act(async () => button('tts.playAnywhere.here').click()); expect(config.playAnywhere!.onPlayFromHere).toHaveBeenCalledOnce(); expect(config.onRead).not.toHaveBeenCalled()
    await act(async () => button('tts.controls.playbackOptions').click())
    expect(document.activeElement).toBe(document.querySelector('.audio-passage-here'))
    expect(header.contains(document.querySelector('.audio-playback-dock'))).toBe(false)
    await act(async () => document.dispatchEvent(new KeyboardEvent('keydown', {
      key: 'Escape',
      bubbles: true,
    })))
    expect(document.activeElement).toBe(button('tts.controls.playbackOptions'))
    await act(async () => button('tts.controls.playbackOptions').click()); await act(async () => button('tts.playAnywhere.beginning').click()); expect(config.playAnywhere!.onPlayFromBeginning).toHaveBeenCalledOnce()
    await act(async () => app.render(<AudioControls {...config} ttsState={{
      ...config.ttsState,
      status: 'paused',
      currentChunkIndex: 1,
    }} playAnywhere={{
      ...config.playAnywhere!,
      browsing: true,
    }} />))
    await act(async () => button('tts.controls.resume').click()); expect(config.onResume).toHaveBeenCalledOnce()
    await act(async () => button('tts.playAnywhere.return').click()); expect(config.playAnywhere!.onReturnToAudio).toHaveBeenCalledOnce()
    expect(config.playAnywhere!.onPlayFromHere).toHaveBeenCalledOnce()
    expect(button('tts.playAnywhere.return').closest('.audio-floating-playback')).not.toBeNull()
    expect(document.querySelector('.audio-browse-actions')).toBeNull()
    expect(button('tts.playAnywhere.return').querySelector('svg')).not.toBeNull()
    button('tts.playAnywhere.return').focus()
    await act(async () => app.render(<AudioControls {...config} ttsState={{
      ...config.ttsState,
      status: 'paused',
      currentChunkIndex: 1,
    }} />))
    expect(document.activeElement).toBe(button('tts.controls.resume'))
    await act(async () => app.render(<AudioControls {...config} playAnywhere={{
      ...config.playAnywhere!,
      feedback: 'unmapped',
    }} />))
    expect(document.querySelector('[role="status"]')?.textContent).toBe('tts.playAnywhere.unmapped')
    await act(async () => app.render(<AudioControls {...config} ttsState={{
      ...config.ttsState,
      status: 'error',
      message: 'Seek failed',
    }} />))
    expect(document.querySelector('[role="status"]')?.textContent).toBe('Seek failed')
    await act(async () => app.render(<AudioControls {...config} playAnywhere={undefined} />))
    await act(async () => button('tts.controls.playSaved').click())
    expect(config.onRead).toHaveBeenCalledOnce()
  } finally {
    await act(async () => app.unmount())
  }
})

it.each(['wheel', 'pointer', 'keyboard'])('keeps the list position after %s browsing until the popover is reopened', async method => {
  const config = props()
  const element = document.createElement('div')
  document.body.append(element)
  const app = createRoot(element)
  const chunks = Array.from({ length: 175 }, (_, index) => ({
    index,
    chunkId: 'saved-' + index,
    textPreview: 'Saved passage ' + index,
  }))
  const render = (index: number) => app.render(<AudioControls {...config}
    ttsState={{
      ...config.ttsState,
      status: 'playing',
      currentChunkIndex: index,
      chunkSummaries: chunks,
    }} />)
  try {
    await act(async () => render(20))
    const trigger = document.querySelector<HTMLButtonElement>('[aria-expanded="false"]')!
    await act(async () => trigger.click())
    let list = document.querySelector<HTMLDivElement>('.audio-chunk-list')!
    const initial = list.scrollTop
    await act(async () => render(21))
    expect(list.scrollTop).toBeGreaterThan(initial)
    await act(async () => {
      if (method === 'keyboard') {
        list.querySelector('button')!.focus()
      } else {
        list.dispatchEvent(new Event(method === 'wheel' ? 'wheel' : 'pointerdown', { bubbles: true }))
      }
      list.scrollTop = 400
      list.dispatchEvent(new Event('scroll', { bubbles: true }))
    })
    await act(async () => render(22))
    expect(list.scrollTop).toBe(400)
    const views = document.querySelectorAll<HTMLButtonElement>('.audio-menu-views button')
    await act(async () => views[1].click())
    await act(async () => views[0].click())
    list = document.querySelector<HTMLDivElement>('.audio-chunk-list')!
    expect(list.scrollTop).toBe(400)
    await act(async () => document.dispatchEvent(new KeyboardEvent('keydown', {
      key: 'Escape',
      bubbles: true,
    })))
    await act(async () => trigger.click())
    expect(document.querySelector<HTMLDivElement>('.audio-chunk-list')!.scrollTop).not.toBe(400)
  } finally {
    await act(async () => app.unmount())
  }
})

it('keeps one live status with a decorative pending spinner and visible recovery feedback', async () => {
  const config = props()
  const element = document.createElement('div')
  document.body.append(element)
  const app = createRoot(element)
  try {
    await act(async () => app.render(<AudioControls {...config}
      ttsState={{
        ...config.ttsState,
        status: 'loading',
        message: 'Checking saved audio...',
      }} />))
    const status = document.querySelector<HTMLElement>('[role="status"]')!
    expect(status.textContent).toBe('Checking saved audio...')
    expect(status.getAttribute('aria-live')).toBe('polite')
    expect(status.querySelector('.spinner')?.getAttribute('aria-hidden')).toBe('true')
    expect(status.closest('.audio-playback-surface')).toBe(document.querySelector('.audio-floating-playback')?.parentElement)
    expect(document.querySelectorAll('[role="status"]')).toHaveLength(1)
    await act(async () => app.render(<AudioControls {...config}
      playAnywhere={{
        ...config.playAnywhere!,
        feedback: 'unmapped',
      }} />))
    expect(status.textContent).toBe('tts.playAnywhere.unmapped')
    expect(status.classList.contains('audio-target-error')).toBe(true)
    expect(status.querySelector('.spinner')).toBeNull()
    await act(async () => app.render(<AudioControls {...config} />))
    expect(status.textContent).toBe('')
    expect(status.children).toHaveLength(0)
  } finally {
    await act(async () => app.unmount())
  }
})

it('offers compact Return with its full accessible name and a labeled Options action', async () => {
  const config = props()
  const element = document.createElement('div')
  document.body.append(element)
  const app = createRoot(element)
  try {
    await act(async () => app.render(<AudioControls {...config}
      ttsState={{
        ...config.ttsState,
        status: 'paused',
      }}
      playAnywhere={{
        ...config.playAnywhere!,
        browsing: true,
      }} />))
    const compact = document.querySelector<HTMLButtonElement>('.audio-return-control')!
    expect(compact.getAttribute('aria-label')).toBe('tts.playAnywhere.return')
    expect(compact.textContent).toBe('tts.playAnywhere.returnShort')
    const trigger = document.querySelector<HTMLButtonElement>('[aria-expanded="false"]')!
    await act(async () => trigger.click())
    const labeled = [...document.querySelectorAll<HTMLButtonElement>('.audio-passage-command')]
      .find(button => button.textContent === 'tts.playAnywhere.return')!
    await act(async () => labeled.click())
    expect(config.playAnywhere!.onReturnToAudio).toHaveBeenCalledOnce()
    expect(config.onResume).not.toHaveBeenCalled()
    expect(config.playAnywhere!.onPlayFromHere).not.toHaveBeenCalled()
    expect(document.activeElement).toBe(trigger)
  } finally {
    await act(async () => app.unmount())
  }
})

it('snapshots on menu pointerdown before focus and exposes a keyboard-focusable explicit selection action', async () => {
  const config = props(); const element = document.createElement('div'); document.body.append(element); const app = createRoot(element)
  const snapshot = vi.fn(() => true); const play = vi.fn(); const close = vi.fn()
  try {
    await act(async () => app.render(<AudioControls {...config} playAnywhere={{
      ...config.playAnywhere!,
      onSnapshotSelection: snapshot,
      onPlayFromSelection: play,
      onSelectionMenuClose: close,
    }} />))
    const trigger = document.querySelector<HTMLButtonElement>('[aria-expanded="false"]')!
    await act(async () => trigger.dispatchEvent(new Event('pointerdown', { bubbles: true })))
    expect(snapshot).toHaveBeenCalledOnce()
    trigger.focus(); await act(async () => trigger.click())
    const action = [...document.querySelectorAll('button')].find(b => b.textContent === 'tts.playAnywhere.selection')!
    expect(document.activeElement?.textContent).toBe('tts.playAnywhere.here')
    action.focus(); expect(document.activeElement).toBe(action)
    await act(async () => action.click()); expect(play).toHaveBeenCalledOnce(); expect(close).toHaveBeenCalledOnce()
    expect(document.activeElement).toBe(trigger)
    await act(async () => app.render(<AudioControls {...config} playAnywhere={{
      ...config.playAnywhere!,
      feedback: 'no-selection',
    }} />))
    expect(document.querySelector('[role="status"]')?.textContent).toBe('tts.playAnywhere.noSelection')
  } finally {
    await act(async () => app.unmount())
  }
})

it('keeps passage commands in the popover, reports missing selection and closes before viewport capture', async () => {
  const config = props(); const element = document.createElement('div'); document.body.append(element); const app = createRoot(element)
  const play = vi.fn(); const here = vi.fn(() => expect(document.querySelector('.audio-chunk-menu')).toBeNull())
  try {
    await act(async () => app.render(<AudioControls {...config} ttsState={{
      ...config.ttsState,
      status: 'playing',
    }}
    playAnywhere={{
      ...config.playAnywhere!,
      onPlayFromHere: here,
      onSnapshotSelection: () => false,
      onPlayFromSelection: play,
    }} />))
    expect(document.querySelector('.audio-passage-actions')).toBeNull()
    const trigger = document.querySelector<HTMLButtonElement>('[aria-expanded="false"]')!
    await act(async () => trigger.click())
    const commands = [...document.querySelectorAll<HTMLButtonElement>('.audio-passage-command')]
    expect(commands.map(b => b.textContent)).toEqual(['tts.playAnywhere.here', 'tts.playAnywhere.selection', 'tts.playAnywhere.beginning'])
    expect(commands.every(b => b.querySelector('svg')?.getAttribute('aria-hidden') === 'true')).toBe(true)
    expect(commands[1].disabled).toBe(true)
    expect(document.getElementById(commands[1].getAttribute('aria-describedby')!)?.textContent).toBe('tts.playAnywhere.selectionHint')
    await act(async () => commands[1].click()); expect(play).not.toHaveBeenCalled()
    await act(async () => commands[0].click()); expect(here).toHaveBeenCalledOnce()
    expect(document.activeElement).toBe(trigger)
  } finally {
    await act(async () => app.unmount())
  }
})

it('opens a loaded audiobook on chunks, separates options, and resets the view on reopening', async () => {
  const config = props()
  const element = document.createElement('div')
  document.body.append(element)
  const app = createRoot(element)
  const jump = vi.fn()
  const chunks = Array.from({ length: 175 }, (_, index) => ({
    index,
    chunkId: 'saved-' + index,
    textPreview: 'Saved passage ' + index,
  }))
  try {
    await act(async () => app.render(<AudioControls {...config} onJumpToChunk={jump}
      ttsState={{
        ...config.ttsState,
        status: 'playing',
        currentChunkIndex: 2,
        chunkSummaries: chunks,
      }} />))
    const trigger = document.querySelector<HTMLButtonElement>('[aria-expanded="false"]')!
    await act(async () => trigger.click())
    expect(document.querySelector('.audio-chunk-list')).not.toBeNull()
    expect(document.querySelector('.audio-passage-actions')).toBeNull()
    expect(document.querySelector('.audio-word-highlight-toggle')).toBeNull()
    const views = document.querySelectorAll<HTMLButtonElement>('.audio-menu-views button')
    expect(document.activeElement).toBe(views[0])
    expect(views[0].getAttribute('aria-pressed')).toBe('true')
    await act(async () => views[1].click())
    expect(document.querySelector('.audio-chunk-list')).toBeNull()
    expect(document.querySelector('.audio-passage-actions')).not.toBeNull()
    expect(document.querySelector('.audio-word-highlight-toggle')).not.toBeNull()
    await act(async () => views[0].click())
    const chunk = document.querySelector<HTMLButtonElement>('.audio-chunk-item')!
    await act(async () => chunk.click())
    expect(jump).toHaveBeenCalledOnce()
    expect(document.activeElement).toBe(trigger)
    await act(async () => trigger.click())
    expect(document.querySelector('.audio-chunk-list')).not.toBeNull()
    expect(document.querySelector('.audio-passage-actions')).toBeNull()
  } finally {
    await act(async () => app.unmount())
  }
})

it.each([
  {
    complete: true,
    setup: false,
    playable: true,
  },
  {
    complete: true,
    setup: true,
    playable: true,
  },
  {
    complete: false,
    setup: true,
    playable: true,
  },
  {
    complete: false,
    setup: true,
    playable: false,
  },
  {
    complete: false,
    setup: false,
    playable: true,
  },
])('keeps Save presentation and generation setup separate from playable audio: %j', async ({ complete, setup, playable }) => {
  const config = props()
  const host = document.createElement('div')
  document.body.append(host)
  const app = createRoot(host)
  const openSetup = vi.fn()
  const save = vi.fn()
  try {
    await act(async () => app.render(<AudioControls {...config}
      audiobookState={{
        ...config.audiobookState,
        complete,
      }}
      canPlayAudiobook={playable} canSaveAudiobook={!setup}
      modelSetupLabel={setup ? 'Download model' : null}
      onOpenAudioSetup={openSetup} onSave={save} />))
    const button = host.querySelector<HTMLButtonElement>('.audio-compact-row > button:last-child')!
    expect(button.classList.contains('audio-save-complete')).toBe(complete && !setup)
    expect(button.classList.contains('audio-setup-required')).toBe(setup)
    expect(button.disabled).toBe(complete && !setup)
    expect(button.title).toBe(setup
      ? 'Download model' + (playable ? ' tts.controls.savedAudioSetupHint' : '')
      : complete ? 'tts.controls.savedForCurrentSetup' : 'tts.controls.save')
    expect(button.getAttribute('aria-label')).toBe(button.title)
    await act(async () => button.click())
    expect(openSetup).toHaveBeenCalledTimes(setup ? 1 : 0)
    expect(save).toHaveBeenCalledTimes(!setup && !complete ? 1 : 0)
    if (playable) {
      await act(async () => host.querySelector<HTMLButtonElement>('.audio-primary-btn')!.click())
      expect(config.playAnywhere!.onPlayFromHere).toHaveBeenCalledOnce()
    }
  } finally {
    await act(async () => app.unmount())
  }
})

it('keeps Save preparation and progress ahead of setup-required presentation', async () => {
  const config = props()
  const host = document.createElement('div')
  document.body.append(host)
  const app = createRoot(host)
  const manage = vi.fn()
  try {
    for (const status of ['checking', 'saving'] as const) {
      await act(async () => app.render(<AudioControls {...config} saveInProgress modelSetupLabel="Download model"
        audiobookState={{
          ...config.audiobookState,
          status,
        }} onManageSave={manage} />))
      const button = host.querySelector<HTMLButtonElement>('.audio-compact-row > button:last-child')!
      expect(button.classList.contains('audio-setup-required')).toBe(false)
      expect(button.classList.contains('audio-save-complete')).toBe(false)
      expect(button.disabled).toBe(status === 'checking')
      expect(Boolean(button.querySelector('.spinner'))).toBe(status === 'saving')
      await act(async () => button.click())
    }
    expect(manage).toHaveBeenCalledOnce()
  } finally {
    await act(async () => app.unmount())
  }
})
