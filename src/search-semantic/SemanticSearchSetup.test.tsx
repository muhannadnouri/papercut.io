import { renderToStaticMarkup } from 'react-dom/server'
import { expect, it, vi } from 'vitest'
import { SemanticSearchSetup } from './SemanticSearchSetup'
import type { SemanticSearchController } from './useSemanticSearch'

vi.mock('react-i18next', () => ({ useTranslation: () => ({
  t: (key: string, values?: Record<string, unknown>) => `${key}${values ? JSON.stringify(values) : ''}`,
}) }))

function controller(overrides: Partial<SemanticSearchController> = {}): SemanticSearchController {
  return { enabled: false, format: 'float32', busy: false, operation: null, cancelling: false, error: '',
    provider: { compiled: true, activeFormat: null, loadingFormat: null, downloadingFormat: null },
    index: { running: false, cancelRequested: false, indexedDocuments: 1, eligibleDocuments: 2,
      passages: 3, embeddedPassages: 2, reusedPassages: 1, error: null },
    setEnabled: vi.fn(), setFormat: vi.fn(), build: vi.fn(), remove: vi.fn(), cancel: vi.fn(), refresh: vi.fn(),
    ...overrides }
}
function html(semantic: SemanticSearchController, cachePresent = true, cacheBusy = false) {
  return renderToStaticMarkup(<SemanticSearchSetup semantic={semantic} cachePresent={cachePresent} cacheBusy={cacheBusy} />)
}

it('shows off and opt-in wording, coverage, and blocks enabling without a cache or during transfer', () => {
  const ready = html(controller())
  expect(ready).toContain('settings.semantic.off')
  expect(ready).toContain('settings.semantic.description')
  expect(ready).toContain('&quot;ready&quot;:1')
  expect(ready).toContain('role="switch"')
  expect(ready).not.toContain('role="switch" disabled=""')
  expect(html(controller(), false)).toContain('role="switch" disabled=""')
  expect(html(controller(), true, true)).toContain('role="switch" disabled=""')
})

it('labels unavailable opted-in state as lexical fallback with an actionable error and retry', () => {
  const value = html(controller({ enabled: true, error: 'Model cache is damaged' }))
  expect(value).toContain('settings.semantic.fallback')
  expect(value).toContain('role="alert"')
  expect(value).toContain('Model cache is damaged')
  expect(value).toContain('settings.semantic.retry')
})

it('keeps the off switch usable during indexing, shows progress/cancellation and hides old errors after disable', () => {
  const semantic = controller({ enabled: true, busy: true, operation: 'build', cancelling: true })
  const value = html(semantic)
  expect(value).toContain('settings.semantic.progress')
  expect(value).toContain('settings.semantic.cancelling')
  expect(value).not.toContain('role="switch" disabled=""')
  const old = controller()
  old.index!.error = 'Cancelled earlier'
  expect(html(old)).not.toContain('Cancelled earlier')
})
