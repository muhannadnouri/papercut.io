// @vitest-environment jsdom
import {
  act,
  StrictMode,
} from 'react'
import {
  createRoot,
  type Root,
} from 'react-dom/client'
import {
  afterEach,
  beforeEach,
  expect,
  it,
  vi,
} from 'vitest'
import type { LibraryTransferNetworkStatus } from './libraryTransfer'
import { DEFAULT_TTS_MODEL_ID } from '../tts/types'
import en from '../i18n/locales/en.json'

const ipc = vi.hoisted(() => ({
  invoke: vi.fn(),
  stop: vi.fn(),
  listen: vi.fn(),
}))
vi.mock('@tauri-apps/api/core', () => ({
  invoke: ipc.invoke,
  isTauri: () => true,
}))
vi.mock('@tauri-apps/api/event', () => ({ listen: ipc.listen }))
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key === 'libraryTransfer.includeDocumentsDescription'
      ? en.libraryTransfer.includeDocumentsDescription : key,
    i18n: {
      language: 'en',
      getFixedT: () => (key: string) => key,
    },
  }),
}))
vi.mock('../i18n', () => ({
  currentAppLocale: () => 'en',
  default: {},
}))
import { LibraryTransferDialog } from './LibraryTransferDialog'

let root: Root; let container: HTMLDivElement
let snapshot: LibraryTransferNetworkStatus | null
const back = vi.fn(); const close = vi.fn(); const imported = vi.fn()
const waiting: LibraryTransferNetworkStatus = {
  id: 1,
  role: 'receiver',
  mode: 'push',
  state: 'waiting',
  listenerReady: true,
  addresses: ['192.168.1.20:53318'],
  code: '2345-ABCD-GHJK',
  expiresAtMs: 1790000000000,
  cancellable: true,
  canRetry: false,
  documents: 1,
  audiobooks: 0,
  packageBytes: 4096,
  bytesTransferred: 0,
  receiverProgress: null,
  error: null,
  result: null,
}

beforeEach(() => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  vi.stubGlobal('__TAURI_INTERNALS__', {})
  vi.useFakeTimers()
  document.body.innerHTML = '<button>Transfer Library</button><div id="mount"></div>'
  document.querySelector('button')!.focus()
  container = document.querySelector('#mount')!
  root = createRoot(container)
  vi.spyOn(HTMLElement.prototype, 'getClientRects').mockReturnValue([new DOMRect(0, 0, 44, 44)] as unknown as DOMRectList)
  Object.defineProperty(HTMLElement.prototype, 'scrollIntoView', {
    configurable: true,
    value: vi.fn(),
  })
  snapshot = null
  back.mockReset(); close.mockReset(); imported.mockReset().mockResolvedValue(undefined)
  ipc.stop.mockReset(); ipc.listen.mockReset().mockResolvedValue(ipc.stop)
  ipc.invoke.mockReset().mockImplementation(async (command: string, args?: { id?: number;
    request?: { role: string;
      mode: string } }) => {
    if (command === 'library_transfer_interfaces') {
      return [{
        id: '2:wlan0',
        name: 'wlan0',
        index: 2,
        addresses: ['192.168.1.20'],
        automatic: true,
      }]
    }
    if (command === 'library_transfer_diagnostics') {
      return {
        appVersion: '1.9.2',
        build: null,
        os: 'linux',
        arch: 'x86_64',
        protocol: 'PCLAN004',
        events: [],
        dropped: 0,
      }
    }
    if (command === 'document_uploads_list') {
      return [{
        id: 'abc123',
        url: '/uploads/abc123.html',
        title: 'Harbor Guide',
        format: 'html',
        sourceKind: 'html',
        textStatus: 'ready',
        bytes: 12,
        sections: 1,
        importedAtMs: 1,
      }]
    }
    if (command === 'library_transfer_network_start') {
      snapshot = {
        ...waiting,
        ...args?.request,
        role: args?.request?.role as 'sender' | 'receiver',
        mode: args?.request?.mode as 'push' | 'pull',
      }
      const listening = (snapshot.role === 'receiver') === (snapshot.mode === 'push')
      snapshot = {
        ...snapshot,
        state: listening ? 'waiting' : 'connecting',
        listenerReady: listening,
        code: listening ? waiting.code : '',
        addresses: listening ? waiting.addresses : [],
      }
      return snapshot
    }
    if (command === 'library_transfer_network_status') {
      return snapshot
    }
    if (command === 'library_transfer_network_dismiss') {
      if (snapshot?.id === args?.id) {
        snapshot = null
      }
      return
    }
    if (command === 'library_transfer_network_cancel') {
      return true
    }
    if (command === 'library_transfer_network_retry') {
      return {
        ...snapshot,
        canRetry: false,
      }
    }
    if (command === 'tts_list_saved_audiobooks') {
      return []
    }
    throw new Error(`Unexpected IPC command: ${command}`)
  })
})

