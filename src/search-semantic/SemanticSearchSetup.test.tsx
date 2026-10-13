import { renderToStaticMarkup } from 'react-dom/server'
import { expect, it, vi } from 'vitest'
import { SemanticSearchSetup } from './SemanticSearchSetup'
import type { SemanticSearchController } from './useSemanticSearch'
import { semanticSearchStatus } from './semanticSearchStatus'

vi.mock('react-i18next', () => ({ useTranslation: () => ({
  t: (key: string, values?: Record<string, unknown>) => `${key}${values ? JSON.stringify(values) : ''}`,
}) }))

function controller(overrides: Partial<SemanticSearchController> = {}): SemanticSearchController {
  return { enabled: false, format: 'float32', busy: false, operation: null, cancelling: false, error: '', statusError: '',
    provider: { compiled: true, activeFormat: null, loadingFormat: null, downloadingFormat: null },
    index: { running: false, cancelRequested: false, indexedDocuments: 1, eligibleDocuments: 2,
      passages: 3, hasStoredIndex: true, embeddedPassages: 2, reusedPassages: 1, error: null },
    setEnabled: vi.fn(), setFormat: vi.fn(), build: vi.fn(), remove: vi.fn(), cancel: vi.fn(), refresh: vi.fn(), activityNotice: null, dismissActivity: vi.fn(),
    ...overrides }
}
function html(semantic: SemanticSearchController, cachePresent = true, cacheBusy = false) {
  return renderToStaticMarkup(<SemanticSearchSetup semantic={semantic} cachePresent={cachePresent} cacheBusy={cacheBusy} />)
}

it('shows off and opt-in wording, coverage, and blocks enabling without a cache or during transfer', () => {
  const ready = html(controller())
  expect(ready).toContain('settings.semantic.summary.off')
  expect(ready).toContain('settings.semantic.controlsTitle')
  expect(ready).toContain('settings.semantic.enabled')
  expect(ready).not.toContain('settings.searchModel.resourceSummary')
  expect(ready).toContain('settings.semantic.savedIndex')
  expect(ready).not.toContain('settings.semantic.documentsIndexed')
  expect(ready).toContain('role="switch"')
  expect(ready).not.toMatch(/role="switch"[^>]*disabled=""/)
  expect(html(controller(), false)).toMatch(/role="switch"[^>]*disabled=""/)
  expect(html(controller(), false)).toMatch(/aria-describedby="[^"]+-status [^"]+-prerequisite"/)
  expect(html(controller(), true, true)).toMatch(/role="switch"[^>]*disabled=""/)
})

it('labels unavailable opted-in state as lexical fallback with an actionable error and retry', () => {
  const value = html(controller({ enabled: true, error: 'Model cache is damaged' }))
  expect(value).toContain('settings.semantic.summary.fallback')
  expect(value).toContain('role="alert"')
  expect(value).toContain('Model cache is damaged')
  expect(value).toContain('settings.semantic.retry')
  const duplicate = controller({ enabled: true, error: 'Index job failed' })
  duplicate.index!.error = 'Index job failed'
  expect(html(duplicate).match(/Index job failed/g)).toHaveLength(1)
  duplicate.index!.error = 'Different index failure'
  expect(html(duplicate)).toContain('Different index failure')
})

it('keeps the off switch usable during indexing and preserves errors after disable', () => {
  const semantic = controller({ enabled: true, busy: true, operation: 'build', cancelling: true })
  const value = html(semantic)
  expect(value).toContain('settings.semantic.progress')
  expect(value).toContain('class="spinner search-index-spinner" aria-hidden="true"')
  expect(value).toContain('class="search-index-phase" role="status" aria-live="polite"')
  expect(value).toContain('settings.semantic.cancelling')
  expect(value).not.toMatch(/role="switch"[^>]*disabled=""/)
  const old = controller()
  old.index!.error = 'Cancelled earlier'
  expect(html(old)).toContain('Cancelled earlier')
})


it('distinguishes an empty library, incomplete and complete indexes without claiming 0/0 readiness', () => {
  const semantic = controller({ enabled: true, provider: { compiled: true, activeFormat: 'float32', loadingFormat: null, downloadingFormat: null } })
  expect(html(semantic)).toContain('settings.semantic.needsIndex')
  semantic.index!.indexedDocuments = 2
  expect(html(semantic)).toContain('settings.semantic.indexReady')
  semantic.index!.eligibleDocuments = 0; semantic.index!.indexedDocuments = 0
  const empty = html(semantic)
  expect(empty).toContain('settings.semantic.emptyLibrary')
  expect(empty).not.toContain('settings.semantic.indexReady')
  expect(empty).not.toContain('settings.semantic.coverage')
  expect(empty).toMatch(/disabled="">settings.semantic.build/)
})

