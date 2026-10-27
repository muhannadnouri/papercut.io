import {
  useEffect,
  useState,
} from 'react'
import { useTranslation } from 'react-i18next'
import {
  formatTransferDiagnosticReport,
  getTransferDiagnostics,
  type TransferDiagnosticSnapshot,
} from './libraryTransfer'

export function TransferDiagnostics({ active, revision }: { active: boolean;
  revision: string }) {
  const { t } = useTranslation()
  const [snapshot, setSnapshot] = useState<TransferDiagnosticSnapshot | null>(null)
  const [unavailable, setUnavailable] = useState(false)
  const [retry, setRetry] = useState(0)
  const [copy, setCopy] = useState<'copied' | 'copyFailed' | ''>('')
  useEffect(() => {
    let closed = false
    let pending = false
    const refresh = async () => {
      if (pending) {
        return
      }
      pending = true
      try {
        const next = await getTransferDiagnostics()
        if (!closed) {
          setSnapshot(next)
          setUnavailable(false)
        }
      } catch {
        if (!closed) {
          setUnavailable(true)
        }
      } finally {
        pending = false
      }
    }
    void refresh()
    const timer = active ? window.setInterval(() => {
      void refresh()
    }, 1000) : undefined
    return () => {
      closed = true
      window.clearInterval(timer)
    }
  }, [active, revision, retry])

  // The button shows "Copied" briefly, then returns to "Copy".
  useEffect(() => {
    if (copy !== 'copied') {
      return
    }
    const timer = window.setTimeout(() => setCopy(''), 2000)
    return () => window.clearTimeout(timer)
  }, [copy])

  async function copyReport() {
    if (!snapshot || unavailable) {
      return
    }
    try {
      const latest = await getTransferDiagnostics()
      if (!latest) {
        throw new Error('Diagnostics unavailable')
      }
      await navigator.clipboard.writeText(formatTransferDiagnosticReport(latest))
      setSnapshot(latest)
      setCopy('copied')
    } catch {
      setCopy('copyFailed')
    }
  }

  return <details className="library-transfer-alternate library-transfer-diagnostics">
    <summary>{t('libraryTransfer.diagnostics.showDetails')}</summary>
    {/* Sits across from the summary (CSS), so copying needs no scroll past the events. */}
    <button type="button" className="library-transfer-copy-report" disabled={!snapshot || unavailable}
      aria-label={t('libraryTransfer.diagnostics.copy')} title={t('libraryTransfer.diagnostics.copy')} onClick={() => {
        void copyReport()
      }}>
      <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="M9 9h11v11H9zM15 5V3H3v12h2" /></svg>
      <span aria-hidden="true">{t(copy === 'copied' ? 'libraryTransfer.diagnostics.copied' : 'libraryTransfer.diagnostics.copyShort')}</span>
    </button>
    {/* Success shows on the button; screen readers hear it here. Failure needs the full sentence. */}
    {copy && <p role="status" className={copy === 'copied' ? 'library-transfer-sr-only' : undefined}>{t(`libraryTransfer.diagnostics.${copy}`)}</p>}
    <p>{t('libraryTransfer.diagnostics.privacy')}</p>
    {unavailable && <p role="alert">{t('libraryTransfer.diagnostics.unavailable')} <button type="button"
      onClick={() => setRetry(value => value + 1)}>{t('common.retry')}</button></p>}
    {snapshot && <>
      <p dir="ltr">Papercut {snapshot.appVersion} · {snapshot.os} · {snapshot.arch} · {snapshot.protocol}{snapshot.build && ` · ${snapshot.build}`}</p>
      <p>{t('libraryTransfer.diagnostics.retained', {
        count: snapshot.events.length,
        dropped: snapshot.dropped,
      })}</p>
      {/* Newest first so live activity shows at the top; reversed keeps each event's chronological number. */}
      <ol className="library-transfer-diagnostic-events" dir="ltr" tabIndex={0} aria-label={t('libraryTransfer.diagnostics.events')} reversed>
        {[...snapshot.events].reverse().map((event, index) => <li key={`${event.attempt}:${event.timestampMs}:${index}`}>
          <time>{new Date(event.timestampMs).toISOString()}</time>{' '}
          <strong>{event.level.toUpperCase()}</strong>{' '}
          #{event.attempt} {event.role} +{event.elapsedMs}ms · {event.phase} · {event.code}
          {event.bytes !== null && ` · ${event.bytes}/${event.totalBytes ?? '—'} bytes`}
          {event.items !== null && ` · ${event.items}/${event.totalItems ?? '—'} items`}
          {event.port !== null && ` · ${event.addressScope} · port ${event.port}`}
          {event.requiredBytes !== null && ` · required ${event.requiredBytes} / available ${event.availableBytes ?? '—'} bytes`}
          {event.imported !== null && ` · imported ${event.imported} / skipped ${event.skipped ?? 0} / failed ${event.failed ?? 0}`}
          {event.ioKind && ` · ${event.ioKind}`}{event.osCode !== null && ` · OS ${event.osCode}`}
        </li>)}
      </ol>
    </>}
    {!snapshot && !unavailable && <p>{t('libraryTransfer.diagnostics.loading')}</p>}
  </details>
}
