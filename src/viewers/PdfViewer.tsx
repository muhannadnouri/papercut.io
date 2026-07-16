import type { ViewerProps } from './types'

// Stub — implement with pdf.js or similar when PDF support is needed.
// Receives `url` (the PDF URL), `onLoad`, and optional scroll/zoom callbacks via ViewerProps.
export function PdfViewer({ url }: ViewerProps) {
  return (
    <div className="viewer-stub">
      <p>PDF viewer not yet implemented.</p>
      <p><code dir="ltr">{url}</code></p>
    </div>
  )
}