it('shows loading and indexing at the affected step and provides download prerequisites', () => {
  const semantic = controller({ enabled: true, busy: true, operation: 'activate' })
  expect(html(semantic)).toContain('settings.semantic.summary.loading')
  semantic.operation = 'build'; semantic.index!.running = true
  expect(html(semantic)).toContain('settings.semantic.indexing')
  expect(html(controller(), false)).toContain('settings.semantic.downloadFirst')
})

it('shows ready status without a build action, then offers an update when coverage changes', () => {
  const semantic = controller({ enabled: true,
    provider: { compiled: true, activeFormat: 'float32', loadingFormat: null, downloadingFormat: null } })
  semantic.index!.indexedDocuments = 2
  const ready = html(semantic)
  expect(ready).toContain('settings.semantic.summary.on')
  expect(ready).toContain('settings.semantic.documentsIndexed')
  expect(ready).not.toContain('search-index-spinner')
  expect(ready).not.toContain('settings.semantic.build')
  expect(ready).not.toContain('settings.semantic.update')
  expect(ready).not.toContain('search-action-primary')
  expect(ready).not.toContain('settings.semantic.enableTitle')
  expect(ready).not.toContain('search-index-details')
  expect(ready).toContain('class="search-index-passages"')
  semantic.index!.eligibleDocuments = 3
  expect(html(semantic)).toContain('settings.semantic.update')
  expect(html(semantic)).not.toContain('settings.semantic.summary.ready')
  semantic.index!.indexedDocuments = 0; semantic.index!.passages = 0
  semantic.index!.hasStoredIndex = false
  const pending = html(semantic)
  expect(pending).toContain('settings.semantic.build')
  expect(pending).toContain('settings.semantic.documentsIndexed')
  expect(pending).not.toContain('settings.semantic.passages')
  expect(pending).not.toContain('settings.semantic.noStoredIndex')
})

it('does not claim readiness for an inactive encoder or a failed job even with full coverage', () => {
  const semantic = controller({ enabled: true,
    provider: { compiled: true, activeFormat: 'float32', loadingFormat: null, downloadingFormat: null } })
  semantic.index!.indexedDocuments = 2
  semantic.index!.error = 'Incomplete job'
  expect(html(semantic)).not.toContain('settings.semantic.summary.ready')
  expect(html(semantic)).toContain('Incomplete job')
  semantic.index!.error = null; semantic.provider!.activeFormat = 'dynamic-int8'
  expect(html(semantic)).toContain('settings.semantic.summary.fallback')
  expect(html(semantic)).not.toContain('settings.semantic.indexReady')
  semantic.provider!.activeFormat = 'float32'; semantic.operation = 'build'; semantic.busy = true
  expect(html(semantic)).toContain('settings.semantic.indexing')
  expect(html(semantic)).not.toContain('settings.semantic.summary.ready')
})

it('reports observed readiness instead of saved consent and never treats stale or empty coverage as ready', () => {
  const semantic = controller({ enabled: true,
    provider: { compiled: true, activeFormat: 'float32', loadingFormat: null, downloadingFormat: null } })
  expect(semanticSearchStatus(semantic)).toBe('needsIndex')
  semantic.index!.indexedDocuments = 2
  expect(semanticSearchStatus(semantic)).toBe('ready')
  semantic.index!.error = 'Index failed'
  expect(semanticSearchStatus(semantic)).toBe('fallback')
  semantic.index!.error = null
  semantic.operation = 'remove'
  expect(semanticSearchStatus(semantic)).toBe('fallback')
  semantic.operation = 'build'; semantic.index!.running = true
  expect(semanticSearchStatus(semantic)).toBe('indexing')
  semantic.index!.cancelRequested = true
  expect(semanticSearchStatus(semantic)).toBe('cancelling')
  semantic.operation = 'activate'
  expect(semanticSearchStatus(semantic)).toBe('loading')
  semantic.operation = 'disable'
  expect(semanticSearchStatus(semantic)).toBe('disabling')
  semantic.operation = null; semantic.index!.running = false; semantic.index!.cancelRequested = false
  semantic.index!.eligibleDocuments = 0; semantic.index!.indexedDocuments = 0
  expect(semanticSearchStatus(semantic)).toBe('empty')
  semantic.index = undefined
  expect(semanticSearchStatus(semantic)).toBe('checking')
  semantic.provider!.activeFormat = 'dynamic-int8'
  expect(semanticSearchStatus(semantic)).toBe('fallback')
  semantic.enabled = false
  expect(semanticSearchStatus(semantic)).toBe('off')
})


