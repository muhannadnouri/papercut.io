// @vitest-environment jsdom
import { act } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'

const ipc = vi.hoisted(() => ({ invoke: vi.fn(), stop: vi.fn(), listen: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: ipc.invoke, isTauri: () => true }))
vi.mock('@tauri-apps/api/event', () => ({ listen: ipc.listen }))
vi.mock('react-i18next', () => ({ useTranslation: () => ({
  t: (key: string) => key,
  i18n: { language: 'en', getFixedT: () => (key: string) => key },
}) }))
vi.mock('../i18n', () => ({ currentAppLocale: () => 'en', default: {} }))
import { LibraryTransferDialog } from './LibraryTransferDialog'

let root: Root
let container: HTMLDivElement
let releaseSend: (status: object) => void
const waiting = { id: 1, listenerReady: true, addresses: ['127.0.0.1:12345'], state: 'waiting', address: '127.0.0.1:12345', code: 'ABCD-EFGH-JKLM',
  documents: 1, audiobooks: 0, packageBytes: 4096, bytesTransferred: 0 }

beforeEach(() => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  vi.stubGlobal('__TAURI_INTERNALS__', {})
  vi.useFakeTimers()
  document.body.innerHTML = '<button>Transfer Library</button><div id="mount"></div>'
  document.querySelector('button')!.focus()
  container = document.querySelector('#mount')!
  root = createRoot(container)
  vi.spyOn(HTMLElement.prototype, 'getClientRects').mockReturnValue([new DOMRect(0, 0, 44, 44)] as unknown as DOMRectList)
  Object.defineProperty(HTMLElement.prototype, 'scrollIntoView', { configurable: true, value: vi.fn() })
  ipc.stop.mockReset()
  ipc.listen.mockReset().mockResolvedValue(ipc.stop)
  ipc.invoke.mockReset().mockImplementation(async (command: string) => {
    if (command === 'library_transfer_interfaces') return [{ id: '2:wlan0', name: 'wlan0', index: 2, addresses: ['192.168.1.20'], automatic: true }]
    if (command === 'library_transfer_diagnostics') return { appVersion: '1.9.2', build: null, os: 'linux', arch: 'x86_64', protocol: 'PCLAN003', events: [], dropped: 0 }
    if (command === 'document_uploads_list') return [{ id: 'abc123', url: '/uploads/abc123.html', title: 'Harbor Guide', format: 'html', sourceKind: 'html', textStatus: 'ready', bytes: 12, sections: 1, importedAtMs: 1 }]
    if (command === 'library_transfer_send_start') return new Promise(resolve => { releaseSend = resolve })
    if (command === 'library_transfer_send_status') return waiting
    if (command === 'library_transfer_send_cancel') return
    if (command === 'tts_list_saved_audiobooks') return []
    throw new Error(`Unexpected IPC command: ${command}`)
  })
})

it('shows the retained sender interruption outside Details while keeping the waiting session and diagnostics available', async () => {
  await act(() => root.render(<LibraryTransferDialog onBack={vi.fn()} onClose={vi.fn()} onImported={vi.fn()} />))
  await clickLabel('libraryTransfer.startSending')
  await act(() => releaseSend({ ...waiting, error: 'Failed to send library package: Connection reset by peer' }))
  expect(container.querySelector('[role="alert"]')?.textContent).toContain('Connection reset by peer')
  expect(container.querySelector('.library-transfer-session')?.getAttribute('aria-live')).toBeNull()
  expect(container.textContent).toContain('libraryTransfer.waitingToResume')
  expect(container.textContent).toContain(waiting.code)
  expect(container.querySelector('.library-transfer-diagnostics')).not.toBeNull()
  expect(container.querySelector<HTMLButtonElement>('.app-dialog-close')!.disabled).toBe(true)
  expect(ipc.invoke.mock.calls.filter(([command]) => command === 'library_transfer_send_start')).toHaveLength(1)
})

it('exposes failed progress subscription without blocking transfer actions or losing diagnostic access', async () => {
  ipc.listen.mockRejectedValueOnce(new Error('Listener failed with private implementation details'))
  await act(() => root.render(<LibraryTransferDialog onBack={vi.fn()} onClose={vi.fn()} onImported={vi.fn()} />))
  expect(container.querySelector('[role="alert"]')?.textContent).toBe('libraryTransfer.diagnostics.progressUnavailable')
  expect(container.textContent).not.toContain('private implementation details')
  expect(container.querySelector<HTMLButtonElement>('.library-transfer-primary')!.disabled).toBe(false)
  expect(container.querySelector('.library-transfer-diagnostics')).not.toBeNull()
  expect(ipc.invoke.mock.calls.some(([command]) => command === 'library_transfer_send_start')).toBe(false)
})