afterEach(async () => {
  await act(() => root.unmount())
  vi.restoreAllMocks(); vi.useRealTimers(); vi.unstubAllGlobals()
  document.body.replaceChildren()
})
async function mount() {
  await act(() => root.render(<LibraryTransferDialog onBack={back} onClose={close} onImported={imported} />))
}
function button(label: string) {
  return Array.from(container.querySelectorAll('button')).find(b => b.textContent === label)!
}
async function click(label: string) {
  await act(() => button(label).click())
}
async function poll() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(500)
  })
}
async function selectFirstDocument() {
  await act(() => {
    const picker = container.querySelector<HTMLDetailsElement>('.library-transfer-content')!
    picker.open = true
    picker.querySelector<HTMLInputElement>('.library-transfer-picker-list input[type="checkbox"]')!.click()
  })
}
async function openSender() {
  await click('libraryTransfer.modeSend')
  await selectFirstDocument()
  await click('libraryTransfer.continueToConnection')
}
async function enterPeer() {
  const manual = Array.from(container.querySelectorAll('summary')).find(s => s.textContent === 'libraryTransfer.manualAddress')!
  await act(() => {
    (manual.parentElement as HTMLDetailsElement).open = true
  })
  const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!
  for (const [placeholder, value] of [['192.168.1.20:53318', '192.168.1.21:53318'], ['ABCD-EFGH-JKLM', '2345-ABCD-GHJK']]) {
    await act(() => {
      const input = container.querySelector<HTMLInputElement>(`[placeholder="${placeholder}"]`)!
      set.call(input, value); input.dispatchEvent(new Event('input', { bubbles: true }))
    })
  }
}
async function tryNavigation() {
  await act(() => {
    container.querySelector<HTMLButtonElement>('.app-dialog-back')!.click()
    container.querySelector<HTMLButtonElement>('.app-dialog-close')!.click()
    container.querySelector<HTMLElement>('.app-dialog-backdrop')!.click()
    document.dispatchEvent(new KeyboardEvent('keydown', {
      key: 'Escape',
      bubbles: true,
    }))
  })
}

it('starts a default outgoing send only after explicit peer details, with no listener port or hidden direction switch', async () => {
  await mount(); await openSender()
  expect(button('libraryTransfer.startSending').disabled).toBe(true)
  expect(container.querySelector('input[type="number"]')).toBeNull()
  await enterPeer(); await click('libraryTransfer.startSending')
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_network_start', {
    request: {
      role: 'sender',
      mode: 'push',
      interfaceId: '2:wlan0',
      address: '192.168.1.21:53318',
      code: waiting.code,
      selection: {
        documentIds: ['abc123'],
        audiobookIds: [],
      },
    },
  })
  await tryNavigation(); expect(back).not.toHaveBeenCalled(); expect(close).not.toHaveBeenCalled()
})

it('starts with a task choice and keeps content selection ahead of connection setup', async () => {
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation(async (command: string, args: unknown) => {
    const value = await fallback(command, args)
    return command === 'document_uploads_list'
      ? [...value, {
        ...value[0],
        id: 'def456',
        url: '/uploads/def456.html',
        title: 'Other Document',
      }]
      : value
  })
  await mount()
  expect(container.querySelector('.library-transfer-network')).toBeNull()
  expect(button('libraryTransfer.startSending')).toBeUndefined()
  expect(button('libraryTransfer.modeSend').getAttribute('aria-pressed')).toBe('false')
  expect(button('libraryTransfer.modeReceive').getAttribute('aria-pressed')).toBe('false')
  await click('libraryTransfer.modeSend')
  expect(container.textContent).toContain('Harbor Guide')
  expect(container.querySelector('.library-transfer-network')).toBeNull()
  expect(document.activeElement?.textContent).toBe('libraryTransfer.chooseContent')
  const documents = () => [...container.querySelectorAll<HTMLInputElement>('.library-transfer-picker-list input[type="checkbox"]')]
  expect(documents().map(input => input.checked)).toEqual([false, false])
  expect(container.querySelector('.library-transfer-content small')?.textContent)
    .toBe('Choose which uploaded documents to include.')
  expect(button('libraryTransfer.continueToConnection').disabled).toBe(true)
  expect(button('libraryTransfer.export').disabled).toBe(true)
  await click('libraryTransfer.continueToConnection'); await click('libraryTransfer.export')
  expect(ipc.invoke.mock.calls.some(([command]) => /network_start|library_transfer_export/.test(command))).toBe(false)
  await act(() => {
    container.querySelector<HTMLDetailsElement>('.library-transfer-content')!.open = true
  })
  await click('common.selectAll')
  expect(documents().every(input => input.checked)).toBe(true)
  await click('common.deselectAll')
  expect(documents().every(input => !input.checked)).toBe(true)
  await selectFirstDocument()
  await click('libraryTransfer.continueToConnection')
  expect(document.activeElement?.textContent).toBe('libraryTransfer.chooseReceiver')
  expect(button('libraryTransfer.nearby.find')).toBeDefined()
  expect(container.querySelector('.library-transfer-content')).toBeNull()
  expect(container.querySelector('.library-transfer-mode[aria-label="libraryTransfer.connectionMethod"]')).toBeNull()
  expect(ipc.invoke.mock.calls.some(([command]) => /network_start|discovery_start/.test(command))).toBe(false)
  await click('libraryTransfer.changeSelection')
  expect(container.querySelector<HTMLInputElement>('.library-transfer-content input[type="checkbox"]')?.checked).toBe(true)
  expect(container.querySelectorAll<HTMLInputElement>('.library-transfer-content input[type="checkbox"]')[1].checked).toBe(false)
  expect(container.querySelector('.library-transfer-network')).toBeNull()
  await act(() => root.unmount())
  root = createRoot(container); await mount(); await click('libraryTransfer.modeSend')
  expect(documents().map(input => input.checked)).toEqual([false, false])
  expect(button('libraryTransfer.continueToConnection').disabled).toBe(true)
})

