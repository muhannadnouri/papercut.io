// @vitest-environment jsdom
import { act, type ComponentProps } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { TranslationPanel } from './TranslationPanel'
import en from '../../i18n/locales/en.json'
import type { TranslationModelInfo, TranslationModelStatus } from '../api/nativeTranslation'

vi.mock('react-i18next', async original => ({ ...await original<typeof import('react-i18next')>(), useTranslation: () => ({ t: (key: string) => key, i18n: { language: 'en' } }) }))
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
  it.each([
    ['runtime-unavailable', 'runtimeUnavailable'],
    ['acceptance-pending', 'pending'],
  ] as const)('scopes the %s explanation to strict alignment and permits normal OPUS jobs', async (reason, key) => {
    await render({ capabilities: { ...props.capabilities!, formatting: { available: false, reason, modelId: '', archiveBytes: 0 } }, modelStatuses: { [opus.id]: { ...installed, modelId: opus.id } } })
    expect(en.translation.formatting[key]).toMatch(/^Strict formatting\b/)
    expect(en.translation.formatting[key]).toContain('Standard translation does not require it.')
    expect(container.textContent).toContain(`translation.formatting.${key}`)
    expect(container.textContent).toContain('translation.models.ctranslate2BoundaryNote')
    const strict = container.querySelector('input[type=checkbox]') as HTMLInputElement
    expect(strict.disabled).toBe(true)
    expect(strict.checked).toBe(false)
    expect(button('translation.workbench.run').disabled).toBe(false)
    await click('translation.workbench.run')
    expect(props.onStartTranslationPreflight).toHaveBeenCalledExactlyOnceWith({ documentUrl: '/uploads/book.html', modelId: opus.id, sourceLanguage: 'fr', targetLanguage: 'en', qualityMode: 'balanced', useHardwareAcceleration: false })
  })

  it('explains whole-sentence translation and review fallback only for CTranslate2 without enabling strict formatting', async () => {
    expect(en.translation.models.ctranslate2BoundaryNote).toContain('Translates whole sentences.')
    expect(en.translation.models.ctranslate2BoundaryNote).toContain('formatted original is kept beside the translation for review')
    expect(container.textContent).toContain('translation.models.ctranslate2BoundaryNote')
    const strict = container.querySelector('input[type=checkbox]') as HTMLInputElement
    expect(strict.disabled).toBe(true)
    expect(strict.checked).toBe(false)
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
  it.each(['stored', 'needs-review'])('keeps %s outcomes distinct in the completion, progress and saved list', async status => {
    const jobId = 'sentence-job'
    await render({
      startState: {
        ...props.startState, jobId, message: 'Backend result', result: { jobId, status, message: 'Backend result' },
        progress: { jobId, status, message: 'Backend result', totalSegments: 0, completedSegments: 0, totalBatches: 0, completedBatches: 0, percent: 100, elapsedMs: 10 } as NonNullable<typeof props.startState.progress>,
      },
      translatedDocuments: [{ id: 'saved', title: 'Saved translation', documentUrl: '/uploads/translated.html', sourceDocumentUrl: '/uploads/book.html', sourceLanguage: 'fr', targetLanguage: 'en', modelId: opus.id, status, createdAtMs: 1, updatedAtMs: 1 }],
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
function formattingCheckbox() { return container.querySelector<HTMLInputElement>('section input[type=checkbox]')! }
async function chooseFormatting() { await act(async () => formattingCheckbox().click()) }

describe('strict formatting setup and consent (test-only ready capability)', () => {
  it('keeps the release gate honest without a download or automatic worker fallback', async () => {
    await render({ capabilities: { ...props.capabilities!, formatting: { ...readyFormatting, available: false, reason: 'acceptance-pending' } }, modelStatuses: { [opus.id]: installed, [pack]: installed } })
    expect(formattingCheckbox().disabled).toBe(true)
    expect(container.textContent).toContain('translation.formatting.pending')
    expect(button('translation.workbench.run').disabled).toBe(false)
    await click('translation.workbench.run')
    expect(props.onStartTranslationPreflight).toHaveBeenLastCalledWith(expect.not.objectContaining({ preserveFormatting: true }))
    expect(props.onInstallTranslationModel).not.toHaveBeenCalled()
    expect(picker().value).toBe(opus.id)
  })

  it.each([undefined, { ...installed, modelId: pack, installed: false }, { ...installed, modelId: pack, installing: true }])('requires verified idle assets (%j) and never installs automatically', async status => {
    await render({ capabilities: { ...props.capabilities!, formatting: readyFormatting }, modelStatuses: { [opus.id]: installed, ...(status ? { [pack]: status } : {}) } })
    expect(formattingCheckbox().disabled).toBe(true)
    expect(formattingCheckbox().checked).toBe(false)
    expect([...picker().options].some(o => o.value === pack)).toBe(false)
    expect(props.onInstallTranslationModel).not.toHaveBeenCalled()
  })

  it('reuses explicit pack installation and removal confirmation outside the engine selector', async () => {
    await render({ capabilities: { ...props.capabilities!, formatting: readyFormatting } })
    const setup = container.querySelector('section[aria-label="translation.formatting.label"]')!
    await act(async () => setup.querySelector<HTMLButtonElement>('button')!.click())
    expect(props.onInstallTranslationModel).toHaveBeenCalledExactlyOnceWith(pack)
    await render({ modelStatuses: { [pack]: { ...installed, modelId: pack } } })
    await act(async () => setup.querySelector<HTMLButtonElement>('button')!.click())
    expect(props.onRemoveTranslationModel).not.toHaveBeenCalled()
    // Shared confirmation names this pack, not the currently selected translation engine.
    expect(container.querySelector('[role="dialog"]')?.textContent).toContain('translation.confirmModelRemove.title')
    await act(async () => container.querySelector<HTMLButtonElement>('[role="dialog"] .app-dialog-danger')!.click())
    expect(props.onRemoveTranslationModel).toHaveBeenCalledExactlyOnceWith(pack)
  })

  it('passes explicit strict consent, blocks pack loss and resets consent on engine changes', async () => {
    await render({ capabilities: { ...props.capabilities!, formatting: readyFormatting }, modelStatuses: { [opus.id]: installed, [pack]: { ...installed, modelId: pack } } })
    expect(formattingCheckbox().checked).toBe(false)
    await chooseFormatting()
    await click('translation.workbench.run')
    expect(props.onStartTranslationPreflight).toHaveBeenLastCalledWith(expect.objectContaining({ modelId: opus.id, preserveFormatting: true }))
    await render({ modelStatuses: { [opus.id]: installed, [pack]: { ...installed, modelId: pack, installed: false } } })
    expect(formattingCheckbox().checked).toBe(true)
    expect(button('translation.workbench.run').disabled).toBe(true)
    await chooseFormatting()
    expect(button('translation.workbench.run').disabled).toBe(false)
    await render({ modelStatuses: { [opus.id]: installed, [pack]: { ...installed, modelId: pack } } })
    await chooseFormatting()
    await select(french.id)
    expect(formattingCheckbox()).toBeNull()
    await select(opus.id)
    expect(formattingCheckbox().checked).toBe(false)
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