afterEach(async () => {
  await act(() => root.unmount())
  vi.restoreAllMocks(); vi.useRealTimers(); vi.unstubAllGlobals()
  document.body.replaceChildren()
})

async function clickLabel(label: string) {
  const button = Array.from(container.querySelectorAll('button')).find(button => button.textContent === label)!
  await act(() => button.click())
}
async function tryNavigation() {
  await act(() => {
    container.querySelector<HTMLButtonElement>('.app-dialog-back')!.click()
    container.querySelector<HTMLButtonElement>('.app-dialog-close')!.click()
    container.querySelector<HTMLElement>('.app-dialog-backdrop')!.click()
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
  })
}

it.each(['cancel', 'complete'] as const)('blocks dismissal while preparing/waiting and releases it after %s', async (finish) => {
  const back = vi.fn(), close = vi.fn()
  await act(() => root.render(<LibraryTransferDialog onBack={back} onClose={close} onImported={vi.fn()} />))
  await act(async () => { await vi.waitFor(() => expect(container.querySelector<HTMLButtonElement>('.library-transfer-primary')!.disabled).toBe(false)) })
  await clickLabel('libraryTransfer.startSending')
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_send_start', { request: { documentIds: ['abc123'], audiobookIds: [] }, connection: { interfaceId: '2:wlan0', port: 53318 } })
  await tryNavigation()
  expect(back).not.toHaveBeenCalled(); expect(close).not.toHaveBeenCalled()
  await act(() => releaseSend(waiting))
  await tryNavigation()
  expect(back).not.toHaveBeenCalled(); expect(close).not.toHaveBeenCalled()
  if (finish === 'cancel') {
    ipc.invoke.mockRejectedValueOnce('Cancel failed')
    await clickLabel('libraryTransfer.changeSelection')
    await tryNavigation()
    expect(back).not.toHaveBeenCalled(); expect(close).not.toHaveBeenCalled()
    await clickLabel('libraryTransfer.changeSelection')
    expect(ipc.invoke.mock.calls.filter(([command]) => command === 'library_transfer_send_cancel')).toHaveLength(2)
    expect(container.querySelector<HTMLButtonElement>('.app-dialog-close')!.disabled).toBe(true)
    ipc.invoke.mockResolvedValueOnce({ ...waiting, state: 'cancelled', listenerReady: false, address: '', code: '' })
    await act(async () => { await vi.advanceTimersByTimeAsync(500) })
  } else {
    ipc.invoke.mockResolvedValueOnce({ ...waiting, state: 'complete', bytesTransferred: 4096 })
    await act(async () => { await vi.advanceTimersByTimeAsync(500) })
  }
  await tryNavigation()
  expect(back).toHaveBeenCalledOnce(); expect(close).toHaveBeenCalledTimes(3)
  await act(() => root.unmount())
  expect(ipc.stop).toHaveBeenCalledOnce()
  const count = ipc.invoke.mock.calls.length
  await vi.advanceTimersByTimeAsync(1000)
  expect(ipc.invoke).toHaveBeenCalledTimes(count)
})

it('requires an explicit interface when multiple networks are eligible and keeps file export available', async () => {
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation(async (command: string, args: unknown) => command === 'library_transfer_interfaces'
    ? [
        { id: '2:wlan0', name: 'wlan0', addresses: ['192.168.1.20'], automatic: true },
        { id: '3:eth0', name: 'eth0', addresses: ['10.0.0.20'], automatic: true },
      ] : fallback(command, args))
  await act(() => root.render(<LibraryTransferDialog onBack={vi.fn()} onClose={vi.fn()} onImported={vi.fn()} />))
  expect(container.querySelector<HTMLButtonElement>('.library-transfer-primary')!.disabled).toBe(true)
  const fileButton = Array.from(container.querySelectorAll('button')).find(b => b.textContent === 'libraryTransfer.export')!
  expect(fileButton.disabled).toBe(false)
  const choose = container.querySelector<HTMLButtonElement>('[aria-label="libraryTransfer.network"]')!
  await act(() => { choose.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true })) })
  await act(() => { choose.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true })) })
  await clickLabel('libraryTransfer.startSending')
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_send_start', expect.objectContaining({ connection: { interfaceId: '2:wlan0', port: 53318 } }))
  await act(() => releaseSend(waiting))
})

