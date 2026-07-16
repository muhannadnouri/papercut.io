import type { ViewerProps } from './types'

// Raw EPUB rendering is reserved for a richer future viewer. Uploaded EPUBs open through generated reading HTML.
export function EpubViewer({ url }: ViewerProps) {
  return (
    <div className="viewer-stub">
      <p>Raw EPUB viewer not yet implemented.</p>
      <p><code dir="ltr">{url}</code></p>
    </div>
  )
}
