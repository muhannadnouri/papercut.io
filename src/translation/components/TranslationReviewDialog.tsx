import { createElement, useCallback, useEffect, useEffectEvent, useId, useMemo, useRef, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { AppDialog } from '../../components/AppDialog/AppDialog'
import { finishTranslationReview, loadTranslationReview, saveTranslationReview, TranslationError, type TranslatedDocumentInfo } from '../api/nativeTranslation'
import { markedRuns, markState, rangeAllowed, selectionFromTextarea, selectionRange, toggleMark } from '../review/selection'
import type { Mark, MarkKind, ReviewDraft, ScalarRange } from '../review/types'
import './TranslationReviewDialog.css'

interface Props {
  document: TranslatedDocumentInfo
  onClose: () => void
  onPublished: (document: TranslatedDocumentInfo) => void | Promise<void>
}
type Destination = { index: number } | 'close' | 'reload'
type LocalEdit = { marks: Mark[]; reset: boolean }
const blockerKeys = {
  typography: 'translation.review.blocker.typography', link: 'translation.review.blocker.link', note: 'translation.review.blocker.note',
  attachment: 'translation.review.blocker.attachment', sourceFormatting: 'translation.review.blocker.sourceFormatting',
  sourceOnlyLiteral: 'translation.review.blocker.sourceOnlyLiteral', sourceOnlyStructure: 'translation.review.blocker.sourceOnlyStructure',
} as const
const sameMarks = (left: Mark[], right: Mark[]) => JSON.stringify(left) === JSON.stringify(right)

/** Edit only emphasis coordinates; native evidence and unchanged English remain the publication authority. */
export function TranslationReviewDialog({ document: info, onClose, onPublished }: Props) {
  const { t } = useTranslation()
  const [draft, setDraft] = useState<ReviewDraft | null>(null)
  const [loading, setLoading] = useState(true)
  const [busy, setBusy] = useState(false)
  const [index, setIndex] = useState(0)
  const [local, setLocal] = useState<LocalEdit>({ marks: [], reset: false })
  const [history, setHistory] = useState<LocalEdit[]>([])
  const [selection, setSelection] = useState<ScalarRange | null>(null)
  const [invalidSelection, setInvalidSelection] = useState(false)
  const [error, setError] = useState('')
  const [conflict, setConflict] = useState(false)
  const [message, setMessage] = useState('')
  const [confirm, setConfirm] = useState<Destination | null>(null)
  const [published, setPublished] = useState(false)
  const [finishing, setFinishing] = useState(false)
  const english = useRef<HTMLDivElement>(null)
  const toolbar = useRef<HTMLDivElement>(null)
  const keyboardText = useRef<HTMLTextAreaElement>(null)
  const confirmationHeading = useRef<HTMLHeadingElement>(null)
  const wasConfirming = useRef(false)
  const busyRef = useRef(false)
  const sequence = useRef(0)
  const alive = useRef(false)
  const descriptionId = useId()
  const confirmId = useId()
  const block = draft?.document.blocks[index]
  const dirty = Boolean(block && (local.reset || !sameMarks(local.marks, block.editable)))
  const blockPreview = draft?.blockPreviews[index]
  const blockers = blockPreview?.blockers ?? []
  const approved = draft?.document.blocks.filter(row => row.approved).length ?? 0
  const total = draft?.document.blocks.length ?? 0
  const pending = total - approved
  const firstBlocked = draft?.blockPreviews.findIndex(row => row.blockers.length > 0) ?? -1
  const publicationBlocked = Boolean(draft?.publicationError || draft?.blockPreviews.some(row => row.blockers.length))
  const selectionSupported = typeof Intl.Segmenter === 'function'
  const usableSelection = Boolean(selectionSupported && block && selection && rangeAllowed(block.text, selection, block.locked))
  const selectedText = block && selection ? Array.from(block.text).slice(selection.start, selection.end).join('') : ''
  // Ancestor emphasis is readonly presentation, not a change to saved evidence or editable marks.
  const baselineMarks = useMemo(() => [...(block?.fixed ?? []), ...(blockPreview?.inheritedMarks ?? [])], [block, blockPreview])
  const runs = useMemo(() => block ? markedRuns(block.text, [...baselineMarks, ...local.marks]) : [], [block, baselineMarks, local.marks])
  const source = useMemo(() => inertSource(blockPreview?.sourceHtml ?? ''), [blockPreview?.sourceHtml])

  // A synchronous guard also covers two click events before React paints the disabled button.
  function startRequest() {
    if (busyRef.current) return null
    busyRef.current = true
    setBusy(true)
    return sequence.current
  }
  function active(token: number) { return alive.current && token === sequence.current }
  function endRequest(token: number) {
    if (active(token)) { busyRef.current = false; setBusy(false) }
  }
  function receive(next: ReviewDraft, position: number) {
    setDraft(next)
    const current = Math.min(position, Math.max(0, next.document.blocks.length - 1))
    setIndex(current)
    setLocal({ marks: next.document.blocks[current]?.editable ?? [], reset: false })
    setHistory([])
  }
  function failure(reason: unknown) {
    const stale = reason instanceof TranslationError && ['review-conflict', 'review-unavailable'].includes(reason.code)
    setConflict(stale)
    setError(stale ? t('translation.review.conflict') : reason instanceof Error ? reason.message : t('translation.review.actionFailed'))
  }
  async function load() {
    if (busyRef.current) return
    busyRef.current = true
    const token = ++sequence.current
    setLoading(true); setBusy(true); setError(''); setMessage('')
    try {
      const next = await loadTranslationReview(info.id)
      if (!alive.current || token !== sequence.current) return
      if (next) receive(next, 0)
      else setDraft(null)
      setSelection(null); setInvalidSelection(false); setPublished(false); setConflict(false)
    } catch (reason) {
      if (alive.current && token === sequence.current) setError(reason instanceof Error ? reason.message : t('translation.review.loadFailed'))
    } finally {
      if (alive.current && token === sequence.current) { busyRef.current = false; setLoading(false); setBusy(false) }
    }
  }

  // Locale/callback changes must not reopen the document and discard local edits.
  const loadOnOpen = useEffectEvent(() => { void load() })
  useEffect(() => {
    alive.current = true
    busyRef.current = false
    setDraft(null)
    loadOnOpen()
    return () => { alive.current = false; sequence.current += 1; busyRef.current = false }
  }, [info.id])
  useEffect(() => {
    if (confirm) confirmationHeading.current?.focus()
    else if (wasConfirming.current) english.current?.focus()
    wasConfirming.current = Boolean(confirm)
  }, [confirm])
  useEffect(() => {
    if (!dirty) return
    const protect = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = '' }
    window.addEventListener('beforeunload', protect)
    return () => window.removeEventListener('beforeunload', protect)
  }, [dirty])

  /** Keep logical scalar coordinates when a toolbar button takes focus from a real DOM selection. */
  const captureSelection = useCallback((preserveCollapsed = false) => {
    if (!english.current || !block) return
    const selected = window.getSelection()
    const controlFocused = toolbar.current?.contains(document.activeElement) || document.activeElement === keyboardText.current
    if (preserveCollapsed && controlFocused && (!selected || selected.isCollapsed || document.activeElement === keyboardText.current)) return
    if (!selectionSupported || !selected || !english.current.contains(selected.anchorNode) || !english.current.contains(selected.focusNode)) {
      setSelection(null); setInvalidSelection(false); return
    }
    const range = selectionRange(english.current, block.text, selected)
    setSelection(range)
    setInvalidSelection(!selected.isCollapsed && (!range || !rangeAllowed(block.text, range, block.locked)))
  }, [block, selectionSupported])
  useEffect(() => {
    // Replacing preview runs can collapse the browser range; only a direct user selection clears the logical range.
    const changed = () => captureSelection(true)
    document.addEventListener('selectionchange', changed)
    return () => document.removeEventListener('selectionchange', changed)
  }, [captureSelection])

  /** Readonly text controls expose genuine keyboard selection offsets even without browser caret browsing. */
  function captureTextSelection(target: HTMLTextAreaElement) {
    if (!block || !selectionSupported) return
    const range = selectionFromTextarea(block.text, target.selectionStart, target.selectionEnd)
    setSelection(range)
    setInvalidSelection(target.selectionStart !== target.selectionEnd && (!range || !rangeAllowed(block.text, range, block.locked)))
  }
  function changeMarks(next: LocalEdit) {
    if (busyRef.current || confirm || !block || conflict || published) return
    setHistory(previous => [...previous.slice(-31), local])
    setLocal(next); setMessage(''); setError('')
  }
  function toggle(kind: MarkKind) {
    if (!block || !selection || !usableSelection) return
    try { changeMarks({ marks: toggleMark(local.marks, selection, kind, block.locked), reset: local.reset }) }
    catch { setError(t('translation.review.invalidSelection')) }
  }
  function go(destination: Destination) {
    setConfirm(null)
    if (destination === 'close') { onClose(); return }
    if (destination === 'reload') { void load(); return }
    if (!draft) return
    setIndex(destination.index)
    setLocal({ marks: draft.document.blocks[destination.index].editable, reset: false })
    setSelection(null); setInvalidSelection(false); setHistory([]); setMessage(''); setError('')
  }
  function request(destination: Destination) {
    if (busyRef.current || confirm) return
    if (dirty) setConfirm(destination)
    else go(destination)
  }

  /** If approval fails after a successful save, retain its new revision rather than resending a stale token. */
  async function save(approve: boolean, destination?: Destination) {
    if (!draft || !block || conflict || published) return
    const token = startRequest()
    if (token === null) return
    setError(''); setMessage('')
    let next = draft
    try {
      if (dirty) {
        next = await saveTranslationReview({ expected: next.document.expected, block: block.key, edit: { action: 'replaceMarks', marks: local.marks } })
        if (!active(token)) return
        receive(next, index)
      }
      if (approve) {
        next = await saveTranslationReview({ expected: next.document.expected, block: block.key, edit: { action: 'approve' } })
        if (!active(token)) return
        receive(next, index)
      }
      if (!active(token)) return
      setMessage(t(approve ? 'translation.review.approved' : 'translation.review.saved'))
      if (destination) {
        setConfirm(null)
        if (destination === 'close') onClose()
        else if (destination === 'reload') { endRequest(token); void load() }
        else {
          setIndex(destination.index)
          setLocal({ marks: next.document.blocks[destination.index].editable, reset: false })
          setSelection(null); setInvalidSelection(false); setHistory([]); setMessage('')
        }
      }
    } catch (reason) { if (active(token)) { failure(reason); setConfirm(null) } }
    finally { endRequest(token) }
  }

  /** Finish sends no HTML or approval claims; native preflight and atomic publication run again. */
  async function finish() {
    if (!draft || dirty || pending || publicationBlocked || conflict || published) return
    const token = startRequest()
    if (token === null) return
    setError(''); setMessage(''); setFinishing(true)
    try {
      const result = await finishTranslationReview(draft.document.expected)
      if (!active(token)) return
      setPublished(true); setMessage(t('translation.review.published'))
      await onPublished(result)
    } catch (reason) { if (active(token)) failure(reason) }
    finally { if (active(token)) setFinishing(false); endRequest(token) }
  }

  const controlsDisabled = busy || conflict || published || Boolean(confirm)
  return <AppDialog title={t('translation.review.title')} description={info.title}
    className="translation-review-dialog" showCloseButton navigationDisabled={busy}
    onCancel={() => request('close')} actions={<>
      <button type="button" className="app-dialog-cancel" disabled={busy} onClick={() => request('close')}>{t('translation.review.close')}</button>
      {draft && <button type="button" className="app-dialog-submit" disabled={controlsDisabled || dirty || pending > 0 || publicationBlocked}
        onClick={() => { void finish() }}>{t(finishing ? 'translation.review.finishing' : 'translation.review.finish')}</button>}
    </>}>
    <p className="translation-review-description">{t('translation.review.description')}</p>
    {loading ? <p role="status">{t('translation.review.loading')}</p> : <>
      {error && <div className="translation-review-error" role="alert"><p>{error}</p>
        {!draft && <button type="button" disabled={busy} onClick={() => request('reload')}>{t('translation.review.retry')}</button>}
      </div>}
      {!draft && !error && <p>{t('translation.review.unavailable')}</p>}
      {message && <p role="status">{message}</p>}
      {busy && <p role="status">{t(finishing ? 'translation.review.finishing' : 'translation.review.saving')}</p>}
      {draft && block && <>
        {confirm !== null && <section className="translation-review-confirm" aria-labelledby={confirmId}>
          <h3 id={confirmId} ref={confirmationHeading} tabIndex={-1}>{t(confirm === 'reload' ? 'translation.review.reloadPrompt' : 'translation.review.dirtyPrompt')}</h3>
          <div className="translation-review-buttons">
            {confirm !== 'reload' && <button type="button" disabled={busy || conflict} onClick={() => { void save(false, confirm) }}>{t('translation.review.saveContinue')}</button>}
            <button type="button" disabled={busy} onClick={() => go(confirm)}>{t('translation.review.discardContinue')}</button>
            <button type="button" disabled={busy} onClick={() => setConfirm(null)}>{t('translation.review.stay')}</button>
          </div>
        </section>}
        <div className="translation-review-content" inert={Boolean(confirm)}>
          <p role="status">{t('translation.review.progress', { approved, total })}{dirty ? ` · ${t('translation.review.unsaved')}` : ''}</p>
          <button type="button" disabled={busy} onClick={() => request('reload')}>{t('translation.review.reload')}</button>
          {firstBlocked >= 0 && firstBlocked !== index && <aside className="translation-review-blockers">
            <h3>{t('translation.review.blockerTitle')}</h3>
            <ul>{draft.blockPreviews[firstBlocked].blockers.map(code => <li key={code}>{t(blockerKeys[code as keyof typeof blockerKeys] ?? 'translation.review.publicationInvalid')}</li>)}</ul>
            <button type="button" disabled={busy} onClick={() => request({ index: firstBlocked })}>{t('translation.review.paragraph', { index: firstBlocked + 1, total })}</button>
          </aside>}
          <nav className="translation-review-buttons" aria-label={t('translation.review.paragraphPicker')}>
            <button type="button" disabled={busy || index === 0} onClick={() => request({ index: index - 1 })}>{t('translation.review.previous')}</button>
            <label>{t('translation.review.paragraphPicker')} <select value={index} disabled={busy} onChange={event => request({ index: Number(event.target.value) })}>
              {draft.document.blocks.map((row, position) => <option value={position} key={row.key.ordinal}>{t('translation.review.paragraph', { index: position + 1, total })}{row.approved ? ' ✓' : ''}</option>)}
            </select></label>
            <button type="button" disabled={busy || index === total - 1} onClick={() => request({ index: index + 1 })}>{t('translation.review.next')}</button>
          </nav>
          <details key={block.key.ordinal} className="translation-review-source"><summary>{t('translation.review.original')}</summary><div lang={info.sourceLanguage} dir="auto">{source}</div></details>
          <section aria-label={t('translation.review.english')}>
            <h3>{t('translation.review.english')}</h3>
            <p id={descriptionId}>{t('translation.review.selectHint')}</p>
            <div ref={english} className="translation-review-english" lang="en" dir="ltr" role="region" aria-label={t('translation.review.english')} aria-describedby={descriptionId} tabIndex={0}
              onMouseUp={() => captureSelection()} onTouchEnd={() => captureSelection()} onKeyUp={() => captureSelection()}>{runs.map((run, at) => <TextRun key={at} {...run} />)}</div>
          </section>
          <details className="translation-review-keyboard" key={`keyboard-${block.key.ordinal}`}><summary>{t('translation.review.keyboardSelect')}</summary>
            <label>{t('translation.review.keyboardTextHint')}<textarea ref={keyboardText} lang="en" dir="ltr" readOnly value={block.text} disabled={busy || Boolean(confirm)}
              aria-label={t('translation.review.keyboardSelect')} aria-describedby={descriptionId}
              onSelect={event => captureTextSelection(event.currentTarget)} onKeyUp={event => captureTextSelection(event.currentTarget)}
              onMouseUp={event => captureTextSelection(event.currentTarget)} onTouchEnd={event => captureTextSelection(event.currentTarget)} /></label>
          </details>
          {!selectionSupported && <p role="status">{t('translation.review.selectionUnsupported')}</p>}
          <p className="translation-review-selection" role="status">{invalidSelection ? t('translation.review.invalidSelection') : selectedText ? t('translation.review.selected', { text: selectedText }) : t('translation.review.lockedHint')}</p>
          <div ref={toolbar} className="translation-review-buttons" role="group" aria-label={t('translation.review.applied')}>
            {(['bold', 'italic'] as const).map(kind => <button key={kind} type="button" disabled={controlsDisabled || !usableSelection || Boolean(selection && markState(baselineMarks, selection, kind) === true)}
              aria-pressed={selection ? markState([...baselineMarks, ...local.marks], selection, kind) : false} onClick={() => toggle(kind)}>{t(`translation.review.${kind}`)}</button>)}
            <button type="button" disabled={controlsDisabled || !history.length} onClick={() => {
              const previous = history.at(-1)
              if (previous) { setLocal(previous); setHistory(history.slice(0, -1)); setMessage('') }
            }}>{t('translation.review.undo')}</button>
            <button type="button" disabled={controlsDisabled} onClick={() => changeMarks({ marks: [], reset: true })}>{t('translation.review.reset')}</button>
          </div>
          <div className="translation-review-marks">
            <h3>{t('translation.review.applied')}</h3>
            {baselineMarks.length > 0 && <p>{t('translation.review.baselineHint')}</p>}
            {!local.marks.length ? <p>{t('translation.review.noMarks')}</p> : <ul>{local.marks.map((mark, at) => {
              const kind = t(`translation.review.${mark.kind}`)
              const text = Array.from(block.text).slice(mark.range.start, mark.range.end).join('')
              return <li key={`${mark.kind}-${mark.range.start}-${mark.range.end}-${at}`}><span>{t('translation.review.markLabel', { kind, text })}</span>
                <button type="button" disabled={controlsDisabled} aria-label={t('translation.review.removeMark', { kind, text })}
                  onClick={() => changeMarks({ marks: local.marks.filter((_, position) => position !== at), reset: local.reset })}>{t('translation.review.removeMark', { kind, text })}</button></li>
            })}</ul>}
          </div>
          {blockers.length > 0 && <div className="translation-review-blockers"><h3>{t('translation.review.blockerTitle')}</h3><ul>{blockers.map(code => <li key={code}>{t(blockerKeys[code as keyof typeof blockerKeys] ?? 'translation.review.publicationInvalid')}</li>)}</ul></div>}
          <p>{t('translation.review.approvalHint')}</p>
          <div className="translation-review-buttons">
            <button type="button" disabled={controlsDisabled || !dirty} onClick={() => { void save(false) }}>{t('translation.review.save')}</button>
            <button type="button" disabled={controlsDisabled || (block.approved && !dirty)} onClick={() => { void save(true) }}>{t(block.approved && !dirty ? 'translation.review.approved' : 'translation.review.approve')}</button>
          </div>
          {draft.publicationError && <p className="translation-review-error">{t(draft.publicationError === 'publication-blocked' ? 'translation.review.publicationBlocked' : 'translation.review.publicationInvalid')}</p>}
          <p>{pending > 0 || publicationBlocked ? t('translation.review.pending', { count: pending }) : t('translation.review.finishHint')}</p>
        </div>
      </>}
    </>}
  </AppDialog>
}

