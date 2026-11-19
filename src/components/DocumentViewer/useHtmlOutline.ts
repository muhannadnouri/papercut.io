import {
  useCallback,
  useState,
} from 'react'
import type { OutlineEntry } from '../../viewers/types'
import { buildHtmlOutline } from './htmlOutline'

/** The article DOM is ready after commit. During a switch the previous article's
 * result can still be in state, so expose it only for the source that produced it. */
export function useHtmlOutline(uploadUrl: string | undefined, content: string) {
  const [source, setSource] = useState<{
    url: string
    content: string
    entries: OutlineEntry[]
  } | null>(null)

  const collect = useCallback((body: HTMLElement | null) => {
    setSource(uploadUrl && body ? {
      url: uploadUrl,
      content,
      entries: buildHtmlOutline(body),
    } : null)
  }, [uploadUrl, content])

  return {
    bodyRef: uploadUrl ? collect : undefined,
    entries: source && source.url === uploadUrl && source.content === content ? source.entries : undefined,
  }
}
