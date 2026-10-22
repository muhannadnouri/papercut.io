// @vitest-environment jsdom
import { act, type ComponentProps } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { TranslationReviewDialog } from './TranslationReviewDialog'
import { TranslationError } from '../api/nativeTranslation'
import type { ReviewDraft, ReviewEditRequest } from '../review/types'

const mock = vi.hoisted(() => ({ load: vi.fn(), save: vi.fn(), finish: vi.fn(),
  t: (key: string, options?: Record<string, unknown>) => key + (options ? ' ' + Object.values(options).join(' ') : '') }))
vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: mock.t }) }))
vi.mock('../api/nativeTranslation', async original => ({ ...await original<typeof import('../api/nativeTranslation')>(),
  loadTranslationReview: mock.load, saveTranslationReview: mock.save, finishTranslationReview: mock.finish }))
vi.mock('../../i18n', () => ({ default: { t: (key: string) => key }, currentAppLocale: () => 'en' }))

let root: Root
let container: HTMLDivElement
let props: ComponentProps<typeof TranslationReviewDialog>
let stored: ReviewDraft
function fixture(): ReviewDraft {
  return {
    document: { expected: { schemaVersion: 1, policy: 'opus-assisted-emphasis-v1', variantId: 'saved', generation: 'a'.repeat(32), revision: 0 },
      normalizationPolicy: 'source-v1', sourceHash: 'a', publishedHash: 'b', blocks: ['A bright morning.', 'A quiet evening.'].map((text, ordinal) => ({
        key: { ordinal, sourceIdentity: 'source-' + ordinal }, structureIdentity: 'structure-' + ordinal, text, textHash: 'text-' + ordinal,
        locked: [], fixed: [], editable: [], obligations: ['emphasis'], approved: false,
      })) },
    sourcePreviewHtml: '', previewHtml: '', assetPaths: {}, publicationError: null,
    blockPreviews: [{ sourceHtml: '<p><strong>Un matin</strong> <em>lumineux.</em></p>', inheritedMarks: [], blockers: [] }, { sourceHtml: '<p>Un soir calme.</p>', inheritedMarks: [], blockers: [] }],
  }
}
async function render(overrides: Partial<typeof props> = {}) {
  props = { ...props, ...overrides }
  await act(async () => root.render(<TranslationReviewDialog {...props} />))
}
function button(key: string) {
  const result = [...container.querySelectorAll('button')].find(row => row.textContent === key || row.getAttribute('aria-label') === key)
  if (!result) throw Error('Missing button ' + key)
  return result
}
async function click(key: string) { await act(async () => button(key).click()) }
function english() { return container.querySelector<HTMLDivElement>('.translation-review-english')! }
function deferred<T>() { let resolve!: (value: T) => void; let reject!: (error: unknown) => void; const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no }); return { promise, resolve, reject } }
/** Real DOM ranges cross semantic text nodes; no synthesized model output or browser layout is claimed. */
async function selectText(start: number, end: number, event = 'selectionchange') {
  await act(async () => {
    const walker = document.createTreeWalker(english(), NodeFilter.SHOW_TEXT)
    const nodes: Text[] = []
    while (walker.nextNode()) nodes.push(walker.currentNode as Text)
    const point = (offset: number): [Text, number] => {
      for (const node of nodes) { if (offset <= node.length) return [node, offset]; offset -= node.length }
      throw Error('Selection outside English')
    }
    const range = document.createRange(); range.setStart(...point(start)); range.setEnd(...point(end))
    const selection = window.getSelection()!; selection.removeAllRanges(); selection.addRange(range)
    if (event === 'selectionchange') document.dispatchEvent(new Event(event))
    else english().dispatchEvent(new Event(event, { bubbles: true }))
  })
}
beforeEach(() => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  mock.load.mockReset(); mock.save.mockReset(); mock.finish.mockReset()
  stored = fixture()
  mock.load.mockImplementation(async () => structuredClone(stored))
  mock.save.mockImplementation(async (request: ReviewEditRequest) => {
    if (request.expected.revision !== stored.document.expected.revision) throw new TranslationError('review-conflict', 'conflict')
    const block = stored.document.blocks.find(row => row.key.ordinal === request.block.ordinal)!
    if (request.edit.action === 'replaceMarks') { block.editable = request.edit.marks; block.approved = false }
    else if (request.edit.action === 'approve') block.approved = true
    else { block.editable = []; block.approved = false }
    stored.document.expected.revision++
    return structuredClone(stored)
  })
  props = { document: { id: 'saved', title: 'Book', documentUrl: '/uploads/saved/index.html', sourceDocumentUrl: '/uploads/source/index.html', sourceLanguage: 'fr', targetLanguage: 'en', modelId: 'opus-mt-fr-en-ctranslate2', status: 'needs-review', createdAtMs: 1, updatedAtMs: 1 }, onClose: vi.fn(), onPublished: vi.fn() }
  mock.finish.mockResolvedValue({ ...props.document, status: 'ready' })
  container = document.createElement('div'); document.body.append(container); root = createRoot(container)
})
afterEach(async () => { await act(async () => root.unmount()); container.remove(); window.getSelection()?.removeAllRanges(); vi.unstubAllGlobals() })