it('starts audiobooks unselected and includes their source only after an explicit selection', async () => {
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation((command: string, args: unknown) => command === 'tts_list_saved_audiobooks'
    ? Promise.resolve([{
      id: 'audio1',
      documentUrl: '/uploads/abc123.html',
      title: 'Saved Harbor Guide',
      modelId: DEFAULT_TTS_MODEL_ID,
      voice: 'af_heart',
      speed: 1,
      textPreprocessor: 'none',
      dtype: 'float32',
      savedAt: 1,
      chunks: 1,
    }]) : fallback(command, args))
  await mount(); await click('libraryTransfer.modeSend')
  const pickers = container.querySelectorAll<HTMLDetailsElement>('.library-transfer-content')
  const source = pickers[0].querySelector<HTMLInputElement>('.library-transfer-picker-list input')!
  const audio = pickers[1].querySelector<HTMLInputElement>('.library-transfer-picker-list input')!
  expect(source.checked).toBe(false); expect(audio.checked).toBe(false)
  expect(button('libraryTransfer.continueToConnection').disabled).toBe(true)
  await act(() => {
    pickers[1].open = true; audio.click()
  })
  expect(source.checked).toBe(true); expect(source.disabled).toBe(true)
  expect(button('libraryTransfer.continueToConnection').disabled).toBe(false)
  await act(() => audio.click())
  expect(source.checked).toBe(false); expect(source.disabled).toBe(false)
  expect(button('libraryTransfer.continueToConnection').disabled).toBe(true)
  await act(() => audio.click())
  await click('libraryTransfer.continueToConnection'); await enterPeer(); await click('libraryTransfer.startSending')
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_network_start', {
    request: expect.objectContaining({
      selection: {
        documentIds: ['abc123'],
        audiobookIds: ['audio1'],
      },
    }),
  })
})

it('does not opt documents into sending after retrying a failed document load', async () => {
  const fallback = ipc.invoke.getMockImplementation()!
  let failed = false
  ipc.invoke.mockImplementation((command: string, args: unknown) => {
    if (command === 'document_uploads_list' && !failed) {
      failed = true; return Promise.reject(new Error('List failed'))
    }
    return fallback(command, args)
  })
  await mount(); await click('libraryTransfer.modeSend')
  expect(container.textContent).toContain('libraryTransfer.documentsLoadFailed')
  await click('common.retry')
  expect(container.querySelector<HTMLInputElement>('.library-transfer-picker-list input')?.checked).toBe(false)
  expect(button('libraryTransfer.continueToConnection').disabled).toBe(true)
  expect(button('libraryTransfer.export').disabled).toBe(true)
})

it('explains an empty library before showing address, code or network controls', async () => {
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation((command: string, args: unknown) => command === 'document_uploads_list'
    ? Promise.resolve([]) : fallback(command, args))
  await mount(); await click('libraryTransfer.modeSend')
  expect(container.textContent).toContain('libraryTransfer.emptyLibrary')
  expect(button('libraryTransfer.continueToConnection').disabled).toBe(true)
  expect(container.querySelector('.library-transfer-network')).toBeNull()
  await click('libraryTransfer.modeReceive')
  expect(button('libraryTransfer.showPairingDetails').disabled).toBe(false)
})

it('keeps diagnostic details available after a file-export failure on the content step', async () => {
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation((command: string, args: unknown) => command === 'library_transfer_export'
    ? Promise.reject(new Error('Export failed')) : fallback(command, args))
  await mount(); await click('libraryTransfer.modeSend')
  expect(container.querySelector('.library-transfer-diagnostics')).toBeNull()
  await selectFirstDocument()
  await click('libraryTransfer.export')
  expect(container.querySelector('.library-transfer-network')).toBeNull()
  expect(container.textContent).toContain('Export failed')
  expect(container.querySelector('.library-transfer-diagnostics')).not.toBeNull()
  expect(button('libraryTransfer.diagnostics.copy').disabled).toBe(false)
})

