// @vitest-environment jsdom
import { act, type ComponentProps } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { TranslationPanel } from './TranslationPanel'
import en from '../../i18n/locales/en.json'
import type { TranslatedDocumentInfo, TranslationModelInfo, TranslationModelStatus } from '../api/nativeTranslation'

// Interpolate confirmation payloads so refresh tests can detect a stale document title.
vi.mock('react-i18next', async original => ({
  ...await original<typeof import('react-i18next')>(),
  useTranslation: () => ({
    t: (key: string, options?: { size?: string; title?: string }) => {
      if (key === 'translation.confirmModelRemove.description') return `${key} ${options?.size ?? ''}`
      if (key === 'translation.confirmDelete.description') return `${key} ${options?.title ?? ''}`
      return key
    },
    i18n: { language: 'en' },
  }),
}))
vi.mock('../../i18n', () => ({ default: { t: (key: string) => key }, currentAppLocale: () => 'en' }))
const opus: TranslationModelInfo = {
  id: 'opus-mt-fr-en-ctranslate2', name: 'OPUS French', engine: 'ctranslate2', available: true,
  tier: 'fast', manifestState: 'pinned-file-manifest', sourceLanguages: ['fr'], targetLanguages: ['en'],
  defaultQualityMode: 'balanced', recommendedPlatforms: ['desktop'], licenseNotes: '', sizeNotes: '', notes: '',
}
const french: TranslationModelInfo = { ...opus, id: 'bergamot-fr-en-worker', name: 'Bergamot French (Experimental)', engine: 'bergamot' }
const spanish: TranslationModelInfo = { ...french, id: 'bergamot-es-en-worker', name: 'Bergamot Spanish (Experimental)', sourceLanguages: ['es'] }
const installed: TranslationModelStatus = { modelId: french.id, installed: true, installing: false, sourceUrl: '', sourceLabel: '', archiveBytes: 42, installedBytes: 42, sha256: '', message: '' }
let root: Root
let container: HTMLDivElement
let props: ComponentProps<typeof TranslationPanel>

/** Render the real picker and action handlers; no browser-layout or model-inference claim. */
async function render(overrides: Partial<typeof props> = {}) {
  props = { ...props, ...overrides }
  await act(async () => { root.render(<TranslationPanel {...props} />) })
}
function button(text: string) {
  const found = [...container.querySelectorAll('button')].find(b => b.textContent?.includes(text))
  if (!found) throw new Error(`Missing button ${text}`)
  return found
}
async function click(text: string) { await act(async () => { button(text).click() }) }
function picker() { return container.querySelector('select')! }
async function select(id: string) {
  await act(async () => { picker().value = id; picker().dispatchEvent(new Event('change', { bubbles: true })) })
}
beforeEach(async () => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  container = document.createElement('div'); document.body.append(container); root = createRoot(container)
  props = {
    capabilities: { available: true, backend: 'test', reason: '', platform: 'android', defaultQualityMode: 'balanced', hardwareAcceleration: null, models: [opus, french, spanish] },
    deleteState: null, error: '', loading: false, modelInstallState: { installingModelId: '', progress: null, result: null, message: '' },
    modelRemoveState: { removingModelId: '', result: null, message: '' }, modelStatuses: {},
    selectedDocument: { title: 'Book', url: '/uploads/book.html', format: 'html' },
    startState: { checking: false, cancelling: false, jobId: '', progress: null, result: null, message: '' }, translatedDocuments: [],
    onCancelTranslation: vi.fn(), onDeleteTranslatedDocument: vi.fn(), onOpenTranslatedDocument: vi.fn(),
    onInstallTranslationModel: vi.fn(), onRemoveTranslationModel: vi.fn(), onStartTranslationPreflight: vi.fn(), refresh: vi.fn(),
  }
  await render()
})
afterEach(async () => { await act(async () => root.unmount()); container.remove(); vi.unstubAllGlobals() })

