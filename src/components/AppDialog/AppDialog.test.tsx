import { Children, isValidElement, type ReactElement, type ReactNode } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { expect, it, vi } from 'vitest'

vi.mock('react', async (original) => ({
  ...await original<typeof import('react')>(),
  useId: () => 'dialog-title',
  useRef: () => ({ current: null }),
  useEffect: () => {},
  useEffectEvent: (callback: unknown) => callback,
}))
vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))
import { AppDialog } from './AppDialog'

function elements(node: ReactNode): ReactElement<Record<string, unknown>>[] {
  return Children.toArray(node).flatMap(child => isValidElement<Record<string, unknown>>(child)
    ? [child, ...elements((child.props.children ?? null) as ReactNode)] : [])
}

it('keeps Back separate from Close, with labeled non-submit header controls and no empty footer', () => {
  const back = vi.fn(), close = vi.fn()
  const tree = AppDialog({ title: 'Settings', showCloseButton: true, onBack: back, onCancel: close, children: 'Body' })
  const buttons = elements(tree).filter(node => node.type === 'button')
  expect(buttons).toHaveLength(2)
  expect(buttons.every(node => node.props.type === 'button')).toBe(true)
  expect(buttons[0].props['aria-label']).toBe('common.back')
  expect(buttons[0].props.title).toBe('common.back')
  expect(renderToStaticMarkup(buttons[0])).not.toContain('>common.back<')
  ;(buttons[0].props.onClick as () => void)()
  expect(back).toHaveBeenCalledOnce()
  expect(close).not.toHaveBeenCalled()
  expect(buttons[1].props['aria-label']).toBe('common.close')
  expect(buttons[1].props.title).toBe('common.close')
  ;(buttons[1].props.onClick as () => void)()
  expect(close).toHaveBeenCalledOnce()
  const html = renderToStaticMarkup(tree)
  expect(html).toContain('app-dialog-navigable')
  expect(html).toContain('common.back')
  expect(html).not.toContain('app-dialog-actions')
})

it.each([true, false])('applies the busy navigation guard to Back, Close and backdrop (disabled=%s)', navigationDisabled => {
  const back = vi.fn(), close = vi.fn()
  const tree = AppDialog({ title: 'Transfer', showCloseButton: true, onBack: back, onCancel: close, navigationDisabled })
  const buttons = elements(tree).filter(node => node.type === 'button')
  expect(buttons.every(node => node.props.disabled === navigationDisabled)).toBe(true)
  for (const button of buttons) (button.props.onClick as () => void)()
  ;(tree.props.onClick as () => void)()
  expect(back).toHaveBeenCalledTimes(navigationDisabled ? 0 : 1)
  expect(close).toHaveBeenCalledTimes(navigationDisabled ? 0 : 2)
})

it('preserves form submission and footer actions unless a caller opts into header navigation', () => {
  const submit = vi.fn()
  const tree = AppDialog({ title: 'Edit', onCancel: vi.fn(), onSubmit: submit,
    children: <input aria-label="Name" />, actions: <><button type="button">Cancel</button><button type="submit">Save</button></> })
  const form = elements(tree).find(node => node.type === 'form')!
  expect(form.props.onSubmit).toBe(submit)
  const html = renderToStaticMarkup(tree)
  expect(html).toContain('app-dialog-actions')
  expect(html).toContain('>Cancel</button>')
  expect(html).toContain('type="submit">Save</button>')
  expect(html).not.toContain('app-dialog-navigable')
  expect(html).not.toContain('app-dialog-close')
})