it('explains missing content when restoring a cancelled sender instead of leaving a dangling description', async () => {
  snapshot = {
    ...waiting,
    role: 'sender',
    state: 'cancelled',
    cancellable: false,
    listenerReady: false,
  }
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation((command: string, args: unknown) => command === 'document_uploads_list'
    ? Promise.resolve([]) : fallback(command, args))
  await mount()
  const start = button('libraryTransfer.startSending')
  expect(start.disabled).toBe(true)
  expect(document.getElementById(start.getAttribute('aria-describedby')!)?.textContent).toBe('libraryTransfer.selectAtLeastOne')
  await click('libraryTransfer.changeSelection')
  expect(container.textContent).toContain('libraryTransfer.emptyLibrary')
})

it('keeps manual and alternate setup secondary and discards credentials on method/network changes', async () => {
  await mount(); await openSender()
  const details = (key: string) => Array.from(container.querySelectorAll('details')).find(d => d.querySelector('summary')?.textContent === key)!
  for (const key of ['libraryTransfer.manualAddress', 'libraryTransfer.otherConnectionMethod', 'libraryTransfer.networkSettings']) {
    expect(details(key).open).toBe(false)
  }
  expect(container.querySelector('[placeholder="ABCD-EFGH-JKLM"]')).toBeNull()
  await enterPeer()
  await click('libraryTransfer.showCodeHere')
  expect(container.querySelector('[placeholder="192.168.1.20:53318"]')).toBeNull()
  await click('libraryTransfer.useOtherCode')
  expect(container.querySelector<HTMLInputElement>('[placeholder="192.168.1.20:53318"]')?.value).toBe('')
  await enterPeer(); await click('libraryTransfer.refreshNetworks')
  expect(container.querySelector<HTMLInputElement>('[placeholder="192.168.1.20:53318"]')?.value).toBe('')
  expect(container.querySelector('[placeholder="ABCD-EFGH-JKLM"]')).toBeNull()
  expect(ipc.invoke.mock.calls.some(([command]) => command === 'library_transfer_network_start')).toBe(false)
})

it('shows the explicitly selected unverified peer name and requires its code before sending', async () => {
  const fallback = ipc.invoke.getMockImplementation()!
  const name = '<b>Receiving phone</b>'
  ipc.invoke.mockImplementation((command: string, args: unknown) => {
    if (command === 'library_transfer_discovery_start') {
      return Promise.resolve({
        id: 77,
        state: 'finished',
        peers: [{
          id: 'phone',
          name,
          addresses: ['192.168.1.21:53318'],
        }],
        name: null,
      })
    }
    if (command === 'library_transfer_discovery_select') {
      return Promise.resolve('192.168.1.21:53318')
    }
    if (command === 'library_transfer_discovery_stop') {
      return Promise.resolve()
    }
    return fallback(command, args)
  })
  await mount(); await openSender(); await click('libraryTransfer.nearby.find'); await click(name)
  expect(container.querySelector('.library-transfer-network > [role="status"]')?.textContent).toContain(name)
  expect(container.querySelector('b')).toBeNull()
  expect(button('libraryTransfer.startSending').disabled).toBe(true)
  expect(ipc.invoke.mock.calls.some(([command]) => command === 'library_transfer_network_start')).toBe(false)
  const code = container.querySelector<HTMLInputElement>('[placeholder="ABCD-EFGH-JKLM"]')!
  await act(() => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(code, waiting.code)
    code.dispatchEvent(new Event('input', { bubbles: true }))
  })
  await click('libraryTransfer.startSending')
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_network_start', expect.objectContaining({
    request: expect.objectContaining({
      address: '192.168.1.21:53318',
      code: waiting.code,
      mode: 'push',
    }),
  }))
})

it('listens by default on Receive and shows credentials and instructions for the sender', async () => {
  await mount(); await click('libraryTransfer.modeReceive')
  expect(container.querySelector('.library-transfer-pairing-fields')).toBeNull()
  await click('libraryTransfer.showPairingDetails')
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_network_start', {
    request: {
      role: 'receiver',
      mode: 'push',
      interfaceId: '2:wlan0',
      port: 53318,
    },
  })
  expect(container.textContent).toContain(waiting.code)
  expect(container.textContent).toContain('libraryTransfer.receiverPairingStep')
  await tryNavigation(); expect(close).not.toHaveBeenCalled()
})

