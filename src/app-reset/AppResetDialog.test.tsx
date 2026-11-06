// @vitest-environment jsdom
import {
  act,
  type ComponentProps,
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
import { AppResetDialog } from './AppResetDialog'
import type { AppResetSummary } from './appReset'

const mocks = vi.hoisted(() => ({
  getAppResetSummary: vi.fn(),
  eraseAppData: vi.fn(),
}))

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))
vi.mock('../i18n', () => ({
  currentAppLocale: () => 'en',
}))
vi.mock('./appReset', () => mocks)

const SUMMARY: AppResetSummary = {
  documents: 1234,
  audiobooks: 2,
  totalBytes: 5 * 1024 * 1024 * 1024,
  locations: ['/home/me/.local/share/papercut', '/home/me/.cache/papercut'],
  blockedBy: null,
}

let container: HTMLDivElement
let root: Root

async function renderDialog(props: Partial<ComponentProps<typeof AppResetDialog>> = {}) {
  const handlers = {
    onBack: vi.fn(),
    onClose: vi.fn(),
    onExportFirst: vi.fn(),
  }
  await act(async () => {
    root.render(
      <AppResetDialog
        blockedBy={null}
        fallbackFocusRef={{ current: null }}
        {...handlers}
        {...props}
      />,
    )
  })
  return handlers
}

function button(label: string): HTMLButtonElement {
  const match = [...container.querySelectorAll('button')]
    .find((candidate) => candidate.textContent === label)
  if (!match) {
    throw new Error(`No button labelled ${label}`)
  }
  return match
}

const confirmButton = () => button('settings.erase.confirm')

async function acknowledge() {
  const checkbox = container.querySelector<HTMLInputElement>('input[type="checkbox"]')!
  await act(async () => {
    checkbox.click()
  })
}

beforeEach(() => {
  vi.clearAllMocks()
  mocks.getAppResetSummary.mockResolvedValue(SUMMARY)
  mocks.eraseAppData.mockResolvedValue(null)
  vi.spyOn(HTMLElement.prototype, 'getClientRects').mockImplementation(
    () => [new DOMRect(0, 0, 44, 44)] as unknown as DOMRectList,
  )
  ;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true
  container = document.createElement('div')
  document.body.append(container)
  root = createRoot(container)
})

afterEach(async () => {
  await act(async () => root.unmount())
  container.remove()
})

it('shows what will be erased', async () => {
  await renderDialog()

  expect(container.textContent).toContain('1,234')
  expect(container.textContent).toContain('settings.erase.keptBackups')
  expect(container.textContent).toContain('settings.erase.exportNote')
})

it('shows where the data lives, left to right in every locale', async () => {
  await renderDialog()

  const paths = container.querySelector('.app-reset-locations')!
  expect(paths.getAttribute('dir')).toBe('ltr')
  expect([...paths.querySelectorAll('li')].map((item) => item.textContent)).toEqual(SUMMARY.locations)
  expect(container.textContent).toContain('settings.erase.locationNote')
})

it('leaves out the location section on mobile', async () => {
  mocks.getAppResetSummary.mockResolvedValue({
    ...SUMMARY,
    locations: [],
  })
  await renderDialog()

  expect(container.querySelector('.app-reset-locations')).toBeNull()
  expect(container.textContent).not.toContain('settings.erase.locationHeading')
})

it('enables erasing only after the acknowledgement', async () => {
  await renderDialog()
  expect(confirmButton().disabled).toBe(true)

  await acknowledge()
  expect(confirmButton().disabled).toBe(false)

  await act(async () => confirmButton().click())
  expect(mocks.eraseAppData).toHaveBeenCalledOnce()
})

it('never focuses the destructive button first', async () => {
  await renderDialog()
  expect(document.activeElement).not.toBe(confirmButton())
})

it.each(['frontend', 'native'] as const)('explains a job reported by the %s and refuses', async (source) => {
  if (source === 'native') {
    mocks.getAppResetSummary.mockResolvedValue({
      ...SUMMARY,
      blockedBy: 'transfer',
    })
  }
  await renderDialog({ blockedBy: source === 'frontend' ? 'transfer' : null })
  await acknowledge()

  expect(confirmButton().disabled).toBe(true)
  expect(container.querySelector('[role="status"]')?.textContent)
    .toBe('settings.erase.blocked.transfer')
})

it('shows a refusal that arrives after confirming and allows no retry until it clears', async () => {
  mocks.eraseAppData.mockResolvedValue('imports')
  await renderDialog()
  await acknowledge()

  await act(async () => confirmButton().click())

  expect(container.querySelector('[role="status"]')?.textContent)
    .toBe('settings.erase.blocked.imports')
  expect(confirmButton().disabled).toBe(true)
})

it('stays busy and undismissable while the restart is pending', async () => {
  const { onBack, onClose } = await renderDialog()
  await acknowledge()

  await act(async () => confirmButton().click())

  expect(button('settings.erase.erasing').disabled).toBe(true)
  expect(button('settings.erase.keep').disabled).toBe(true)
  await act(async () => {
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
  })
  expect(onClose).not.toHaveBeenCalled()
  expect(onBack).not.toHaveBeenCalled()
})

it('reports a failed erase and lets the user retry', async () => {
  mocks.eraseAppData.mockRejectedValueOnce(new Error('disk'))
  await renderDialog()
  await acknowledge()

  await act(async () => confirmButton().click())

  expect(container.querySelector('[role="alert"]')?.textContent).toBe('settings.erase.failed')
  expect(confirmButton().disabled).toBe(false)
})

it('offers export and keeps the data on cancel', async () => {
  const { onBack, onExportFirst } = await renderDialog()

  await act(async () => button('settings.erase.exportFirst').click())
  await act(async () => button('settings.erase.keep').click())

  expect(onExportFirst).toHaveBeenCalledOnce()
  expect(onBack).toHaveBeenCalledOnce()
  expect(mocks.eraseAppData).not.toHaveBeenCalled()
})