it('validates an advanced port before IPC and passes a valid override unchanged', async () => {
  await act(() => root.render(<LibraryTransferDialog onBack={vi.fn()} onClose={vi.fn()} onImported={vi.fn()} />))
  const input = container.querySelector<HTMLInputElement>('input[type="number"]')!
  const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!
  const enter = async (value: string) => act(() => { set.call(input, value); input.dispatchEvent(new Event('input', { bubbles: true })) })
  for (const port of ['0', '1023', '65536', '53318.5']) {
    await enter(port)
    expect(container.querySelector<HTMLButtonElement>('.library-transfer-primary')!.disabled).toBe(true)
  }
  expect(ipc.invoke.mock.calls.some(([command]) => command === 'library_transfer_send_start')).toBe(false)
  await enter('54444')
  await clickLabel('libraryTransfer.startSending')
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_send_start', expect.objectContaining({ connection: { interfaceId: '2:wlan0', port: 54444 } }))
  await act(() => releaseSend(waiting))
})

it('drops late sender polls after teardown and ignores snapshots from a different generation', async () => {
  await act(() => root.render(<LibraryTransferDialog onBack={vi.fn()} onClose={vi.fn()} onImported={vi.fn()} />))
  await clickLabel('libraryTransfer.startSending')
  await act(() => releaseSend(waiting))
  ipc.invoke.mockResolvedValueOnce({ ...waiting, id: 999, state: 'complete' })
  await act(async () => { await vi.advanceTimersByTimeAsync(500) })
  expect(container.textContent).toContain(waiting.code)
  expect(container.textContent).not.toContain('libraryTransfer.completionTitle')
  let releasePoll!: (value: unknown) => void
  ipc.invoke.mockImplementationOnce(() => new Promise(resolve => { releasePoll = resolve }))
  await act(async () => { await vi.advanceTimersByTimeAsync(500) })
  await act(() => root.unmount())
  await act(() => releasePoll({ ...waiting, state: 'complete' }))
  expect(container.textContent).toBe('')
})

it('hides expired credentials and offers a fresh selection instead of a dead endpoint', async () => {
  await act(() => root.render(<LibraryTransferDialog onBack={vi.fn()} onClose={vi.fn()} onImported={vi.fn()} />))
  await clickLabel('libraryTransfer.startSending')
  await act(() => releaseSend({ ...waiting, expiresAtMs: 1790000000000 }))
  expect(container.textContent).toContain('libraryTransfer.expiresAt')
  ipc.invoke.mockResolvedValueOnce({ ...waiting, state: 'failed', reason: 'session-expired', listenerReady: false, code: '', address: '', addresses: [], expiresAtMs: null })
  await act(async () => { await vi.advanceTimersByTimeAsync(500) })
  expect(container.textContent).toContain('libraryTransfer.expired')
  expect(container.textContent).not.toContain(waiting.code)
  expect(container.textContent).not.toContain(waiting.address)
  await clickLabel('libraryTransfer.changeSelection')
  expect(container.querySelector<HTMLButtonElement>('.library-transfer-primary')!.disabled).toBe(false)
})

it.each(['failed', 'complete'] as const)('clears a %s sender when switching modes and shows subsequent Receive errors', async (state) => {
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation(async (command: string, args: unknown) => {
    if (command === 'library_transfer_receive') throw new Error('Could not connect: connection timed out')
    return fallback(command, args)
  })
  await act(() => root.render(<LibraryTransferDialog onBack={vi.fn()} onClose={vi.fn()} onImported={vi.fn()} />))
  await clickLabel('libraryTransfer.startSending')
  await act(() => releaseSend(waiting))
  ipc.invoke.mockResolvedValueOnce({ ...waiting, state, listenerReady: false, address: '', addresses: [], code: '', error: state === 'failed' ? 'Pairing failed' : null })
  await act(async () => { await vi.advanceTimersByTimeAsync(500) })
  await clickLabel('libraryTransfer.modeReceive')
  expect(container.querySelector('[aria-label="libraryTransfer.network"]')).not.toBeNull()
  expect(container.textContent).not.toContain(waiting.code)
  expect(container.textContent).not.toContain('Pairing failed')
  const inputs = container.querySelectorAll<HTMLInputElement>('.library-transfer-pairing-fields input')
  const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!
  await act(() => {
    for (const [i, value] of ['192.168.1.21:53318', '2345-ABCD-GHJK'].entries()) {
      set.call(inputs[i], value); inputs[i].dispatchEvent(new Event('input', { bubbles: true }))
    }
  })
  await clickLabel('libraryTransfer.receive')
  expect(container.querySelector('[role="alert"]')?.textContent).toContain('Could not connect: connection timed out')
  await clickLabel('libraryTransfer.modeSend')
  expect(container.querySelector<HTMLButtonElement>('.library-transfer-primary')!.disabled).toBe(false)
  expect(container.textContent).not.toContain('Could not connect: connection timed out')
  await clickLabel('libraryTransfer.startSending')
  const starts = ipc.invoke.mock.calls.filter(([command]) => command === 'library_transfer_send_start')
  expect(starts).toHaveLength(2)
  expect(starts[1]).toEqual(['library_transfer_send_start', { request: { documentIds: ['abc123'], audiobookIds: [] }, connection: { interfaceId: '2:wlan0', port: 53318 } }])
  await act(() => releaseSend({ ...waiting, id: 2 }))
})