describe('Bergamot catalog workbench', () => {
  it('confirms zero stored bytes for an empty model directory without substituting the download estimate', async () => {
    const model = { ...french, available: false, unavailableReason: 'acceptance-pending' as const, sizeNotes: 'About 42 MB including the runtime' }
    await render({ capabilities: { ...props.capabilities!, available: false, backend: 'bergamot-worker', models: [model] },
      modelStatuses: { [model.id]: { ...installed, installed: false, hasStoredFiles: true, installedBytes: 0, archiveBytes: 42000000 } } })
    await click('translation.models.title')
    const row = container.querySelector('.translation-model-item')!
    expect(row.textContent).toContain('0 MB')
    const remove = row.querySelector<HTMLButtonElement>('.translation-model-remove-btn')!
    expect(remove.disabled).toBe(false)
    await act(async () => remove.click())
    const description = document.querySelector('.app-dialog-header p')!.textContent
    expect(description).toContain('0 MB')
    expect(description).not.toContain(model.sizeNotes)
    expect(props.onRemoveTranslationModel).not.toHaveBeenCalled()
    await act(async () => document.querySelector<HTMLButtonElement>('.app-dialog-danger')!.click())
    expect(props.onRemoveTranslationModel).toHaveBeenCalledExactlyOnceWith(model.id)
    expect(props.onInstallTranslationModel).not.toHaveBeenCalled()
  })

  it.each(['bergamot-fr-en-worker', 'hy-mt2-1.8b-q8'])('removes invalid stored %s files without admitting translation or downloads', async id => {
    const model = { ...french, id, name: id, available: false, unavailableReason: 'acceptance-pending' as const }
    const partial = { ...installed, modelId: id, installed: false, hasStoredFiles: true, installedBytes: 16, modelDir: null }
    await render({ capabilities: { ...props.capabilities!, models: [opus, model] }, modelStatuses: { [id]: partial } })
    await click('translation.models.title')
    const row = [...container.querySelectorAll('.translation-model-item')].find(r => r.textContent?.includes(model.name))!
    expect(row.textContent).toContain('translation.models.filesStored')
    expect(row.querySelector('.translation-model-install-btn')).toBeNull()
    const remove = row.querySelector<HTMLButtonElement>('.translation-model-remove-btn')!
    expect(remove.disabled).toBe(false)
    await act(async () => remove.click())
    await act(async () => document.querySelector<HTMLButtonElement>('.app-dialog-cancel')!.click())
    expect(props.onRemoveTranslationModel).not.toHaveBeenCalled()
    await act(async () => remove.click())
    await act(async () => document.querySelector<HTMLButtonElement>('.app-dialog-danger')!.click())
    expect(props.onRemoveTranslationModel).toHaveBeenCalledExactlyOnceWith(id)
    expect(props.onInstallTranslationModel).not.toHaveBeenCalled()
    expect(props.onStartTranslationPreflight).not.toHaveBeenCalled()
    await render({ modelStatuses: { [id]: { ...partial, hasStoredFiles: false, installedBytes: 0 } } })
    expect(row.querySelector('.translation-model-remove-btn')).toBeNull()
    expect(row.querySelector<HTMLButtonElement>('.translation-model-install-btn')!.disabled).toBe(true)
  })

  it('keeps explicit repair available for an admitted partial pack while protecting in-progress removal', async () => {
    const partial = { ...installed, modelId: opus.id, installed: false, hasStoredFiles: true, installing: true }
    await render({ modelStatuses: { [opus.id]: partial } })
    await click('translation.models.title')
    const row = [...container.querySelectorAll('.translation-model-item')].find(r => r.textContent?.includes(opus.name))!
    expect(row.querySelector<HTMLButtonElement>('.translation-model-remove-btn')!.disabled).toBe(true)
    expect(row.querySelector<HTMLButtonElement>('.translation-model-install-btn')!.disabled).toBe(true)
    expect(button('translation.workbench.run').disabled).toBe(true)
    await render({ modelStatuses: { [opus.id]: { ...partial, installing: false } } })
    expect(row.querySelector<HTMLButtonElement>('.translation-model-remove-btn')!.disabled).toBe(false)
    await act(async () => row.querySelector<HTMLButtonElement>('.translation-model-install-btn')!.click())
    expect(props.onInstallTranslationModel).toHaveBeenCalledExactlyOnceWith(opus.id)
    // The hook retains terminal progress for its success message; it must not lock removal afterward.
    await render({
      modelStatuses: { [opus.id]: { ...partial, installed: true, installing: false } },
      modelInstallState: { ...props.modelInstallState, progress: { modelId: opus.id, status: 'installed', message: '', downloadedBytes: 42, totalBytes: 42, percent: 100 } },
    })
    expect(row.querySelector<HTMLButtonElement>('.translation-model-remove-btn')!.disabled).toBe(false)
  })

  it.each([
    ['translation-unavailable', false, 'translation.status.unavailableReason'],
    ['bergamot-worker', false, 'translation.models.acceptancePending'],
    ['ctranslate2+bergamot-worker', true, null],
  ] as const)('uses the %s build reason independently of pending catalog rows', async (backend, available, expected) => {
    await render({ capabilities: { ...props.capabilities!, backend, available, models: [
      { ...opus, available }, { ...french, available: false, unavailableReason: 'acceptance-pending' },
    ] } })
    const banner = container.querySelector('.translation-status-stack .translation-alert')
    if (expected) {
      expect(banner?.textContent).toContain(expected)
    } else {
      expect(banner).toBeNull()
      expect(picker().value).toBe(opus.id)
    }
  })

  it('gates unqualified jobs/downloads while keeping OPUS and explicit model removal usable', async () => {
    const hy = { ...french, id: 'hy-mt2-1.8b-q8', name: 'HY-MT2', engine: 'llama.cpp', available: false, unavailableReason: 'acceptance-pending' as const }
    // Simulate an older selection when a fresh capability response closes admission.
    await select(french.id)
    await render({
      capabilities: { ...props.capabilities!, models: [opus, { ...french, available: false, unavailableReason: 'acceptance-pending' }, hy] },
      modelStatuses: { [opus.id]: { ...installed, modelId: opus.id }, [french.id]: installed },
    })
    expect(picker().value).toBe(opus.id)
    expect(picker().options[1].disabled).toBe(true)
    expect(picker().options[2].disabled).toBe(true)
    await click('translation.workbench.run')
    expect(props.onStartTranslationPreflight).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ modelId: opus.id }))
    await click('translation.models.title')
    const rows = [...container.querySelectorAll('.translation-model-item')]
    const hyRow = rows.find(row => row.textContent?.includes(hy.name))!
    expect(hyRow.textContent).toContain('translation.models.acceptancePending')
    expect(hyRow.textContent).not.toContain('translation.models.engineUnavailable')
    await act(async () => hyRow.querySelector<HTMLButtonElement>('button')!.click())
    expect(hyRow.querySelector<HTMLButtonElement>('button')!.disabled).toBe(true)
    expect(props.onInstallTranslationModel).not.toHaveBeenCalled()
    const workerRow = rows.find(row => row.textContent?.includes(french.name))!
    expect(workerRow.querySelector<HTMLButtonElement>('button')!.disabled).toBe(false)
    await act(async () => workerRow.querySelector<HTMLButtonElement>('button')!.click())
    await act(async () => document.querySelector<HTMLButtonElement>('.app-dialog-danger')!.click())
    expect(props.onRemoveTranslationModel).toHaveBeenCalledExactlyOnceWith(french.id)
  })

  it('explains acceptance-pending worker-only builds and still opens saved translations', async () => {
    await render({
      capabilities: { ...props.capabilities!, available: false, models: [{ ...french, available: false, unavailableReason: 'acceptance-pending' }] },
      translatedDocuments: [{ id: 'saved', title: 'Saved', documentUrl: '/uploads/translated.html', sourceDocumentUrl: '/uploads/book.html', sourceLanguage: 'fr', targetLanguage: 'en', modelId: french.id, status: 'stored', createdAtMs: 1, updatedAtMs: 1 }],
    })
    expect(container.querySelector('select')).toBeNull()
    expect(container.textContent).toContain('translation.models.acceptancePending')
    expect(container.textContent).not.toContain('translation.status.unavailableReason')
    await click('common.view')
    expect(props.onOpenTranslatedDocument).toHaveBeenCalledExactlyOnceWith('/uploads/translated.html')
  })

  it.each(['runtime-unavailable', 'acceptance-pending'] as const)('ignores old %s setup and permits normal OPUS jobs', async reason => {
    await render({ capabilities: { ...props.capabilities!, formatting: { available: false, reason, modelId: '', archiveBytes: 0 } }, modelStatuses: { [opus.id]: { ...installed, modelId: opus.id } } })
    expect(container.textContent).toContain('translation.models.ctranslate2BoundaryNote')
    const strict = container.querySelector('input[type=checkbox]') as HTMLInputElement
    expect(strict).toBeNull()
    expect(button('translation.workbench.run').disabled).toBe(false)
    await click('translation.workbench.run')
    expect(props.onStartTranslationPreflight).toHaveBeenCalledExactlyOnceWith({ documentUrl: '/uploads/book.html', modelId: opus.id, sourceLanguage: 'fr', targetLanguage: 'en', qualityMode: 'balanced', useHardwareAcceleration: false })
  })

  it('explains whole-sentence translation and review fallback only for CTranslate2 without enabling strict formatting', async () => {
    expect(en.translation.models.ctranslate2BoundaryNote).toContain('Translates whole sentences.')
    expect(en.translation.models.ctranslate2BoundaryNote).toContain('formatted original is kept beside the translation for review')
    expect(container.textContent).toContain('translation.models.ctranslate2BoundaryNote')
    const strict = container.querySelector('input[type=checkbox]') as HTMLInputElement
    expect(strict).toBeNull()
    await select(french.id)
    expect(container.textContent).not.toContain('translation.models.ctranslate2BoundaryNote')
  })

  it('preserves the native default and requires explicit installation before running', async () => {
    expect(picker().value).toBe(opus.id)
    await select(french.id)
    expect(button('translation.workbench.run').disabled).toBe(true)
    expect(container.textContent).toContain('translation.errors.modelNotInstalled')
    expect(props.onInstallTranslationModel).not.toHaveBeenCalled()
    await click('translation.models.title')
    const row = [...container.querySelectorAll('.translation-model-item')].find(r => r.textContent?.includes(french.name))!
    await act(async () => (row.querySelector('button') as HTMLButtonElement).click())
    expect(props.onInstallTranslationModel).toHaveBeenCalledExactlyOnceWith(french.id)
    expect(props.onStartTranslationPreflight).not.toHaveBeenCalled()
  })

  it.each(['html', 'epub'])('runs installed %s with the selected fixed pair and foreground guidance', async format => {
    await render({ modelStatuses: { [french.id]: installed, [spanish.id]: { ...installed, modelId: spanish.id } }, selectedDocument: { ...props.selectedDocument!, format } })
    await select(spanish.id)
    const selectors = container.querySelectorAll('select')
    expect(selectors[1].value).toBe('es'); expect(selectors[1].disabled).toBe(true)
    expect(selectors[2].value).toBe('en'); expect(selectors[2].options).toHaveLength(1)
    expect(container.textContent).toContain('translation.models.bergamotNote')
    expect(container.querySelector('input[type=checkbox]')).toBeNull()
    await click('translation.workbench.run')
    expect(props.onStartTranslationPreflight).toHaveBeenCalledExactlyOnceWith({ documentUrl: '/uploads/book.html', modelId: spanish.id, sourceLanguage: 'es', targetLanguage: 'en', qualityMode: 'balanced', useHardwareAcceleration: false })
  })

  it('selects a supported worker in worker-only builds and disables uncompiled native choices', async () => {
    await render({ capabilities: { ...props.capabilities!, models: [{ ...opus, available: false }, french, spanish] } })
    expect(picker().value).toBe(french.id)
    expect(picker().options[0].disabled).toBe(true)
    await click('translation.models.title')
    expect(container.textContent).toContain('translation.models.engineUnavailable')
  })

  it('explains failed WebView gates, keeps native translation usable and permits removal of unavailable installed files', async () => {
    await select(french.id)
    await render({ capabilities: { ...props.capabilities!, models: [opus, { ...french, available: false, unavailableReason: 'webview' }] }, modelStatuses: { [opus.id]: installed, [french.id]: installed } })
    expect(picker().value).toBe(opus.id)
    expect(picker().options[1].disabled).toBe(true)
    expect(button('translation.workbench.run').disabled).toBe(false)
    await click('translation.models.title')
    expect(container.textContent).toContain('translation.models.webviewUnavailable')
    const workerRow = [...container.querySelectorAll('.translation-model-item')].find(r => r.textContent?.includes(french.name))!
    expect(workerRow.querySelector<HTMLButtonElement>('button')?.disabled).toBe(false)
  })

  it('disables document jobs for unsupported formats even with installed worker files', async () => {
    await render({ selectedDocument: { ...props.selectedDocument!, format: 'pdf' }, modelStatuses: { [french.id]: installed } })
    await select(french.id)
    expect(button('translation.workbench.run').disabled).toBe(true)
    expect(container.textContent).toContain('translation.workbench.htmlEpubOnly')
    await click('translation.workbench.run')
    expect(props.onStartTranslationPreflight).not.toHaveBeenCalled()
  })

  it('keeps saved variants available when no engine is runnable', async () => {
    await render({ capabilities: { ...props.capabilities!, available: false, models: [{ ...french, available: false, unavailableReason: 'webview' }] }, translatedDocuments: [{ id: 'saved', title: 'Saved', documentUrl: '/uploads/translated.html', sourceDocumentUrl: '/uploads/book.html', sourceLanguage: 'fr', targetLanguage: 'en', modelId: french.id, status: 'stored', createdAtMs: 1, updatedAtMs: 1 }] })
    expect(container.querySelector('select')).toBeNull()
    await click('common.view')
    expect(props.onOpenTranslatedDocument).toHaveBeenCalledExactlyOnceWith('/uploads/translated.html')
  })
})

