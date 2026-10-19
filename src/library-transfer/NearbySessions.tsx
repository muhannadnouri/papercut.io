import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { getTransferDiscovery, selectTransferPeer, startTransferDiscovery, stopTransferDiscovery,
  type TransferConnectionMode, type TransferDiscoverySnapshot } from './libraryTransfer'

export function NearbySessions({ interfaceId, mode, disabled, onSelect }: {
  interfaceId: string; mode: TransferConnectionMode; disabled: boolean; onSelect: (address: string, name: string) => void
}) {
  const { t } = useTranslation()
  const [snapshot, setSnapshot] = useState<TransferDiscoverySnapshot | null>(null)
  const [pending, setPending] = useState(false)
  const [failed, setFailed] = useState(false)
  const session = useRef<number | null>(null)
  const alive = useRef(true)
  const generation = useRef(0)
  useEffect(() => {
    alive.current = true
    return () => {
      alive.current = false
      if (session.current !== null) void stopTransferDiscovery(session.current).catch(() => {})
    }
  }, [])
  const id = snapshot?.id
  const watching = snapshot?.state === 'starting' || snapshot?.state === 'searching'
    || (snapshot?.state === 'finished' && snapshot.peers.length > 0)
  useEffect(() => {
    if (id === undefined || !watching) return
    const attempt = generation.current
    let disposed = false, polling = false
    const timer = window.setInterval(() => {
      if (polling) return
      polling = true
      void getTransferDiscovery(id).then(next => {
        if (!disposed && generation.current === attempt) { setSnapshot(next); setFailed(!next) }
      }).catch(() => { if (!disposed && generation.current === attempt) setFailed(true) }).finally(() => { polling = false })
    }, 250)
    return () => { disposed = true; window.clearInterval(timer) }
  }, [id, watching])
  async function browse() {
    if (disabled || pending) return
    generation.current += 1
    setPending(true); setFailed(false)
    try {
      const next = await startTransferDiscovery(interfaceId, mode)
      if (!alive.current) { await stopTransferDiscovery(next.id); return }
      session.current = next.id; setSnapshot(next)
    } catch { if (alive.current) setFailed(true) }
    finally { if (alive.current) setPending(false) }
  }
  async function choose(peer: string) {
    if (!snapshot || disabled || pending) return
    setPending(true); setFailed(false)
    const name = snapshot.peers.find(candidate => candidate.id === peer)?.name ?? ''
    try {
      const address = await selectTransferPeer(snapshot.id, peer)
      if (alive.current) onSelect(address, name)
    } catch { if (alive.current) setFailed(true) }
    finally { if (alive.current) setPending(false) }
  }
  const state = failed ? 'unavailable' : snapshot?.state
  return <section className="library-transfer-nearby" aria-label={t('libraryTransfer.nearby.title')}>
    <h4>{t('libraryTransfer.nearby.title')}</h4>
    <p>{t('libraryTransfer.nearby.help')}</p>
    <button type="button" disabled={disabled || pending || !interfaceId} onClick={() => { void browse() }}>
      {t(snapshot ? 'libraryTransfer.nearby.refresh' : 'libraryTransfer.nearby.find')}
    </button>
    {state && <p role={['denied', 'limited', 'unavailable'].includes(state) ? 'alert' : 'status'} aria-live="polite">
      {t(`libraryTransfer.nearby.${state}`)}
    </p>}
    {snapshot && snapshot.peers.length === 0 && state === 'finished' && <p>{t('libraryTransfer.nearby.empty')}</p>}
    {snapshot && !failed && <ul>{snapshot.peers.map(peer => <li key={peer.id}>
      <button type="button" disabled={disabled || pending} onClick={() => { void choose(peer.id) }}><bdi>{peer.name}</bdi></button>
    </li>)}</ul>}
  </section>
}
