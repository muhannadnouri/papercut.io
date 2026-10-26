import {
  Children,
  isValidElement,
} from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import {
  expect,
  it,
  vi,
} from 'vitest'
import { SearchActivity } from './SearchActivity'
import type { SemanticSearchController } from './useSemanticSearch'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, values?: { ready: number;
      total: number }) => values ? `${key}: ${values.ready}/${values.total}` : key,
  }),
}))

it('keeps a quiet live region mounted and distinguishes work phases without replacing Settings', () => {
  const semantic = {
    busy: false,
    operation: null,
    activityNotice: null,
    provider: {
      compiled: true,
      activeFormat: null,
      downloadingFormat: null,
      loadingFormat: null,
    },
  } as SemanticSearchController
  const render = () => renderToStaticMarkup(<SearchActivity semantic={semantic} onManage={() => {}} />)
  expect(render()).not.toContain('<button')
  expect(render()).toContain('role="status"')
  semantic.busy = true
  semantic.provider!.downloadingFormat = 'float32'
  expect(render()).toContain('settings.semantic.activity.downloading')
  expect(render()).toContain('class="spinner" aria-hidden="true"')
  expect(render()).toContain('aria-haspopup="dialog"')
  semantic.provider!.downloadingFormat = null
  semantic.operation = 'activate'
  expect(render()).toContain('settings.semantic.activity.loading')
  semantic.operation = 'build'
  expect(render()).toContain('settings.semantic.activity.indexing')
  expect(render()).not.toContain('search-activity-count') // Do not show pre-reservation coverage as live work.
  semantic.operation = 'disable'
  expect(render()).toContain('settings.semantic.activity.disabling')
})

it('shows document coverage without announcing every tick or implying time remaining', () => {
  const semantic = {
    busy: true,
    operation: 'build',
    index: {
      running: true,
      indexedDocuments: 1,
      eligibleDocuments: 5,
    },
  } as SemanticSearchController
  const render = () => renderToStaticMarkup(<SearchActivity semantic={semantic} onManage={() => {}} />)
  expect(render()).toContain('1 / 5')
  expect(render()).toContain('settings.semantic.documentsReady: 1/5')
  expect(render()).toMatch(/role="status"[^>]*>settings.semantic.activity.indexing<\/span>/)
  semantic.index!.indexedDocuments = 2
  expect(render()).toContain('2 / 5')
  expect(render()).toMatch(/role="status"[^>]*>settings.semantic.activity.indexing<\/span>/)
  expect(render()).not.toContain('progressbar')
  semantic.cancelling = true
  expect(render()).toContain('settings.semantic.activity.cancelling')
  semantic.index!.eligibleDocuments = 0
  expect(render()).not.toContain('search-activity-count')
})

it('retains actionable outcomes without a busy spinner and opens the existing management view', () => {
  const semantic = {
    busy: false,
    activityNotice: 'failed',
    dismissActivity: vi.fn(),
  } as unknown as SemanticSearchController
  const manage = vi.fn()
  const render = () => renderToStaticMarkup(<SearchActivity semantic={semantic} onManage={manage} />)
  expect(render()).toContain('settings.semantic.activity.failed')
  expect(render()).toContain('settings.semantic.activity.review')
  expect(render()).not.toContain('class="spinner"')
  const tree = SearchActivity({
    semantic,
    onManage: manage,
  })
  const button = Children.toArray(tree.props.children).find(child => isValidElement(child) && child.type === 'button')
  expect(isValidElement<{ onClick: () => void }>(button)).toBe(true)
  if (isValidElement<{ onClick: () => void }>(button)) {
    button.props.onClick()
  }
  expect(manage).toHaveBeenCalledOnce()
  expect(semantic.dismissActivity).toHaveBeenCalledOnce()
  for (const notice of ['complete', 'cancelled', 'statusFailed'] as const) {
    semantic.activityNotice = notice
    expect(render()).toContain(`settings.semantic.activity.${notice}`)
    expect(render()).not.toContain('class="spinner"')
  }
  semantic.busy = true; semantic.operation = 'build'
  expect(render()).toContain('settings.semantic.activity.indexing')
  expect(render()).not.toContain('settings.semantic.activity.statusFailed')
})