describe('saved translations needing formatting review', () => {
  it.each([['stored', 'ready'], ['stored', 'stored'], ['needs-review', 'needs-review']])('keeps job %s and saved %s outcomes distinct', async (status, savedStatus) => {
    const jobId = 'sentence-job'
    await render({
      startState: {
        ...props.startState, jobId, message: 'Backend result', result: { jobId, status, message: 'Backend result' },
        progress: { jobId, status, message: 'Backend result', totalSegments: 0, completedSegments: 0, totalBatches: 0, completedBatches: 0, percent: 100, elapsedMs: 10 } as NonNullable<typeof props.startState.progress>,
      },
      translatedDocuments: [{ id: 'saved', title: 'Saved translation', documentUrl: '/uploads/translated.html', sourceDocumentUrl: '/uploads/book.html', sourceLanguage: 'fr', targetLanguage: 'en', modelId: opus.id, status: savedStatus, createdAtMs: 1, updatedAtMs: 1 }],
    })
    const review = status === 'needs-review'
    expect(container.querySelector('.translation-status-stack strong')?.textContent).toBe(`translation.status.${review ? 'translationNeedsReview' : 'translationComplete'}`)
    expect(container.querySelector('.translation-alert-message')?.textContent).toBe(`translation.status.${review ? 'translationNeedsReviewMessage' : 'translationCompleteMessage'}`)
    const label = review ? 'translation.status.translationNeedsReview' : 'translation.progress.status.completed'
    expect(container.querySelector('[role="progressbar"]')?.getAttribute('aria-valuetext')).toBe(label)
    expect(container.querySelector('[role="progressbar"]')?.getAttribute('aria-valuenow')).toBe('100')
    expect(container.querySelector('.translation-progress-meter.is-indeterminate')).toBeNull()
    expect(container.querySelector('.translation-document-item')?.textContent).toContain(label)
    await click('common.view')
    expect(props.onOpenTranslatedDocument).toHaveBeenCalledExactlyOnceWith('/uploads/translated.html')
    await act(async () => container.querySelector<HTMLButtonElement>('[aria-label="translation.status.dismissTranslation"]')!.click())
    expect(container.querySelector('.translation-status-stack')).toBeNull()
    expect(container.querySelector('.translation-document-item')?.textContent).toContain(label)
  })
})