it.each(['send', 'receive'] as const)('offers the explicit alternate for %s and clears old peer inputs', async role => {
  await mount(); await openSender(); await enterPeer()
  if (role === 'receive') {
    await click('libraryTransfer.modeReceive')
  }
  await click(role === 'send' ? 'libraryTransfer.showCodeHere' : 'libraryTransfer.useOtherCode')
  if (role === 'receive') {
    await enterPeer()
  } else {
    expect(container.querySelector('.library-transfer-pairing-fields')).toBeNull()
  }
  await click(role === 'send' ? 'libraryTransfer.startSending' : 'libraryTransfer.receive')
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_network_start', {
    request: {
      role: role === 'send' ? 'sender' : 'receiver',
      mode: 'pull',
      interfaceId: '2:wlan0',
      ...(role === 'send' ? {
        port: 53318,
        selection: {
          documentIds: ['abc123'],
          audiobookIds: [],
        },
      } : {
        address: '192.168.1.21:53318',
        code: waiting.code,
      }),
    },
  })
})

it.each(['cancelled', 'complete', 'expired', 'failed', 'outcomeUnknown'] as const)('keeps navigation blocked until native cleanup reports %s', async state => {
  await mount(); await openSender(); await enterPeer(); await click('libraryTransfer.startSending')
  await tryNavigation(); expect(close).not.toHaveBeenCalled()
  await click('libraryTransfer.cancelTransfer')
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_network_cancel', { id: 1 })
  await tryNavigation(); expect(close).not.toHaveBeenCalled()
  snapshot = {
    ...snapshot!,
    state,
    listenerReady: false,
    addresses: [],
    code: '',
    cancellable: false,
  }
  await poll(); await tryNavigation()
  expect(back).toHaveBeenCalledOnce(); expect(close).toHaveBeenCalledTimes(3)
  expect(container.textContent).not.toContain(waiting.code)
  expect(container.textContent).not.toContain(waiting.addresses[0])
  if (state === 'outcomeUnknown') {
    expect(container.textContent).toContain('libraryTransfer.outcomeUnknown')
    expect(button('common.retry')).toBeUndefined()
  }
})

it('does not dismiss when cancellation fails or native import admission refuses it', async () => {
  await mount(); await click('libraryTransfer.modeReceive'); await click('libraryTransfer.showPairingDetails')
  ipc.invoke.mockRejectedValueOnce(new Error('Cancel request failed'))
  await click('libraryTransfer.cancelTransfer')
  expect(container.textContent).toContain('Cancel request failed')
  await tryNavigation(); expect(close).not.toHaveBeenCalled()
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation(async (command: string, args: unknown) => {
    if (command === 'library_transfer_network_cancel') {
      snapshot = {
        ...snapshot!,
        state: 'finishingImport',
        cancellable: false,
        listenerReady: false,
        addresses: [],
        code: '',
      }; return false
    }
    return fallback(command, args)
  })
  await click('libraryTransfer.cancelTransfer')
  expect(container.textContent).toContain('libraryTransfer.finishingImport')
  expect(button('libraryTransfer.cancelTransfer').disabled).toBe(true)
  await tryNavigation(); expect(close).not.toHaveBeenCalled()
})

it('surfaces retained interruptions and retries only the same native connector generation after an explicit click', async () => {
  await mount(); await openSender(); await enterPeer(); await click('libraryTransfer.startSending')
  snapshot = {
    ...snapshot!,
    state: 'waitingResume',
    listenerReady: false,
    addresses: [],
    code: '',
    canRetry: true,
    error: {
      code: 'TRANSFER_CONNECT_FAILED',
      message: 'Connection timed out',
    },
  }
  await poll()
  expect(container.querySelector('[role="alert"]')?.textContent).toContain('Connection timed out')
  expect(container.textContent).toContain('libraryTransfer.waitingToResume')
  await poll()
  expect(ipc.invoke.mock.calls.filter(([c]) => c === 'library_transfer_network_retry')).toHaveLength(0)
  await click('common.retry')
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_network_retry', { id: 1 })
  expect(button('common.retry')).toBeUndefined()
  expect(ipc.invoke.mock.calls.filter(([c]) => c === 'library_transfer_network_start')).toHaveLength(1)
})

it('drops mismatched-generation and late status polls', async () => {
  await mount(); await click('libraryTransfer.modeReceive'); await click('libraryTransfer.showPairingDetails')
  snapshot = {
    ...waiting,
    id: 999,
    state: 'complete',
  }
  await poll(); expect(container.textContent).toContain(waiting.code)
  let release!: (value: unknown) => void
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation((command: string, args: unknown) => command === 'library_transfer_network_status'
    ? new Promise(resolve => {
      release = resolve
    }) : fallback(command, args))
  await poll(); await act(() => root.unmount()); await act(() => release({
    ...waiting,
    state: 'complete',
  }))
  expect(container.textContent).toBe('')
  expect(ipc.stop).toHaveBeenCalledOnce()
})

