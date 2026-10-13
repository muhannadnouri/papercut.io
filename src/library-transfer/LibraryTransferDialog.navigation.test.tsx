// @vitest-environment jsdom
import { act } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'

const ipc = vi.hoisted(() => ({ invoke: vi.fn(), stop: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: ipc.invoke, isTauri: () => true }))
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => ipc.stop }))
vi.mock('react-i18next', () => ({ useTranslation: () => ({
  t: (key: string) => key,
  i18n: { language: 'en', getFixedT: () => (key: string) => key },
}) }))
vi.mock('../i18n', () => ({ currentAppLocale: () => 'en', default: {} }))
import { LibraryTransferDialog } from './LibraryTransferDialog'

let root: Root
let container: HTMLDivElement
let releaseSend: (status: object) => void
const waiting = { state: 'waiting', address: '127.0.0.1:12345', code: 'ABCD-EFGH-JKLM',
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
  ipc.stop.mockReset()
  ipc.invoke.mockReset().mockImplementation(async (command: string) => {
    if (command === 'document_uploads_list') return [{ id: 'abc123', url: '/uploads/abc123.html', title: 'Harbor Guide', format: 'html', sourceKind: 'html', textStatus: 'ready', bytes: 12, sections: 1, importedAtMs: 1 }]
    if (command === 'library_transfer_send_start') return new Promise(resolve => { releaseSend = resolve })
    if (command === 'library_transfer_send_status') return waiting
    if (command === 'library_transfer_send_cancel') return
    if (command === 'tts_list_saved_audiobooks') return []
    throw new Error(`Unexpected IPC command: ${command}`)
  })
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
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_send_start', { request: { documentIds: ['abc123'], audiobookIds: [] } })
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