const pack = 'alignment-awesome-align-int8'
const readyFormatting = { available: true, reason: 'available' as const, modelId: pack, archiveBytes: 153000000 }
describe('retired formatting-pack management', () => {
  it.each([false, true])('never offers consent/download or requests strict translation (old capability available=%s)', async available => {
    await render({ capabilities: { ...props.capabilities!, formatting: { ...readyFormatting, available } }, modelStatuses: { [opus.id]: installed } })
    expect(container.querySelector('section input[type=checkbox]')).toBeNull()
    expect(container.textContent).not.toContain('translation.formatting.retired')
    expect([...picker().options].some(o => o.value === pack)).toBe(false)
    await click('translation.workbench.run')
    expect(props.onStartTranslationPreflight).toHaveBeenLastCalledWith(expect.not.objectContaining({ preserveFormatting: true }))
    expect(props.onInstallTranslationModel).not.toHaveBeenCalled()
  })

  it.each([true, false])('keeps complete/partial zero-byte packs removable without a backend or document (installed=%s)', async complete => {
    await render({
      selectedDocument: null,
      capabilities: { ...props.capabilities!, available: false, formatting: { ...readyFormatting, available: false, reason: 'retired' } },
      modelStatuses: { [pack]: { ...installed, modelId: pack, installed: complete, hasStoredFiles: true, installedBytes: 0 } },
    })
    const section = container.querySelector('section[aria-label="translation.formatting.packName"]')!
    expect(section.textContent).toContain('translation.formatting.retired')
    expect(section.querySelector('input')).toBeNull()
    await act(async () => section.querySelector<HTMLButtonElement>('button')!.click())
    expect(props.onInstallTranslationModel).not.toHaveBeenCalled()
    expect(props.onRemoveTranslationModel).not.toHaveBeenCalled()
    expect(container.querySelector('[role="dialog"]')?.textContent).toContain('translation.confirmModelRemove.title')
    await act(async () => container.querySelector<HTMLButtonElement>('[role="dialog"] .app-dialog-danger')!.click())
    expect(props.onRemoveTranslationModel).toHaveBeenCalledExactlyOnceWith(pack)
  })
})

  it.each(['loading-alignment', 'aligning-batch', 'reconstructing-batch'])('localizes the %s progress phase and keeps Cancel reachable', async status => {
    await render({ startState: { ...props.startState, checking: true, jobId: 'strict-job', message: 'Formatting progress', progress: { jobId: 'strict-job', status, message: 'Internal diagnostic', totalSegments: 10, completedSegments: 0, totalBatches: 2, completedBatches: 0, percent: 0, elapsedMs: 0 } as NonNullable<typeof props.startState.progress> } })
    const key = { 'loading-alignment': 'loadingAlignment', 'aligning-batch': 'aligningBatch', 'reconstructing-batch': 'reconstructingBatch' }[status]!
    expect(container.textContent).toContain('translation.progress.status.' + key)
    expect(container.textContent).not.toContain('Formatting progress')
    await click('common.cancel')
    expect(props.onCancelTranslation).toHaveBeenCalledOnce()
  })

