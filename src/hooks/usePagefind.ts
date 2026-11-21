import {
  useState,
  useEffect,
  useRef,
} from 'react'
import type {
  PagefindInstance,
  DocumentInfo,
} from '../types/search'

interface UsePagefindReturn {
  pagefindRef: React.MutableRefObject<PagefindInstance | null>
  pagefindReady: boolean
  allDocuments: DocumentInfo[]
  documentsLoading: boolean
}

export function usePagefind(): UsePagefindReturn {
  const pagefindRef = useRef<PagefindInstance | null>(null)
  const [pagefindReady, setPagefindReady] = useState(false)
  const [allDocuments, setAllDocuments] = useState<DocumentInfo[]>([])
  const [documentsLoading, setDocumentsLoading] = useState(true)

  useEffect(() => {
    async function loadPagefind() {
      try {
        const pagefindPath = '/pagefind/pagefind.js'
        const pagefind = await import(/* @vite-ignore */ pagefindPath) as unknown as PagefindInstance
        pagefindRef.current = pagefind
        setPagefindReady(true)

        const allResults = await pagefind.search('')
        if (allResults.results.length === 0) {
          const fallback = await pagefind.search('a')
          const docs = await Promise.all(fallback.results.map((r) => r.data()))
          setAllDocuments(docs.map((d) => ({
            title: d.meta.title,
            url: d.url,
          })))
        } else {
          const docs = await Promise.all(allResults.results.map((r) => r.data()))
          setAllDocuments(docs.map((d) => ({
            title: d.meta.title,
            url: d.url,
          })))
        }
      } catch {
        console.error('Pagefind index not found. Run `npm run build` first to generate the index.')
      } finally {
        setDocumentsLoading(false)
      }
    }
    loadPagefind()
  }, [])

  return {
    pagefindRef,
    pagefindReady,
    allDocuments,
    documentsLoading,
  }
}