it('preserves diagnostic access on polling/subscription failures without inventing terminal success', async () => {
  ipc.listen.mockRejectedValueOnce(new Error('private subscription detail'))
  await mount(); await click('libraryTransfer.modeReceive'); await click('libraryTransfer.showPairingDetails')
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation(async (command: string, args: unknown) => command === 'library_transfer_network_status' ? Promise.reject('poll failure') : fallback(command, args))
  await poll()
  expect(container.textContent).toContain('libraryTransfer.statusUnavailable')
  expect(container.textContent).toContain('libraryTransfer.diagnostics.progressUnavailable')
  expect(container.textContent).not.toContain('private subscription detail')
  expect(container.querySelector('.library-transfer-diagnostics')).not.toBeNull()
  await tryNavigation(); expect(close).not.toHaveBeenCalled()
})

it('validates a listener override and leaves file export available when no interface is selected', async () => {
  await mount(); await openSender(); await click('libraryTransfer.showCodeHere')
  const input = container.querySelector<HTMLInputElement>('input[type="number"]')!
  const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!
  for (const port of ['0', '1023', '65536', '53318.5']) {
    await act(() => {
      set.call(input, port); input.dispatchEvent(new Event('input', { bubbles: true }))
    })
    expect(button('libraryTransfer.startSending').disabled).toBe(true)
  }
  await act(() => {
    set.call(input, '54444'); input.dispatchEvent(new Event('input', { bubbles: true }))
  })
  await click('libraryTransfer.startSending')
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_network_start', expect.objectContaining({ request: expect.objectContaining({ port: 54444 }) }))
})

it('requires an explicit interface when several are eligible and still permits saving a transfer file', async () => {
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation(async (command: string, args: unknown) => command === 'library_transfer_interfaces'
    ? [{
      id: '2:wlan0',
      name: 'wlan0',
      addresses: ['192.168.1.20'],
      automatic: true,
    }, {
      id: '3:eth0',
      name: 'eth0',
      addresses: ['10.0.0.20'],
      automatic: true,
    }]
    : fallback(command, args))
  await mount(); await click('libraryTransfer.modeSend')
  await selectFirstDocument()
  expect(button('libraryTransfer.export').disabled).toBe(false)
  await click('libraryTransfer.continueToConnection'); await enterPeer()
  expect(button('libraryTransfer.startSending').disabled).toBe(true)
  const settings = Array.from(container.querySelectorAll('details')).find(d => d.querySelector('summary')?.textContent === 'libraryTransfer.networkSettings')!
  expect(settings.open).toBe(true)
})

it('resolves native ownership before allowing role changes, new operations or navigation', async () => {
  let release!: (value: unknown) => void
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation((command: string, args: unknown) => command === 'library_transfer_network_status'
    ? new Promise(resolve => {
      release = resolve
    }) : fallback(command, args))
  await mount()
  expect(button('libraryTransfer.startSending')).toBeUndefined()
  expect(button('libraryTransfer.export')).toBeUndefined()
  for (const key of ['libraryTransfer.modeReceive', 'libraryTransfer.modeSend']) {
    expect(button(key).disabled).toBe(true)
    await click(key)
  }
  await tryNavigation(); expect(close).not.toHaveBeenCalled()
  expect(ipc.invoke.mock.calls.some(([command]) => command === 'library_transfer_network_start')).toBe(false)
  snapshot = {
    ...waiting,
    role: 'sender',
    mode: 'pull',
  }
  ipc.invoke.mockImplementation(fallback)
  await act(() => release(snapshot))
  expect(button('libraryTransfer.modeSend').getAttribute('aria-pressed')).toBe('true')
  expect(container.textContent).toContain(waiting.code)
  expect(button('libraryTransfer.cancelTransfer').disabled).toBe(false)
  await tryNavigation(); expect(close).not.toHaveBeenCalled()
  await click('libraryTransfer.cancelTransfer')
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_network_cancel', { id: 1 })
  snapshot = {
    ...snapshot!,
    state: 'cancelled',
    cancellable: false,
    listenerReady: false,
  }
  await poll(); await tryNavigation(); expect(close).toHaveBeenCalled()
})

it('keeps new work disabled after an ownership lookup fails, with visible Retry and Close', async () => {
  let failLookup = true
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation((command: string, args: unknown) => command === 'library_transfer_network_status' && failLookup
    ? Promise.reject(new Error('private status lookup failure')) : fallback(command, args))
  await mount()
  expect(container.querySelector('[role="alert"]')?.textContent).toContain('libraryTransfer.statusUnavailable')
  expect(container.textContent).not.toContain('private status lookup failure')
  expect(button('libraryTransfer.modeReceive').disabled).toBe(true)
  expect(button('libraryTransfer.startSending')).toBeUndefined()
  expect(button('libraryTransfer.export')).toBeUndefined()
  expect(container.querySelector<HTMLButtonElement>('.app-dialog-close')!.disabled).toBe(false)
  failLookup = false
  snapshot = { ...waiting }
  await click('common.retry')
  expect(button('libraryTransfer.modeReceive').getAttribute('aria-pressed')).toBe('true')
  expect(container.textContent).toContain(waiting.code)
  expect(button('libraryTransfer.cancelTransfer').disabled).toBe(false)
})

