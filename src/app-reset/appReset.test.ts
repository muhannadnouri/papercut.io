// @vitest-environment jsdom
import {
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from 'vitest'

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  isTauri: vi.fn(() => true),
  isMobile: vi.fn(() => false),
  forgetNativeAudio: vi.fn(async () => {}),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocks.invoke,
  isTauri: mocks.isTauri,
}))
vi.mock('../utils/platform', () => ({
  isMobileUserAgent: mocks.isMobile,
}))
vi.mock('../tts/playback/nativeMobileAudio', () => ({
  forgetNativeAudio: mocks.forgetNativeAudio,
}))

async function loadModule() {
  vi.resetModules()
  return import('./appReset')
}

beforeEach(() => {
  vi.clearAllMocks()
  mocks.isTauri.mockReturnValue(true)
  mocks.isMobile.mockReturnValue(false)
  window.localStorage.clear()
  window.localStorage.setItem('papercut.theme.v1', 'dark')
  window.localStorage.setItem('papercut:reader-bookmark:/doc', '{}')
})

describe('finishPendingAppReset', () => {
  it('leaves storage alone when no erase is pending', async () => {
    mocks.invoke.mockResolvedValue(null)
    const { finishPendingAppReset, getCompletedAppReset } = await loadModule()

    await finishPendingAppReset()

    expect(window.localStorage.length).toBe(2)
    expect(mocks.invoke).toHaveBeenCalledTimes(1)
    expect(getCompletedAppReset()).toBeNull()
  })

  it('clears every key before acknowledging, so a crash repeats the clear', async () => {
    let keysAtAcknowledge = -1
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'app_reset_pending') {
        return { failedPaths: ['/stuck'] }
      }
      keysAtAcknowledge = window.localStorage.length
      return undefined
    })
    const { finishPendingAppReset, getCompletedAppReset } = await loadModule()

    await finishPendingAppReset()

    expect(keysAtAcknowledge).toBe(0)
    expect(mocks.invoke).toHaveBeenLastCalledWith('app_reset_acknowledge')
    expect(getCompletedAppReset()).toEqual({ failedPaths: ['/stuck'] })
  })

  it('never rejects, so a failure cannot block launch', async () => {
    mocks.invoke.mockRejectedValue(new Error('ipc down'))
    vi.spyOn(console, 'error').mockImplementation(() => {})
    const { finishPendingAppReset, getCompletedAppReset } = await loadModule()

    await expect(finishPendingAppReset()).resolves.toBeUndefined()

    expect(window.localStorage.length).toBe(2)
    expect(getCompletedAppReset()).toBeNull()
  })

  it('does nothing outside Tauri', async () => {
    mocks.isTauri.mockReturnValue(false)
    const { finishPendingAppReset } = await loadModule()

    await finishPendingAppReset()

    expect(mocks.invoke).not.toHaveBeenCalled()
    expect(window.localStorage.length).toBe(2)
  })
})

describe('eraseAppData', () => {
  function stubReload() {
    const reload = vi.fn()
    vi.stubGlobal('location', {
      ...window.location,
      reload,
    })
    return reload
  }

  it('returns the blocker without reloading when native refuses', async () => {
    const reload = stubReload()
    mocks.invoke.mockResolvedValue({
      blockedBy: 'audio',
      restarting: false,
    })
    const { eraseAppData } = await loadModule()

    await expect(eraseAppData()).resolves.toBe('audio')

    expect(reload).not.toHaveBeenCalled()
  })

  it('leaves the desktop restart to native', async () => {
    const reload = stubReload()
    mocks.invoke.mockResolvedValue({
      blockedBy: null,
      restarting: true,
    })
    const { eraseAppData } = await loadModule()

    await expect(eraseAppData()).resolves.toBeNull()

    expect(reload).not.toHaveBeenCalled()
    expect(mocks.forgetNativeAudio).not.toHaveBeenCalled()
  })

  it('releases audio, then reloads after a mobile wipe', async () => {
    const reload = stubReload()
    mocks.isMobile.mockReturnValue(true)
    mocks.invoke.mockResolvedValue({
      blockedBy: null,
      restarting: false,
    })
    const { eraseAppData } = await loadModule()

    await eraseAppData()

    expect(mocks.forgetNativeAudio.mock.invocationCallOrder[0])
      .toBeLessThan(mocks.invoke.mock.invocationCallOrder[0])
    expect(reload).toHaveBeenCalledOnce()
  })

  it('still erases when audio cannot be released', async () => {
    stubReload()
    mocks.isMobile.mockReturnValue(true)
    mocks.forgetNativeAudio.mockRejectedValueOnce(new Error('no player'))
    mocks.invoke.mockResolvedValue({
      blockedBy: null,
      restarting: false,
    })
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    const { eraseAppData } = await loadModule()

    await expect(eraseAppData()).resolves.toBeNull()

    expect(mocks.invoke).toHaveBeenCalledWith('app_reset_request')
  })
})
