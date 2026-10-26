import type { ViewerPlugin } from './types'
import { HtmlViewer } from './HtmlViewer'
import { PdfViewer } from './PdfViewer'
import { EpubViewer } from './EpubViewer'

// HTML is the catch-all fallback (canHandle always true).
const htmlPlugin: ViewerPlugin = {
  id: 'html',
  canHandle: () => true,
  Component: HtmlViewer,
}

// Order matters: more specific formats before the HTML fallback.
const viewerPlugins: ViewerPlugin[] = [
  {
    id: 'pdf',
    canHandle: (url) => /\.pdf$/i.test(url),
    Component: PdfViewer,
  },
  {
    id: 'epub',
    canHandle: (url) => /\.epub$/i.test(url),
    Component: EpubViewer,
  },
  htmlPlugin,
]

export function resolveViewer(url: string, format?: string): ViewerPlugin {
  if (format === 'epub' && /\.html(?:[#?].*)?$/i.test(url)) {
    return htmlPlugin
  }
  return viewerPlugins.find((p) => p.canHandle(url)) ?? htmlPlugin
}