function TextRun({ text, bold, italic }: { text: string; bold: boolean; italic: boolean }) {
  let result: ReactNode = text
  if (italic) result = <em>{result}</em>
  if (bold) result = <strong>{result}</strong>
  return result
}

/** Native HTML remains evidence only: render semantic comparison without URLs, IDs, CSS or active resources. */
function inertSource(html: string): ReactNode[] {
  const source = document.createElement('template')
  source.innerHTML = html
  const allowed = new Set(['p', 'div', 'span', 'strong', 'b', 'em', 'i', 'u', 's', 'sub', 'sup', 'code', 'pre', 'kbd', 'samp', 'small', 'mark', 'br', 'blockquote', 'ul', 'ol', 'li', 'table', 'thead', 'tbody', 'tr', 'th', 'td', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6'])
  const dropped = new Set(['script', 'style', 'iframe', 'object', 'embed', 'form', 'input', 'textarea', 'select', 'button', 'svg', 'math', 'link', 'meta', 'base'])
  const visit = (node: Node, key: string): ReactNode => {
    if (node.nodeType === Node.TEXT_NODE) return node.textContent
    if (!(node instanceof Element) || dropped.has(node.localName)) return null
    if (node.localName === 'img') return node.getAttribute('alt') ?? null
    const children = Array.from(node.childNodes, (child, at) => visit(child, `${key}-${at}`))
    return createElement(allowed.has(node.localName) ? node.localName : 'span', { key }, node.localName === 'br' ? undefined : children)
  }
  return Array.from(source.content.childNodes, (node, index) => visit(node, String(index)))
}