it('ignores an obsolete ownership reply after effect teardown instead of replacing the current session', async () => {
  const replies: Array<(value: unknown) => void> = []
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation((command: string, args: unknown) => command === 'library_transfer_network_status'
    ? new Promise(resolve => {
      replies.push(resolve)
    }) : fallback(command, args))
  await act(() => root.render(<StrictMode><LibraryTransferDialog onBack={back} onClose={close} onImported={imported} /></StrictMode>))
  expect(replies).toHaveLength(2)
  ipc.invoke.mockImplementation(fallback)
  snapshot = {
    ...waiting,
    id: 2,
    code: '3456-BCDE-HJKL',
  }
  await act(() => replies[1](snapshot))
  await act(() => replies[0]({
    ...waiting,
    role: 'sender',
    mode: 'pull',
  }))
  expect(container.textContent).toContain('3456-BCDE-HJKL')
  expect(container.textContent).not.toContain(waiting.code)
  expect(button('libraryTransfer.modeReceive').getAttribute('aria-pressed')).toBe('true')
})

it.each(['failed', 'expired', 'outcomeUnknown'] as const)('acknowledges %s before Change Selection, so Close and reopen stay on setup', async state => {
  snapshot = {
    ...waiting,
    state,
    listenerReady: false,
    cancellable: false,
    error: {
      code: 'FAILED',
      message: 'Disconnected',
    },
  }
  await mount(); await click('libraryTransfer.changeSelection')
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_network_dismiss', { id: 1 })
  expect(snapshot).toBeNull()
  await tryNavigation(); expect(close).toHaveBeenCalled()
  await act(() => root.unmount())
  root = createRoot(container); await mount()
  expect(button('libraryTransfer.startSending')).toBeUndefined()
  expect(button('libraryTransfer.modeSend')).toBeDefined()
  expect(button('libraryTransfer.changeSelection')).toBeUndefined()
  expect(container.textContent).not.toContain('Disconnected')
})

it('retains terminal state and exposes dismissal errors until Change Selection succeeds', async () => {
  snapshot = {
    ...waiting,
    state: 'failed',
    listenerReady: false,
    cancellable: false,
    error: {
      code: 'FAILED',
      message: 'Disconnected',
    },
  }
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation((command: string, args: unknown) => command === 'library_transfer_network_dismiss'
    ? Promise.reject(new Error('Could not dismiss transfer')) : fallback(command, args))
  await mount(); await click('libraryTransfer.changeSelection')
  expect(container.textContent).toContain('Disconnected')
  expect(container.textContent).toContain('Could not dismiss transfer')
  expect(snapshot?.id).toBe(1)
  expect(button('libraryTransfer.changeSelection').disabled).toBe(false)
  // Role changes use the same acknowledgement boundary.
  await click('libraryTransfer.modeSend')
  expect(button('libraryTransfer.modeReceive').getAttribute('aria-pressed')).toBe('true')
  expect(container.textContent).toContain('Disconnected')
  ipc.invoke.mockImplementation(fallback)
  await click('libraryTransfer.changeSelection')
  expect(snapshot).toBeNull()
  expect(container.textContent).not.toContain('Could not dismiss transfer')
})

it('locks controls during dismissal and cannot dismiss a newer native generation with the old ID', async () => {
  snapshot = {
    ...waiting,
    state: 'failed',
    listenerReady: false,
    cancellable: false,
    error: {
      code: 'FAILED',
      message: 'Disconnected',
    },
  }
  let release!: () => void
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation((command: string, args: unknown) => command === 'library_transfer_network_dismiss'
    ? new Promise(resolve => {
      release = () => {
        void Promise.resolve(fallback(command, args)).then(resolve)
      }
    }) : fallback(command, args))
  await mount(); await click('libraryTransfer.changeSelection')
  expect(container.textContent).toContain('Disconnected')
  expect(button('libraryTransfer.changeSelection').disabled).toBe(true)
  expect(button('libraryTransfer.modeSend').disabled).toBe(true)
  await tryNavigation(); expect(close).not.toHaveBeenCalled()
  snapshot = {
    ...waiting,
    id: 2,
    code: '3456-BCDE-HJKL',
  }
  await act(() => release())
  expect(snapshot?.id).toBe(2)
  ipc.invoke.mockImplementation(fallback)
  await tryNavigation(); expect(close).toHaveBeenCalled()
  await act(() => root.unmount())
  root = createRoot(container); await mount()
  expect(container.textContent).toContain('3456-BCDE-HJKL')
  expect(ipc.invoke).not.toHaveBeenCalledWith('library_transfer_network_dismiss', { id: 2 })
})

