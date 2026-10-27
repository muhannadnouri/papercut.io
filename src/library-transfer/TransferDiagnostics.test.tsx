// @vitest-environment jsdom
import { act } from 'react'
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
import { invoke } from '@tauri-apps/api/core'
import { TransferDiagnostics } from './TransferDiagnostics'
import {
  formatTransferDiagnosticReport,
  type TransferDiagnosticSnapshot,
} from './libraryTransfer'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  isTauri: () => true,
}))
vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))
const initial: TransferDiagnosticSnapshot = {
  appVersion: '1.9.2',
  build: null,
  os: 'linux',
  arch: 'x86_64',
  protocol: 'PCLAN003',
  dropped: 0,
  events: [{
    attempt: 1,
    role: 'receive',
    timestampMs: 1000,
    elapsedMs: 1,
    phase: 'tcp-connect',
    level: 'error',
    code: 'io-error',
    bytes: null,
    totalBytes: null,
    items: null,
    totalItems: null,
    ioKind: 'ConnectionRefused',
    osCode: 111,
    port: 12345,
    addressScope: 'private',
    requiredBytes: null,
    availableBytes: null,
    imported: null,
    skipped: null,
    failed: null,
  }],
}
let root: Root
let container: HTMLDivElement
const writeText = vi.fn()
beforeEach(() => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  vi.useFakeTimers()
  container = document.createElement('div')
  document.body.append(container)
  root = createRoot(container)
  vi.mocked(invoke).mockReset().mockResolvedValue(initial)
  writeText.mockReset().mockResolvedValue(undefined)
  vi.stubGlobal('navigator', { clipboard: { writeText } })
})
afterEach(async () => {
  await act(() => root.unmount())
  container.remove()
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

it('copies only bounded diagnostic facts, never extra request, credential, path, address or item fields', () => {
  const event = {
    ...initial.events[0],
    code: '<script>escaped</script>',
    address: '192.168.1.9',
    key: 'ABCD-EFGH-JKLM',
    item: 'Private title',
    path: '/home/private/library',
    error: 'secret',
  }
  const report = formatTransferDiagnosticReport({
    ...initial,
    events: Array.from({ length: 300 }, () => event),
  })
  expect(JSON.parse(report).events).toHaveLength(200)
  expect(JSON.parse(report).dropped).toBe(100)
  for (const secret of ['192.168', 'ABCD', 'Private title', '/home', 'secret']) {
    expect(report).not.toContain(secret)
  }
  expect(report).toContain('ConnectionRefused')
})

it('recovers native history on open, polls only while active, reports clipboard errors and stops on unmount', async () => {
  await act(() => root.render(<TransferDiagnostics active revision="receiving" />))
  expect(container.querySelector('details')?.open).toBe(false)
  await act(() => container.querySelector('summary')!.click())
  expect(container.textContent).toContain('tcp-connect')
  expect(container.textContent).toContain('ConnectionRefused')
  expect(container.querySelector('ol')?.getAttribute('tabindex')).toBe('0')
  const button = container.querySelector<HTMLButtonElement>('button')!
  expect(button.firstElementChild?.getAttribute('aria-hidden')).toBe('true')
  await act(() => button.click())
  expect(JSON.parse(writeText.mock.calls[0][0]).redacted).toBe(true)
  expect(container.querySelector('[role="status"]')?.textContent).toBe('libraryTransfer.diagnostics.copied')
  // "Copied" shows on the button itself and reverts to "Copy" after two seconds.
  expect(button.textContent).toBe('libraryTransfer.diagnostics.copied')
  expect(button.getAttribute('aria-label')).toBe('libraryTransfer.diagnostics.copy')
  await act(() => vi.advanceTimersByTimeAsync(2000))
  expect(button.textContent).toBe('libraryTransfer.diagnostics.copyShort')
  vi.mocked(invoke).mockResolvedValue({
    ...initial,
    events: [{
      ...initial.events[0],
      code: 'latest-terminal-error',
    }],
  })
  await act(() => button.click())
  expect(writeText.mock.calls.at(-1)![0]).toContain('latest-terminal-error')
  writeText.mockRejectedValueOnce(new Error('denied'))
  await act(() => button.click())
  expect(container.querySelector('[role="status"]')?.textContent).toBe('libraryTransfer.diagnostics.copyFailed')
  vi.mocked(invoke).mockResolvedValue({
    ...initial,
    events: [...initial.events, {
      ...initial.events[0],
      phase: 'pairing',
    }],
  })
  await act(() => vi.advanceTimersByTimeAsync(1000))
  // Newest event first, numbered chronologically; Copy comes before the event list.
  expect(container.querySelector('ol li')?.textContent).toContain('pairing')
  expect(container.querySelector('ol')?.hasAttribute('reversed')).toBe(true)
  expect(button.compareDocumentPosition(container.querySelector('ol')!) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
  await act(() => root.render(<TransferDiagnostics active={false} revision="error" />))
  const count = vi.mocked(invoke).mock.calls.length
  await act(() => vi.advanceTimersByTimeAsync(3000))
  expect(invoke).toHaveBeenCalledTimes(count)
  await act(() => root.unmount())
  await vi.advanceTimersByTimeAsync(3000)
  expect(invoke).toHaveBeenCalledTimes(count)
})

it('keeps a failed diagnostic read explicit and recovers through Retry without starting any transfer', async () => {
  vi.mocked(invoke).mockRejectedValueOnce('secret backend error')
  await act(() => root.render(<TransferDiagnostics active={false} revision="error" />))
  expect(container.querySelector('[role="alert"]')?.textContent).toContain('libraryTransfer.diagnostics.unavailable')
  expect(container.textContent).not.toContain('secret backend error')
  expect(container.querySelector<HTMLButtonElement>('.library-transfer-copy-report')!.disabled).toBe(true)
  await act(() => container.querySelector<HTMLButtonElement>('[role="alert"] button')!.click())
  expect(container.querySelector('[role="alert"]')).toBeNull()
  expect(container.querySelector<HTMLButtonElement>('.library-transfer-copy-report')!.disabled).toBe(false)
  expect(new Set(vi.mocked(invoke).mock.calls.map(([command]) => command))).toEqual(new Set(['library_transfer_diagnostics']))
})

it('does not overlap slow polls or let a late active snapshot replace the terminal history', async () => {
  let resolveOld!: (snapshot: TransferDiagnosticSnapshot) => void
  vi.mocked(invoke).mockImplementationOnce(() => new Promise(resolve => {
    resolveOld = resolve
  }))
  await act(() => root.render(<TransferDiagnostics active revision="receiving" />))
  await act(() => vi.advanceTimersByTimeAsync(3000))
  expect(invoke).toHaveBeenCalledOnce()
  await act(() => root.render(<TransferDiagnostics active={false} revision="error" />))
  expect(container.textContent).toContain('ConnectionRefused')
  await act(() => resolveOld({
    ...initial,
    events: [{
      ...initial.events[0],
      code: 'stale-history',
    }],
  }))
  expect(container.textContent).not.toContain('stale-history')
  expect(container.textContent).toContain('ConnectionRefused')
})
