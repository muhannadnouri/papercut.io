import { expect, it, vi } from 'vitest'

const slots = vi.hoisted(() => ({ cursor: 0, overrides: {} as Record<number, unknown> }))
vi.mock('react', async (original) => ({
  ...await original<typeof import('react')>(),
  useState: (initial: unknown) => {
    const index = slots.cursor++
    return [index in slots.overrides ? slots.overrides[index] : initial, vi.fn()]
  },
  useId: () => 'transfer-help',
  useMemo: (callback: () => unknown) => callback(),
  useCallback: (callback: unknown) => callback,
  useEffect: () => {},
}))
vi.mock('react-i18next', () => ({ useTranslation: () => ({
  t: (key: string) => key,
  i18n: { language: 'en', getFixedT: () => (key: string) => key },
}) }))
vi.mock('../i18n', () => ({ currentAppLocale: () => 'en', default: {} }))

import { LibraryTransferDialog } from './LibraryTransferDialog'

it.each([
  ['idle', null, false],
  ['exporting', null, true],
  ['importing', null, true],
  ['preparingSend', null, true],
  ['receiving', null, true],
  ['idle', 'waiting', true],
  ['idle', 'sending', true],
  ['idle', 'complete', false],
] as const)('guards transfer navigation for %s / %s', (state, sendState, busy) => {
  slots.cursor = 0
  slots.overrides = { 1: { state }, 11: sendState ? { state: sendState } : null }
  const back = vi.fn(), close = vi.fn()
  const dialog = LibraryTransferDialog({ onBack: back, onClose: close, onImported: vi.fn() })
  expect(dialog.props.showCloseButton).toBe(true)
  expect(dialog.props.navigationDisabled).toBe(busy)
  expect(dialog.props.onBack).toBe(back)
  expect(dialog.props.onCancel).toBe(close)
  expect(dialog.props.actions).toBeUndefined()
})
