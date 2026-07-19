import { memo, useMemo } from 'react'
import type { ViewerProps } from './types'

export const HtmlViewer = memo(function HtmlViewer({ content, contentRef }: ViewerProps) {
  const document = useMemo(() => parseHtmlDocument(content ?? ''), [content])

  return (
    <article
      ref={contentRef}
      className="document-html-surface"
      lang={document.language}
      dir={document.direction}
    >
      <div className="document-html-content" dangerouslySetInnerHTML={{ __html: document.bodyHtml }} />
    </article>
  )
})

// Uploaded sources are stored as complete sanitized HTML documents. The app-owned
// surface renders only body content so imported head styles cannot leak globally,
// while root metadata is copied onto that surface instead of the app shell.
function parseHtmlDocument(content: string): {
  bodyHtml: string
  language?: string
  direction: 'ltr' | 'rtl' | 'auto'
} {
  if (typeof DOMParser === 'undefined') {
    return { bodyHtml: content, direction: 'auto' }
  }

  const doc = new DOMParser().parseFromString(content, 'text/html')
  const language = validLanguage(
    doc.body?.getAttribute('lang') ??
    doc.body?.getAttribute('xml:lang') ??
    doc.documentElement.getAttribute('lang') ??
    doc.documentElement.getAttribute('xml:lang'),
  )
  const direction = validDirection(
    doc.body?.getAttribute('dir') ?? doc.documentElement.getAttribute('dir'),
  )
  return {
    bodyHtml: doc.body?.innerHTML || content,
    language,
    direction: direction ?? 'auto',
  }
}

function validLanguage(value: string | null): string | undefined {
  const language = value?.trim()
  return language && language.length <= 64 && /^[a-z0-9-]+$/i.test(language)
    ? language
    : undefined
}

function validDirection(value: string | null): 'ltr' | 'rtl' | 'auto' | undefined {
  const direction = value?.trim().toLowerCase()
  return direction === 'ltr' || direction === 'rtl' || direction === 'auto'
    ? direction
    : undefined
}
