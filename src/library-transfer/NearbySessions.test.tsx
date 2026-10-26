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
import type { TransferDiscoverySnapshot } from './libraryTransfer'
const ipc = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({
  invoke: ipc.invoke,
  isTauri: () => true,
}))
vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))
import { NearbySessions } from './NearbySessions'
let root: Root; let container: HTMLDivElement; let snapshot: TransferDiscoverySnapshot
const selected = vi.fn()
const peer = {
  id: 'session',
  name: '<b>Unverified session</b>',
  addresses: ['192.168.1.3:53318'],
}
beforeEach(() => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true); vi.useFakeTimers()
  container = document.createElement('div'); document.body.append(container); root = createRoot(container)
  selected.mockReset(); snapshot = {
    id: 1,
    state: 'searching',
    name: null,
    peers: [],
  }
  ipc.invoke.mockReset().mockImplementation(async (command: string) => {
    if (command === 'library_transfer_discovery_start' || command === 'library_transfer_discovery_status') {
      return snapshot
    }
    if (command === 'library_transfer_discovery_select') {
      return peer.addresses[0]
    }
    if (command === 'library_transfer_discovery_stop') {
      return
    }
    throw new Error(`Unexpected command: ${command}`)
  })
})
afterEach(async () => {
  await act(() => root.unmount()); container.remove(); vi.useRealTimers(); vi.unstubAllGlobals()
})
async function mount() {
  await act(() => root.render(<NearbySessions interfaceId="42:wlan0" mode="push" disabled={false} onSelect={selected} />))
}
async function click(text: string) {
  await act(() => Array.from(container.querySelectorAll('button')).find(b => b.textContent === text)!.click())
}
async function poll() {
  await act(() => vi.advanceTimersByTimeAsync(250))
}

it('starts only by request and selects an untrusted plain-text label without connecting or changing mode', async () => {
  await mount(); expect(ipc.invoke).not.toHaveBeenCalled()
  await click('libraryTransfer.nearby.find')
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_discovery_start', {
    request: {
      interfaceId: '42:wlan0',
      mode: 'push',
    },
  })
  snapshot = {
    ...snapshot,
    peers: [peer],
  }; await poll()
  expect(container.textContent).toContain(peer.name); expect(container.querySelector('b')).toBeNull()
  expect(selected).not.toHaveBeenCalled()
  await click(peer.name)
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_discovery_select', {
    id: 1,
    peer: 'session',
  })
  expect(selected).toHaveBeenCalledExactlyOnceWith('192.168.1.3:53318', peer.name)
  expect(ipc.invoke.mock.calls.every(([command]) => command.startsWith('library_transfer_discovery_'))).toBe(true)
})

it('removes lost peers and surfaces a rejected stale selection without using cached endpoints', async () => {
  snapshot = {
    ...snapshot,
    peers: [peer],
  }; await mount(); await click('libraryTransfer.nearby.find')
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation((command: string, args: unknown) => command === 'library_transfer_discovery_select'
    ? Promise.reject(new Error('private endpoint detail')) : fallback(command, args))
  await click(peer.name)
  expect(selected).not.toHaveBeenCalled(); expect(container.querySelector('[role="alert"]')?.textContent).toBe('libraryTransfer.nearby.unavailable')
  expect(container.textContent).not.toContain('private endpoint detail')
  snapshot = {
    ...snapshot,
    peers: [],
  }; await poll()
  expect(container.textContent).not.toContain(peer.name)
})

it('cannot revive an old peer list with a poll that completes after Refresh', async () => {
  await mount(); await click('libraryTransfer.nearby.find')
  let release!: (value: unknown) => void
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation((command: string, args: unknown) => command === 'library_transfer_discovery_status'
    ? new Promise(resolve => {
      release = resolve
    }) : fallback(command, args))
  await poll()
  snapshot = {
    ...snapshot,
    id: 2,
    peers: [{
      ...peer,
      id: 'new',
      name: 'New Session',
    }],
  }
  await click('libraryTransfer.nearby.refresh')
  await act(() => release({
    ...snapshot,
    id: 1,
    peers: [peer],
  }))
  expect(container.textContent).toContain('New Session'); expect(container.textContent).not.toContain(peer.name)
  await click('New Session')
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_discovery_select', {
    id: 2,
    peer: 'new',
  })
})

it('disposes a native browser even when its start response arrives after the dialog closes', async () => {
  let release!: (value: unknown) => void
  const fallback = ipc.invoke.getMockImplementation()!
  ipc.invoke.mockImplementation((command: string, args: unknown) => command === 'library_transfer_discovery_start'
    ? new Promise(resolve => {
      release = resolve
    }) : fallback(command, args))
  await mount(); await click('libraryTransfer.nearby.find'); await act(() => root.unmount())
  await act(() => release(snapshot))
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_discovery_stop', { id: 1 })
  expect(selected).not.toHaveBeenCalled()
})

it.each(['denied', 'unsupported', 'unavailable', 'limited'] as const)('shows %s without treating it as transfer readiness or selecting a peer', async state => {
  snapshot = {
    ...snapshot,
    state,
    peers: [],
  }; await mount(); await click('libraryTransfer.nearby.find')
  expect(container.textContent).toContain(`libraryTransfer.nearby.${state}`)
  expect(selected).not.toHaveBeenCalled()
  await act(() => root.unmount())
  expect(ipc.invoke).toHaveBeenCalledWith('library_transfer_discovery_stop', { id: 1 })
})