it.each(['session-expired', 'transport-failed'])('recovers when cancellation races %s, without accepting another generation', async (reason) => {
  let snapshot: object = waiting
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation(async (command: string, args: unknown) => command === 'library_transfer_send_status' ? snapshot : fallback(command, args))
  await act(() => root.render(<LibraryTransferDialog onBack={vi.fn()} onClose={vi.fn()} onImported={vi.fn()} />))
  await clickLabel('libraryTransfer.startSending')
  await act(() => releaseSend(waiting))
  await clickLabel('libraryTransfer.changeSelection')
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_send_cancel', { id: 1 })
  const failed = { ...waiting, state: 'failed', reason, listenerReady: false, address: '', addresses: [], code: '', error: 'Send failed' }
  snapshot = { ...failed, id: 999 }
  await act(async () => { await vi.advanceTimersByTimeAsync(500) })
  expect(Array.from(container.querySelectorAll('button')).find(b => b.textContent === 'libraryTransfer.cancelling')!.disabled).toBe(true)
  expect(container.querySelector<HTMLButtonElement>('.app-dialog-close')!.disabled).toBe(true)
  snapshot = failed
  await act(async () => { await vi.advanceTimersByTimeAsync(500) })
  expect(container.textContent).not.toContain('libraryTransfer.cancelling')
  const change = Array.from(container.querySelectorAll('button')).find(b => b.textContent === 'libraryTransfer.changeSelection')!
  expect(change.disabled).toBe(false)
  expect(container.querySelector<HTMLButtonElement>('.app-dialog-close')!.disabled).toBe(false)
  await clickLabel('libraryTransfer.changeSelection')
  expect(container.querySelector<HTMLButtonElement>('.library-transfer-primary')!.disabled).toBe(false)
  expect(ipc.invoke.mock.calls.filter(([command]) => command === 'library_transfer_send_cancel')).toHaveLength(1)
})

