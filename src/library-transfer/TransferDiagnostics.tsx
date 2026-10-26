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
          setSnapshot(next); setUnavailable(false)
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
      closed = true; window.clearInterval(timer)
    }
  }, [active, revision, retry])

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

  return <details className="library-transfer-diagnostics">
    <summary>{t('libraryTransfer.diagnostics.showDetails')}</summary>
    <p>{t('libraryTransfer.diagnostics.privacy')}</p>
    {unavailable && <p role="alert">{t('libraryTransfer.diagnostics.unavailable')} <button type="button"
      onClick={() => setRetry(value => value + 1)}>{t('common.retry')}</button></p>}
    {snapshot && <>
      <p dir="ltr">Papercut {snapshot.appVersion} · {snapshot.os} · {snapshot.arch} · {snapshot.protocol}{snapshot.build && ` · ${snapshot.build}`}</p>
      <p>{t('libraryTransfer.diagnostics.retained', {
        count: snapshot.events.length,
        dropped: snapshot.dropped,
      })}</p>
      <ol className="library-transfer-diagnostic-events" dir="ltr" tabIndex={0} aria-label={t('libraryTransfer.diagnostics.events')}>
        {snapshot.events.map((event, index) => <li key={`${event.attempt}:${event.timestampMs}:${index}`}>
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
    <button type="button" className="library-transfer-copy-report" disabled={!snapshot || unavailable} onClick={() => {
      void copyReport()
    }}>
      <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="M9 9h11v11H9zM15 5V3H3v12h2" /></svg>
      {t('libraryTransfer.diagnostics.copy')}
    </button>
    {copy && <p role="status">{t(`libraryTransfer.diagnostics.${copy}`)}</p>}
  </details>
}
