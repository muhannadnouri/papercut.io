import { renderToStaticMarkup } from 'react-dom/server'
import { expect, it, vi } from 'vitest'
import { SearchActivity } from './SearchActivity'
import type { SemanticSearchController } from './useSemanticSearch'

vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))

it('hides idle activity and distinguishes downloads, activation, indexing and shutdown', () => {
  const semantic = { busy: false, operation: null,
    provider: { compiled: true, activeFormat: null, downloadingFormat: null, loadingFormat: null },
  } as SemanticSearchController
  const render = () => renderToStaticMarkup(<SearchActivity semantic={semantic} onManage={() => {}} />)
  expect(render()).toBe('')
  semantic.busy = true
  semantic.provider!.downloadingFormat = 'float32'
  expect(render()).toContain('settings.semantic.downloading')
  expect(render()).toContain('aria-haspopup="dialog"')
  semantic.provider!.downloadingFormat = null
  semantic.operation = 'activate'
  expect(render()).toContain('settings.semantic.loading')
  semantic.operation = 'build'
  expect(render()).toContain('settings.semantic.indexing')
  semantic.operation = 'disable'
  expect(render()).toContain('settings.semantic.disabling')
})