it.each(['cancel', 'import', 'cancel-failure', 'status-failure'] as const)('uses native receive status to %s without dismissing active work', async (finish) => {
  let finishReceive!: (value: unknown) => void
  let failReceive!: (error: unknown) => void
  let phase = 'connecting'
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation(async (command: string, args: unknown) => {
    if (command === 'library_transfer_receive') return new Promise((resolve, reject) => { finishReceive = resolve; failReceive = reject })
    if (command === 'library_transfer_receive_status' && finish === 'status-failure') throw new Error('poll failure')
    if (command === 'library_transfer_receive_status') return { id: 7, phase, cancellable: phase !== 'finishingImport' }
    if (command === 'library_transfer_receive_cancel' && finish === 'cancel-failure') throw new Error('Cancel request failed')
    if (command === 'library_transfer_receive_cancel') return true
    return fallback(command, args)
  })
  await act(() => root.render(<LibraryTransferDialog onBack={vi.fn()} onClose={vi.fn()} onImported={vi.fn()} />))
  await clickLabel('libraryTransfer.modeReceive')
  const inputs = container.querySelectorAll<HTMLInputElement>('.library-transfer-pairing-fields input')
  const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!
  await act(() => {
    for (const [i, value] of ['192.168.1.21:41803', '2345-ABCD-GHJK'].entries()) {
      set.call(inputs[i], value); inputs[i].dispatchEvent(new Event('input', { bubbles: true }))
    }
  })
  await clickLabel('libraryTransfer.receive')
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_receive', { request: { address: '192.168.1.21:41803', code: '2345-ABCD-GHJK', interfaceId: '2:wlan0' } })
  await act(async () => { await vi.advanceTimersByTimeAsync(250) })
  if (finish !== 'status-failure') {
    const onProgress = ipc.listen.mock.calls[0][1] as (event: unknown) => void
    await act(() => onProgress({ payload: { operation: 'receive', phase: 'verifying', sessionId: 6, itemsProcessed: 0, itemsTotal: 1 } }))
    expect(container.textContent).not.toContain('libraryTransfer.progressVerifying')
    await act(() => onProgress({ payload: { operation: 'receive', phase: 'verifying', sessionId: 7, itemsProcessed: 0, itemsTotal: 1 } }))
    expect(container.textContent).toContain('libraryTransfer.progressVerifying')
  }
  if (finish === 'status-failure') {
    expect(container.textContent).toContain('libraryTransfer.statusUnavailable')
    expect(Array.from(container.querySelectorAll('button')).find(b => b.textContent === 'libraryTransfer.cancelReceive')!.disabled).toBe(true)
    expect(container.querySelector<HTMLButtonElement>('.app-dialog-close')!.disabled).toBe(true)
    await act(() => failReceive(new Error('receive failed')))
  } else if (finish === 'cancel-failure') {
    await clickLabel('libraryTransfer.cancelReceive')
    await act(async () => { await vi.advanceTimersByTimeAsync(250) })
    expect(container.textContent).toContain('Cancel request failed')
    expect(container.querySelector<HTMLButtonElement>('.app-dialog-close')!.disabled).toBe(true)
    await act(() => failReceive(new Error('receive failed')))
  } else if (finish === 'cancel') {
    await clickLabel('libraryTransfer.cancelReceive')
    expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_receive_cancel', { id: 7 })
    expect(container.querySelector<HTMLButtonElement>('.app-dialog-close')!.disabled).toBe(true)
    await act(() => failReceive({ code: 'TRANSFER_CANCELLED', message: 'cancelled' }))
  } else {
    phase = 'finishingImport'
    await act(async () => { await vi.advanceTimersByTimeAsync(250) })
    expect(container.textContent).toContain('libraryTransfer.finishingImport')
    expect(container.textContent).not.toContain('libraryTransfer.cancelReceive')
    expect(container.querySelector<HTMLButtonElement>('.app-dialog-close')!.disabled).toBe(true)
    await act(() => finishReceive({ selected: 0, imported: 0, skipped: 0, failed: 0, foldersCreated: 0, audiobooksSelected: 0, audiobooksImported: 0, audiobooksSkipped: 0, audiobooksFailed: 0, importedAudiobooks: [], failures: [] }))
  }
  expect(container.querySelector<HTMLButtonElement>('.app-dialog-close')!.disabled).toBe(false)
})

it('waits for a native preparation generation before cancelling and treats cancellation as normal recovery', async () => {
  let failStart!: (error: unknown) => void
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation(async (command: string, args: unknown) => {
    if (command === 'library_transfer_send_start') return new Promise((_, reject) => { failStart = reject })
    if (command === 'library_transfer_send_status') return { ...waiting, state: 'preparing', listenerReady: false, addresses: [], address: '', code: '' }
    return fallback(command, args)
  })
  await act(() => root.render(<LibraryTransferDialog onBack={vi.fn()} onClose={vi.fn()} onImported={vi.fn()} />))
  await clickLabel('libraryTransfer.startSending')
  const stop = () => Array.from(container.querySelectorAll('button')).find(b => b.textContent === 'libraryTransfer.stopSending')!
  expect(stop().disabled).toBe(true)
  await act(async () => { await vi.advanceTimersByTimeAsync(500) })
  expect(stop().disabled).toBe(false)
  expect(container.textContent).not.toContain(waiting.code)
  await clickLabel('libraryTransfer.stopSending')
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_send_cancel', { id: 1 })
  expect(container.querySelector<HTMLButtonElement>('.app-dialog-close')!.disabled).toBe(true)
  await act(() => failStart({ code: 'TRANSFER_CANCELLED', message: 'Library transfer cancelled' }))
  expect(container.querySelector<HTMLButtonElement>('.app-dialog-close')!.disabled).toBe(false)
  expect(container.querySelector('[role="alert"]')).toBeNull()
})