it('hides an empty off index, collapses saved checkpoints, and expands pending work or errors', () => {
  const semantic = controller()
  semantic.index!.hasStoredIndex = false; semantic.index!.passages = 0
  expect(html(semantic)).not.toContain('settings.semantic.libraryTitle')
  semantic.index!.hasStoredIndex = true
  const saved = html(semantic)
  expect(saved).toContain('settings.semantic.savedIndex')
  expect(saved).toContain('<details class="search-model-panel search-library-index">')
  expect(saved).not.toContain('settings.semantic.build')
  expect(saved).not.toContain('settings.semantic.documentsIndexed')
  semantic.index!.running = true
  expect(html(semantic)).toContain('class="search-model-panel search-library-index" open=""')
  expect(html(semantic)).toContain('settings.semantic.stop')
  semantic.index!.running = false; semantic.index!.error = 'Cleanup failed'
  expect(html(semantic)).toContain('class="search-model-panel search-library-index" open=""')
  expect(html(semantic)).toContain('Cleanup failed')
})

it('keeps indexing feedback in one place and hides Build and zero reuse during work', () => {
  const semantic = controller({ enabled: true, busy: true, operation: 'build',
    provider: { compiled: true, activeFormat: 'float32', loadingFormat: null, downloadingFormat: null } })
  semantic.index!.running = true
  semantic.index!.reusedPassages = 0
  const value = html(semantic)
  expect(value).toContain('settings.semantic.summary.on')
  expect(value.match(/settings.semantic.indexing/g)).toHaveLength(1)
  expect(value).not.toContain('settings.semantic.summary.indexing')
  expect(value).not.toContain('settings.semantic.build')
  expect(value).not.toContain('settings.semantic.update')
  expect(value).not.toContain('settings.semantic.indexHint')
  expect(value).not.toContain('settings.semantic.reusedProgress')
  expect(value).toContain('settings.semantic.stop')
  semantic.index!.reusedPassages = 5
  expect(html(semantic)).toContain('settings.semantic.reusedProgress')
})

it('shows an actionable attention state, reserving reuse help for saved work', () => {
  const semantic = controller({ enabled: true,
    provider: { compiled: true, activeFormat: 'float32', loadingFormat: null, downloadingFormat: null } })
  semantic.index!.hasStoredIndex = false
  semantic.index!.passages = 0; semantic.index!.indexedDocuments = 0
  const first = html(semantic)
  expect(first).toContain('search-index-attention')
  expect(first).toContain('settings.semantic.buildHelp')
  expect(first).not.toContain('settings.semantic.indexHint')
  semantic.index!.hasStoredIndex = true
  expect(html(semantic)).not.toContain('settings.semantic.indexHint')
  semantic.index!.passages = 3
  expect(html(semantic)).toContain('settings.semantic.indexHint')
  semantic.enabled = false
  expect(html(semantic)).not.toContain('search-index-attention')
  semantic.enabled = true; semantic.index!.eligibleDocuments = 0
  expect(html(semantic)).not.toContain('search-index-attention')
})

it('provides status recovery without claiming readiness or duplicating action errors', () => {
  const semantic = controller({ enabled: true, statusError: 'Status read failed', error: 'Build failed',
    provider: { compiled: true, activeFormat: 'float32', loadingFormat: null, downloadingFormat: null } })
  semantic.index!.indexedDocuments = 2
  const value = html(semantic)
  expect(value).toContain('settings.semantic.retryStatus')
  expect(value).toContain('settings.semantic.statusUnavailable')
  expect(value).not.toContain('settings.semantic.indexReady')
  expect(value).not.toContain('search-index-attention')
  expect(value).not.toContain('settings.semantic.build')
  expect(value.match(/Status read failed/g)).toHaveLength(1)
  expect(value.match(/Build failed/g)).toHaveLength(1)
  expect(semanticSearchStatus(semantic)).toBe('statusUnavailable')
  semantic.index!.running = true; semantic.busy = true
  expect(html(semantic)).toContain('settings.semantic.stop')
  expect(html(semantic)).not.toMatch(/role="switch"[^>]*disabled=""/)
})