describe('assisted formatting editor', () => {
  it('renders one immutable English paragraph and inert semantic source without live resources or identifiers', async () => {
    stored.blockPreviews[0].sourceHtml = '<p id="collision" style="position:fixed"><strong>Original</strong><a href="https://external">link</a><img src="https://external/pixel" alt="diagram"><script>bad()</script><form><button>submit</button></form></p>'
    await render()
    expect(english().textContent).toBe(stored.document.blocks[0].text)
    expect(english().hasAttribute('contenteditable')).toBe(false)
    expect(english().lang).toBe('en')
    expect(english().dir).toBe('ltr')
    expect(container.querySelector('textarea')?.lang).toBe('en')
    expect(container.textContent).not.toContain(stored.document.blocks[1].text)
    const source = container.querySelector('.translation-review-source')!
    expect(source.hasAttribute('open')).toBe(false)
    expect(source.querySelector('[lang]')?.getAttribute('lang')).toBe('fr')
    expect(source.querySelector('strong')?.textContent).toBe('Original')
    expect(source.querySelector('a,img,script,form,[id],[style]')).toBeNull()
    expect(source.textContent).toContain('linkdiagram')
    expect(mock.finish).not.toHaveBeenCalled()
  })

  it('retains pointer/touch selection across toolbar focus, stages marks, removes and undoes without editing text', async () => {
    await render()
    await selectText(2, 8, 'touchend')
    const bold = button('translation.review.bold')
    await act(async () => { bold.focus(); document.dispatchEvent(new Event('selectionchange')); bold.click() })
    expect(bold.getAttribute('aria-pressed')).toBe('true')
    // The DOM may collapse its range while React replaces text nodes with semantic runs.
    await act(async () => { window.getSelection()?.collapse(english(), 0); document.dispatchEvent(new Event('selectionchange')) })
    await click('translation.review.italic')
    expect(english().querySelector('strong em')?.textContent).toBe('bright')
    await click('translation.review.italic')
    expect(english().querySelector('strong')?.textContent).toBe('bright')
    expect(english().textContent).toBe('A bright morning.')
    expect(mock.save).not.toHaveBeenCalled()
    const remove = container.querySelector<HTMLButtonElement>('.translation-review-marks li button')!
    await act(async () => remove.click())
    expect(english().querySelector('strong')).toBeNull()
    await click('translation.review.undo')
    expect(english().querySelector('strong')?.textContent).toBe('bright')
    await click('translation.review.reset')
    expect(english().querySelector('strong')).toBeNull()
    await click('translation.review.undo')
    expect(english().querySelector('strong')?.textContent).toBe('bright')
  })

  it('clears a prior logical range when a new selection belongs to source or crosses outside English', async () => {
    await render(); await selectText(2, 8)
    expect(button('translation.review.bold').disabled).toBe(false)
    const sourceText = container.querySelector('.translation-review-source strong')!.firstChild!
    await act(async () => {
      const range = document.createRange(); range.selectNodeContents(sourceText)
      const selection = window.getSelection()!; selection.removeAllRanges(); selection.addRange(range)
      document.dispatchEvent(new Event('selectionchange'))
    })
    expect(button('translation.review.bold').disabled).toBe(true)
    await selectText(2, 8)
    await act(async () => {
      const range = document.createRange(); range.setStart(sourceText, 0); range.setEnd(english().firstChild!, 3)
      const selection = window.getSelection()!; selection.removeAllRanges(); selection.addRange(range)
      document.dispatchEvent(new Event('selectionchange'))
    })
    expect(button('translation.review.bold').disabled).toBe(true)
    expect(mock.save).not.toHaveBeenCalled()
  })

  it('fails selection edits closed when grapheme segmentation is unavailable while retaining readable text', async () => {
    const original = Intl.Segmenter
    Object.defineProperty(Intl, 'Segmenter', { configurable: true, value: undefined })
    try {
      await render(); await selectText(2, 8)
      expect(container.textContent).toContain('translation.review.selectionUnsupported')
      expect(button('translation.review.bold').disabled).toBe(true)
      expect(button('translation.review.italic').disabled).toBe(true)
      expect(english().textContent).toBe('A bright morning.')
      expect(button('translation.review.approve').disabled).toBe(false)
    } finally { Object.defineProperty(Intl, 'Segmenter', { configurable: true, value: original }) }
  })

  it('supports native readonly keyboard selection and retains exact Unicode text while rejecting protected selections', async () => {
    stored.document.blocks[0].text = 'A 😀 bright.'
    stored.document.blocks[0].locked = [{ start: 4, end: 10 }]
    await render()
    const textarea = container.querySelector('textarea')!
    expect(textarea.readOnly).toBe(true)
    await act(async () => { textarea.focus(); textarea.setSelectionRange(2, 4); textarea.dispatchEvent(new KeyboardEvent('keyup', { key: 'ArrowRight', shiftKey: true, bubbles: true })) })
    await click('translation.review.italic')
    expect(english().querySelector('em')?.textContent).toBe('😀')
    await act(async () => { textarea.setSelectionRange(2, 3); textarea.dispatchEvent(new KeyboardEvent('keyup', { key: 'ArrowRight', shiftKey: true, bubbles: true })) })
    expect(button('translation.review.bold').disabled).toBe(true)
    expect(container.textContent).toContain('translation.review.invalidSelection')
    await act(async () => { textarea.setSelectionRange(5, 11); textarea.dispatchEvent(new KeyboardEvent('keyup', { key: 'ArrowRight', shiftKey: true, bubbles: true })) })
    expect(button('translation.review.bold').disabled).toBe(true)
    expect(english().textContent).toBe('A 😀 bright.')
    expect(textarea.value).toBe('A 😀 bright.')
  })

  it('maps normalized textarea newlines back to exact native CRLF scalar coordinates', async () => {
    stored.document.blocks[0].text = 'First.\r\nA bright second.'
    await render()
    const textarea = container.querySelector('textarea')!
    expect(textarea.value).toBe('First.\nA bright second.')
    await act(async () => { textarea.focus(); textarea.setSelectionRange(9, 15); textarea.dispatchEvent(new KeyboardEvent('keyup', { key: 'ArrowRight', shiftKey: true, bubbles: true })) })
    await click('translation.review.bold')
    expect(english().querySelector('strong')?.textContent).toBe('bright')
    expect(english().textContent).toBe('First.\r\nA bright second.')
    await click('translation.review.save')
    expect(mock.save).toHaveBeenCalledWith(expect.objectContaining({ edit: { action: 'replaceMarks', marks: [{ kind: 'bold', range: { start: 10, end: 16 } }] } }))
  })

  it('keeps immutable baseline marks distinct and does not offer a misleading removal toggle', async () => {
    stored.document.blocks[0].fixed = [{ kind: 'bold', range: { start: 2, end: 8 } }]
    await render(); await selectText(2, 8)
    expect(english().querySelector('strong')?.textContent).toBe('bright')
    expect(button('translation.review.bold').disabled).toBe(true)
    expect(button('translation.review.bold').getAttribute('aria-pressed')).toBe('true')
    expect(container.textContent).toContain('translation.review.baselineHint')
    expect(container.querySelector('.translation-review-marks li')).toBeNull()
  })

  it('retains readonly ancestor bold in both paragraph previews without adding editable marks', async () => {
    stored.blockPreviews.forEach((preview, ordinal) => {
      preview.sourceHtml = `<strong>${preview.sourceHtml}</strong>`
      preview.inheritedMarks = [{ kind: 'bold', range: { start: 0, end: Array.from(stored.document.blocks[ordinal].text).length } }]
    })
    await render()
    for (let ordinal = 0; ordinal < stored.document.blocks.length; ordinal++) {
      expect(english().querySelector('strong')?.textContent).toBe(stored.document.blocks[ordinal].text)
      expect(container.querySelector('.translation-review-source > div > strong')).not.toBeNull()
      await selectText(2, 7)
      expect(button('translation.review.bold').disabled).toBe(true)
      expect(button('translation.review.bold').getAttribute('aria-pressed')).toBe('true')
      expect(button('translation.review.italic').disabled).toBe(false)
      expect(container.textContent).toContain('translation.review.baselineHint')
      expect(container.querySelector('.translation-review-marks li')).toBeNull()
      if (ordinal === 0) await click('translation.review.next')
    }
    expect(stored.document.blocks.every(block => block.fixed.length === 0 && block.editable.length === 0)).toBe(true)
    expect(mock.save).not.toHaveBeenCalled()
  })

  it('resumes editable marks beside inherited italic and publishes without saving presentation context', async () => {
    stored.blockPreviews[0] = { sourceHtml: '<em><blockquote><p>Un matin lumineux.</p></blockquote></em>',
      inheritedMarks: [{ kind: 'italic', range: { start: 0, end: 17 } }], blockers: [] }
    await render()
    expect(container.querySelector('.translation-review-source em blockquote p')?.textContent).toBe('Un matin lumineux.')
    expect(english().querySelector('em')?.textContent).toBe('A bright morning.')
    await selectText(2, 8)
    expect(button('translation.review.italic').disabled).toBe(true)
    expect(button('translation.review.italic').getAttribute('aria-pressed')).toBe('true')
    await click('translation.review.bold')
    expect(english().querySelector('strong em')?.textContent).toBe('bright')
    await click('translation.review.save')
    expect(mock.save.mock.calls[0][0]).toEqual({ expected: { ...stored.document.expected, revision: 0 }, block: stored.document.blocks[0].key,
      edit: { action: 'replaceMarks', marks: [{ kind: 'bold', range: { start: 2, end: 8 } }] } })
    await click('translation.review.reload')
    expect(mock.load).toHaveBeenCalledTimes(2)
    expect(english().querySelector('strong em')?.textContent).toBe('bright')
    expect([...english().querySelectorAll('em')].map(node => node.textContent).join('')).toBe('A bright morning.')
    expect(stored.document.blocks[0].fixed).toEqual([])
    expect(stored.document.blocks[0].editable).toEqual([{ kind: 'bold', range: { start: 2, end: 8 } }])
    await selectText(2, 8)
    expect(button('translation.review.italic').disabled).toBe(true)
    await click('translation.review.approve')
    await click('translation.review.next'); await click('translation.review.approve')
    await click('translation.review.finish')
    expect(mock.finish).toHaveBeenCalledWith(stored.document.expected)
    expect(mock.save.mock.calls.map(([request]) => request.edit)).toEqual([
      { action: 'replaceMarks', marks: [{ kind: 'bold', range: { start: 2, end: 8 } }] }, { action: 'approve' }, { action: 'approve' },
    ])
    expect(props.onPublished).toHaveBeenCalledTimes(1)
  })

  it('removes a resumed redundant edit without removing readonly inherited emphasis', async () => {
    stored.blockPreviews[0].inheritedMarks = [{ kind: 'bold', range: { start: 0, end: 17 } }]
    stored.document.blocks[0].editable = [{ kind: 'bold', range: { start: 2, end: 8 } }]
    await render(); await selectText(2, 8)
    expect(button('translation.review.bold').disabled).toBe(true)
    const remove = container.querySelector<HTMLButtonElement>('.translation-review-marks li button')!
    expect(remove.disabled).toBe(false)
    await act(async () => remove.click())
    expect(english().querySelector('strong')?.textContent).toBe('A bright morning.')
    expect(container.querySelector('.translation-review-marks li')).toBeNull()
    await click('translation.review.save'); await click('translation.review.reload')
    expect(mock.save.mock.calls[0][0].edit).toEqual({ action: 'replaceMarks', marks: [] })
    expect(stored.document.blocks[0].editable).toEqual([])
    expect(english().querySelector('strong')?.textContent).toBe('A bright morning.')
    await selectText(2, 8)
    expect(button('translation.review.bold').disabled).toBe(true)
    expect(button('translation.review.bold').getAttribute('aria-pressed')).toBe('true')
  })

  it('requires an explicit dirty-navigation choice and saves before moving with revision-bound approval', async () => {
    await render(); await selectText(2, 8); await click('translation.review.bold')
    await click('translation.review.next')
    expect(container.querySelectorAll('[role="dialog"]')).toHaveLength(1)
    expect(container.textContent).toContain('translation.review.dirtyPrompt')
    expect(english().textContent).toBe('A bright morning.')
    await click('translation.review.stay')
    expect(container.querySelector('.translation-review-confirm')).toBeNull()
    expect(document.activeElement).toBe(english())
    await click('translation.review.approve')
    expect(mock.save.mock.calls.map(([request]) => [request.expected.revision, request.edit.action])).toEqual([[0, 'replaceMarks'], [1, 'approve']])
    expect(button('translation.review.approved').disabled).toBe(true)
    await click('translation.review.next')
    expect(english().textContent).toBe('A quiet evening.')
    await selectText(2, 7); await click('translation.review.italic'); await click('translation.review.previous'); await click('translation.review.saveContinue')
    expect(english().textContent).toBe('A bright morning.')
    expect(stored.document.blocks[1].editable).toEqual([{ kind: 'italic', range: { start: 2, end: 7 } }])
  })

  it('keeps local work when the locale binding or parent callbacks change for the same document', async () => {
    await render(); await selectText(2, 8); await click('translation.review.bold')
    const previousT = mock.t
    try {
      mock.t = (key, options) => previousT(key, options)
      await render({ onClose: vi.fn() })
      expect(mock.load).toHaveBeenCalledTimes(1)
      expect(english().querySelector('strong')?.textContent).toBe('bright')
      expect(container.textContent).toContain('translation.review.unsaved')
    } finally { mock.t = previousT }
  })

  it('protects Escape/close and draft reload from silent loss while retaining local marks on a CAS conflict', async () => {
    await render(); await selectText(2, 8); await click('translation.review.bold')
    await act(async () => document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })))
    expect(props.onClose).not.toHaveBeenCalled()
    await click('translation.review.stay')
    mock.save.mockRejectedValueOnce(new TranslationError('review-conflict', 'stale'))
    await click('translation.review.save')
    expect(container.textContent).toContain('translation.review.conflict')
    expect(english().querySelector('strong')?.textContent).toBe('bright')
    expect(button('translation.review.save').disabled).toBe(true)
    await click('translation.review.reload')
    expect(container.textContent).toContain('translation.review.reloadPrompt')
    expect(mock.load).toHaveBeenCalledTimes(1)
    await click('translation.review.stay')
    mock.load.mockRejectedValueOnce(Error('reload failed'))
    await click('translation.review.reload'); await click('translation.review.discardContinue')
    expect(mock.load).toHaveBeenCalledTimes(2)
    expect(container.textContent).toContain('reload failed')
    expect(english().querySelector('strong')?.textContent).toBe('bright')
    expect(button('translation.review.save').disabled).toBe(true)
    expect(button('translation.review.approve').disabled).toBe(true)
    expect(button('translation.review.finish').disabled).toBe(true)
    await click('translation.review.reload'); await click('translation.review.discardContinue')
    expect(mock.load).toHaveBeenCalledTimes(3)
    expect(english().querySelector('strong')).toBeNull()
    expect(button('translation.review.approve').disabled).toBe(false)
  })

  it('keeps the newer revision after save succeeds but approval fails, and ignores duplicate synchronous actions', async () => {
    await render(); await selectText(2, 8); await click('translation.review.bold')
    const first = deferred<ReviewDraft>()
    mock.save.mockImplementationOnce(() => first.promise).mockRejectedValueOnce(Error('approval failed'))
    const approve = button('translation.review.approve')
    await act(async () => { approve.click(); approve.click() })
    expect(mock.save).toHaveBeenCalledTimes(1)
    const next = structuredClone(stored); next.document.expected.revision = 1; next.document.blocks[0].editable = [{ kind: 'bold', range: { start: 2, end: 8 } }]
    await act(async () => first.resolve(next))
    expect(mock.save).toHaveBeenCalledTimes(2)
    expect(mock.save.mock.calls[1][0].expected.revision).toBe(1)
    expect(english().querySelector('strong')?.textContent).toBe('bright')
    stored = structuredClone(next)
    await click('translation.review.approve')
    expect(mock.save.mock.calls[2][0].expected.revision).toBe(1)
    expect(stored.document.blocks[0].approved).toBe(true)
  })

  it('loads without inference capability, handles legacy/null and errors, and discards late old-document results', async () => {
    mock.load.mockRejectedValueOnce(Error('temporary load error'))
    await render()
    expect(container.textContent).toContain('temporary load error')
    await click('translation.review.retry')
    expect(english().textContent).toBe('A bright morning.')
    const old = deferred<ReviewDraft | null>()
    mock.load.mockImplementationOnce(() => old.promise)
    await render({ document: { ...props.document, id: 'other' } })
    expect(container.textContent).toContain('translation.review.loading')
    mock.load.mockResolvedValueOnce(null)
    await render({ document: { ...props.document, id: 'legacy' } })
    expect(container.textContent).toContain('translation.review.unavailable')
    await act(async () => old.resolve(fixture()))
    expect(container.textContent).toContain('translation.review.unavailable')
    expect(container.querySelector('.translation-review-english')).toBeNull()
  })

  it('blocks Finish for dirty drafts, pending approvals and native blockers, then publishes only the expected token once', async () => {
    stored.document.blocks.forEach(block => { block.approved = true })
    stored.blockPreviews[1].blockers = ['note']
    await render()
    expect(button('translation.review.finish').disabled).toBe(true)
    expect(container.textContent).toContain('translation.review.blocker.note')
    await selectText(2, 8); await click('translation.review.bold')
    await click('translation.review.paragraph 2 2')
    expect(container.textContent).toContain('translation.review.dirtyPrompt')
    await click('translation.review.discardContinue')
    expect(english().textContent).toBe('A quiet evening.')
    expect(container.textContent).toContain('translation.review.blocker.note')
    stored.blockPreviews[1].blockers = []; stored.publicationError = 'publication-blocked'
    await click('translation.review.reload')
    expect(container.textContent).toContain('translation.review.publicationBlocked')
    expect(button('translation.review.finish').disabled).toBe(true)
    stored.publicationError = null
    await click('translation.review.reload')
    await selectText(2, 8); await click('translation.review.bold')
    expect(button('translation.review.finish').disabled).toBe(true)
    await click('translation.review.save')
    expect(button('translation.review.finish').disabled).toBe(true)
    await click('translation.review.approve')
    const finished = deferred<typeof props.document>(); mock.finish.mockReturnValueOnce(finished.promise)
    const finish = button('translation.review.finish')
    await act(async () => { finish.click(); finish.click() })
    expect(mock.finish).toHaveBeenCalledExactlyOnceWith(stored.document.expected)
    expect(button('translation.review.close').disabled).toBe(true)
    await act(async () => finished.resolve({ ...props.document, status: 'ready' }))
    expect(props.onPublished).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ status: 'ready' }))
    expect(container.textContent).toContain('translation.review.published')
    expect(button('translation.review.finish').disabled).toBe(true)
    await click('translation.review.close')
    expect(props.onClose).toHaveBeenCalledOnce()
  })

  it('does not publish or set state when Finish completes after the editor unmounts', async () => {
    stored.document.blocks.forEach(block => { block.approved = true })
    await render()
    const finished = deferred<typeof props.document>(); mock.finish.mockReturnValueOnce(finished.promise)
    await click('translation.review.finish')
    await act(async () => root.render(<div>closed</div>))
    await act(async () => finished.resolve({ ...props.document, status: 'ready' }))
    expect(props.onPublished).not.toHaveBeenCalled()
    expect(container.textContent).toBe('closed')
  })
})