/** Model the native saved-row contract, independently of job completion status. */
function savedDocument(overrides: Partial<TranslatedDocumentInfo> = {}): TranslatedDocumentInfo {
  return {
    id: 'saved-one', title: 'Saved translation', documentUrl: '/uploads/saved-one.html',
    sourceDocumentUrl: '/uploads/source.html', sourceLanguage: 'fr', targetLanguage: 'en',
    modelId: opus.id, status: 'ready', createdAtMs: 1, updatedAtMs: 1, ...overrides,
  }
}

// Exercise the extracted view through its real parent to cover status and callback boundaries.
describe('saved-document presentation lifecycle', () => {
  it.each(['ready', 'stored', 'needs-review'])('opens a saved %s result without a source, runtime or installed model', async status => {
    const doc = savedDocument({ status, modelId: 'removed-model' })
    await render({ selectedDocument: null, capabilities: null, modelStatuses: {}, translatedDocuments: [doc] })
    const row = container.querySelector('.translation-document-item')!
    expect(row.textContent).toContain('removed-model')
    expect(row.textContent).toContain(status === 'needs-review'
      ? 'translation.status.translationNeedsReview' : 'translation.progress.status.completed')
    expect(row.textContent).toContain('French → English')
    await click('common.view')
    expect(props.onOpenTranslatedDocument).toHaveBeenCalledExactlyOnceWith(doc.documentUrl)
    expect(props.onStartTranslationPreflight).not.toHaveBeenCalled()
  })

  it('requires confirmation, restores focus on cancel and deletes only the chosen document', async () => {
    const first = savedDocument()
    const second = savedDocument({ id: 'saved-two', title: 'Second translation', documentUrl: '/uploads/saved-two.html' })
    await render({ translatedDocuments: [first, second] })
    const trigger = container.querySelectorAll<HTMLButtonElement>('.translation-document-delete-btn')[1]
    trigger.focus()
    await act(async () => trigger.click())
    const dialog = container.querySelector<HTMLElement>('[role="dialog"]')!
    expect(dialog.getAttribute('aria-modal')).toBe('true')
    expect(document.getElementById(dialog.getAttribute('aria-labelledby')!)?.textContent).toBe('translation.confirmDelete.title')
    expect(document.getElementById(dialog.getAttribute('aria-describedby')!)?.textContent).toContain(second.title)
    expect(dialog.contains(document.activeElement)).toBe(true)
    expect(props.onDeleteTranslatedDocument).not.toHaveBeenCalled()
    await act(async () => dialog.querySelector<HTMLButtonElement>('.app-dialog-cancel')!.click())
    expect(container.querySelector('[role="dialog"]')).toBeNull()
    expect(document.activeElement).toBe(trigger)
    expect(props.onDeleteTranslatedDocument).not.toHaveBeenCalled()
    await act(async () => trigger.click())
    await act(async () => container.querySelector<HTMLButtonElement>('.app-dialog-danger')!.click())
    expect(props.onDeleteTranslatedDocument).toHaveBeenCalledExactlyOnceWith(second.id)
    expect(container.querySelector('[role="dialog"]')).toBeNull()
  })

  it('closes confirmation when refresh removes its target instead of deleting another row', async () => {
    const first = savedDocument()
    const second = savedDocument({ id: 'saved-two', title: 'Second translation' })
    await render({ translatedDocuments: [first, second] })
    await act(async () => container.querySelector<HTMLButtonElement>('.translation-document-delete-btn')!.click())
    expect(container.querySelector('[role="dialog"]')).not.toBeNull()
    await render({ translatedDocuments: [second] })
    expect(container.querySelector('[role="dialog"]')).toBeNull()
    expect(container.querySelector('.translation-document-item')?.textContent).toContain(second.title)
    expect(props.onDeleteTranslatedDocument).not.toHaveBeenCalled()
  })

  it('rebinds an open confirmation to refreshed metadata for the same ID', async () => {
    const doc = savedDocument()
    await render({ translatedDocuments: [doc] })
    await click('common.delete')
    await render({ translatedDocuments: [{ ...doc, title: 'Refreshed title' }] })
    const description = container.querySelector('.app-dialog-header p')!.textContent
    expect(description).toContain('Refreshed title')
    expect(description).not.toContain(doc.title)
    await act(async () => container.querySelector<HTMLButtonElement>('.app-dialog-danger')!.click())
    expect(props.onDeleteTranslatedDocument).toHaveBeenCalledExactlyOnceWith(doc.id)
  })

  it('keeps collapse state across parent updates and exposes the refreshed list on reopen', async () => {
    await render({ translatedDocuments: [savedDocument()] })
    await click('translation.documents.title')
    const toggle = container.querySelector<HTMLButtonElement>('[aria-label="translation.documents.ariaLabel"] .panel-toggle')!
    expect(toggle.getAttribute('aria-expanded')).toBe('false')
    expect(container.querySelector('.translation-document-list')).toBeNull()
    const fresh = savedDocument({ title: 'Refreshed translation' })
    await render({ translatedDocuments: [fresh], loading: true })
    expect(toggle.getAttribute('aria-expanded')).toBe('false')
    await act(async () => toggle.click())
    expect(container.querySelector('.translation-document-item')?.textContent).toContain(fresh.title)
    await render({ translatedDocuments: [] })
    expect(container.querySelector('.translation-document-list')).toBeNull()
    expect(container.textContent).toContain('translation.documents.empty')
  })

  it('keeps a failed deletion retryable and displays the subsequent manager outcome', async () => {
    const doc = savedDocument()
    await render({ translatedDocuments: [doc] })
    await click('common.delete')
    await act(async () => container.querySelector<HTMLButtonElement>('.app-dialog-danger')!.click())
    await render({ deleteState: { id: doc.id, deleted: false, bytesFreed: 0, message: 'Storage is busy' } })
    expect(container.querySelector('[aria-label="translation.documents.ariaLabel"] [role="status"]')?.textContent).toContain('Storage is busy')
    expect(container.querySelector('.translation-document-item')).not.toBeNull()
    await act(async () => container.querySelector<HTMLButtonElement>('[aria-label="translation.status.dismissDelete"]')!.click())
    await click('common.delete')
    await act(async () => container.querySelector<HTMLButtonElement>('.app-dialog-danger')!.click())
    expect(props.onDeleteTranslatedDocument).toHaveBeenCalledTimes(2)
    expect(props.onDeleteTranslatedDocument).toHaveBeenLastCalledWith(doc.id)
    await render({ translatedDocuments: [], deleteState: { id: doc.id, deleted: true, bytesFreed: 10, message: 'Removed' } })
    expect(container.querySelector('.translation-document-item')).toBeNull()
    expect(container.querySelector('[aria-label="translation.documents.ariaLabel"] [role="status"]')?.textContent).toContain('translation.status.documentDeleted')
  })

  it.each(['job', 'delete'])('dismissing %s status does not hide the other operation or new outcomes', async dismissed => {
    const jobId = 'completed-job'
    await render({
      translatedDocuments: [savedDocument()],
      startState: { ...props.startState, jobId, message: 'Job saved', result: { jobId, status: 'stored', message: 'Job saved' } },
      deleteState: { id: 'previous-document', deleted: true, bytesFreed: 10, message: 'Previous deleted' },
    })
    const jobSelector = '[aria-label="translation.status.dismissTranslation"]'
    const deleteSelector = '[aria-label="translation.status.dismissDelete"]'
    const dismissedSelector = dismissed === 'job' ? jobSelector : deleteSelector
    const otherSelector = dismissed === 'job' ? deleteSelector : jobSelector
    await act(async () => container.querySelector<HTMLButtonElement>(dismissedSelector)!.click())
    expect(container.querySelector(dismissedSelector)).toBeNull()
    expect(container.querySelector(otherSelector)).not.toBeNull()
    await render({ loading: true })
    expect(container.querySelector(dismissedSelector)).toBeNull()
    await render({
      startState: { ...props.startState, message: 'New job saved', result: { jobId: 'next-job', status: 'stored', message: 'New job saved' } },
      deleteState: { id: 'another-document', deleted: true, bytesFreed: 20, message: 'Another deleted' },
    })
    expect(container.querySelector(jobSelector)).not.toBeNull()
    expect(container.querySelector(deleteSelector)).not.toBeNull()
  })
})