it('waits for the native generation before enabling cancellation during preparation', async () => {
  let release!: (value: unknown) => void
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation((command: string, args: unknown) => command === 'library_transfer_network_start'
    ? new Promise(resolve => {
      release = resolve
    }) : fallback(command, args))
  await mount(); await openSender(); await enterPeer(); await click('libraryTransfer.startSending')
  expect(button('libraryTransfer.cancelTransfer')).toBeUndefined()
  expect(button('libraryTransfer.changeSelection').disabled).toBe(true)
  await tryNavigation(); expect(close).not.toHaveBeenCalled()
  await act(() => release({
    ...waiting,
    role: 'sender',
    state: 'preparing',
    listenerReady: false,
    addresses: [],
    code: '',
  }))
  expect(button('libraryTransfer.cancelTransfer').disabled).toBe(false)
})

it('reattaches native receiving work and applies a partial import result exactly once', async () => {
  snapshot = {
    ...waiting,
    id: 7,
    state: 'finishingImport',
    listenerReady: false,
    addresses: [],
    code: '',
    cancellable: false,
  }
  await mount()
  expect(button('libraryTransfer.modeReceive').getAttribute('aria-pressed')).toBe('true')
  await tryNavigation(); expect(close).not.toHaveBeenCalled()
  snapshot = {
    ...snapshot,
    state: 'complete',
    result: {
      selected: 2,
      imported: 1,
      skipped: 0,
      failed: 1,
      foldersCreated: 0,
      audiobooksSelected: 0,
      audiobooksImported: 0,
      audiobooksSkipped: 0,
      audiobooksFailed: 0,
      importedAudiobooks: [],
      failures: [{
        item: '<b>Unreadable book</b>',
        error: 'Invalid package item',
      }],
    },
  }
  await poll(); await poll()
  expect(imported).toHaveBeenCalledOnce()
  expect(container.textContent).toContain('libraryTransfer.completionPartialTitle')
  expect(container.textContent).toContain('<b>Unreadable book</b>')
  expect(container.querySelector('.library-transfer-result b')).toBeNull()
  await tryNavigation(); expect(close).toHaveBeenCalledTimes(3)
})

it('ignores a late Retry reply after cancellation and a new generation', async () => {
  await mount(); await openSender(); await enterPeer(); await click('libraryTransfer.startSending')
  snapshot = {
    ...snapshot!,
    state: 'waitingResume',
    listenerReady: false,
    addresses: [],
    code: '',
    canRetry: true,
  }
  await poll()
  let release!: (status: object) => void
  const previous = { ...snapshot }
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation((command: string, args: unknown) => command === 'library_transfer_network_retry'
    ? new Promise(resolve => {
      release = resolve
    }) : fallback(command, args))
  await click('common.retry'); await click('libraryTransfer.cancelTransfer')
  snapshot = {
    ...snapshot!,
    state: 'cancelled',
    canRetry: false,
  }
  await poll(); await click('libraryTransfer.modeReceive'); await click('libraryTransfer.showPairingDetails')
  snapshot = {
    ...snapshot!,
    id: 2,
  }
  // The reply belongs to the old attempt even if it carries its correct native ID.
  await act(() => release({
    ...previous,
    state: 'connecting',
    canRetry: false,
  }))
  expect(button('libraryTransfer.modeReceive').getAttribute('aria-pressed')).toBe('true')
  expect(container.textContent).toContain('libraryTransfer.receiverPairingStep')
})

it('retains completed native receiving results on reopen, and dismisses only after a user leaves', async () => {
  snapshot = {
    ...waiting,
    state: 'complete',
    listenerReady: false,
    addresses: [],
    code: '',
    cancellable: false,
    result: {
      selected: 0,
      imported: 0,
      skipped: 0,
      failed: 0,
      foldersCreated: 0,
      audiobooksSelected: 0,
      audiobooksImported: 0,
      audiobooksSkipped: 0,
      audiobooksFailed: 0,
      importedAudiobooks: [],
      failures: [],
    },
  }
  await mount()
  expect(container.textContent).toContain('libraryTransfer.completionTitle')
  expect(ipc.invoke.mock.calls.some(([command]) => command === 'library_transfer_network_dismiss')).toBe(false)
  await tryNavigation()
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_network_dismiss', { id: 1 })
})

it('keeps manual sending available when system discovery is unsupported', async () => {
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation((command: string, args: unknown) => {
    if (command === 'library_transfer_discovery_start') {
      return Promise.resolve({
        id: 88,
        state: 'unsupported',
        peers: [],
        name: null,
      })
    }
    if (command === 'library_transfer_discovery_stop') {
      return Promise.resolve()
    }
    return fallback(command, args)
  })
  await mount(); await openSender(); await click('libraryTransfer.nearby.find')
  expect(container.textContent).toContain('libraryTransfer.nearby.unsupported')
  await enterPeer(); expect(button('libraryTransfer.startSending').disabled).toBe(false)
  await click('libraryTransfer.startSending')
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_network_start', expect.objectContaining({
    request: expect.objectContaining({
      role: 'sender',
      mode: 'push',
      address: '192.168.1.21:53318',
    }),
  }))
})
