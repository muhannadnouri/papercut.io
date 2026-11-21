import {
  useEffect,
  useEffectEvent,
  useRef,
  useState,
} from 'react'
import {
  listenOpenDocumentRequests,
  takeOpenDocumentSources,
} from '../uploads/DocumentUploads'
import { partitionOpenDocumentSources } from '../tts/api/nativeTts'

/** Retain drained requests across import-status changes; run one consumer at a time. */
export function useOpenDocumentRequests({ busy, onDocuments, onBundle }: {
  busy: boolean
  onDocuments: (sources: string[]) => Promise<void>
  onBundle: (source: string) => Promise<void>
}) {
  const [request, setRequest] = useState(0)
  const pending = useRef<string[]>([])
  const processing = useRef(false)
  const mounted = useRef(false)
  const importDocuments = useEffectEvent(onDocuments)
  const importBundle = useEffectEvent(onBundle)

  useEffect(() => {
    if (!('__TAURI_INTERNALS__' in window)) {
      return
    }
    mounted.current = true
    let disposed = false
    let unlisten: (() => void) | undefined
    void listenOpenDocumentRequests(() => setRequest(value => value + 1)).then(stop => {
      if (disposed) {
        stop()
      } else {
        unlisten = stop
        setRequest(value => value + 1)
      }
    }).catch(error => console.warn('Unable to listen for document open requests:', error))
    return () => {
      disposed = true
      mounted.current = false
      unlisten?.()
    }
  }, [])

  useEffect(() => {
    if (request === 0 || busy || processing.current) {
      return
    }
    processing.current = true
    let disposed = false
    let hasSources = false
    void (async () => {
      const sources = pending.current.splice(0)
      if (sources.length === 0) {
        sources.push(...await takeOpenDocumentSources())
      }
      hasSources = sources.length > 0
      if (disposed) {
        pending.current.unshift(...sources)
        return
      }
      if (sources.length === 0) {
        return
      }
      const { documents, bundles } = await partitionOpenDocumentSources(sources)
      if (disposed) {
        pending.current.unshift(...sources)
        return
      }
      pending.current.push(...bundles)
      if (documents.length > 0) {
        await importDocuments(documents)
      }
      while (!disposed && pending.current.length > 0) {
        await importBundle(pending.current.shift()!)
      }
    })().catch(error => console.warn('Unable to import document open request:', error)).finally(() => {
      processing.current = false
      if (mounted.current && (hasSources || disposed || pending.current.length > 0)) {
        setRequest(value => value + 1)
      }
    })
    return () => {
      disposed = true
    }
  }, [busy, request])
}
