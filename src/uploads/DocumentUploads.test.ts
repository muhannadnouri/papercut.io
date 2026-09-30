import { describe, expect, it } from 'vitest'
import { resolveUploadedDocumentAssets } from './DocumentUploads'

describe('resolveUploadedDocumentAssets', () => {
  it.each(['png', 'jpg', 'gif', 'webp', 'svg'])('resolves only declared generated %s names through the asset protocol', (extension) => {
    const fileName = `image-${'a'.repeat(64)}.${extension}`
    const html = `<img data-papercut-asset="${fileName}" loading="lazy"><img data-papercut-asset="image-${'b'.repeat(64)}.${extension}"><img data-papercut-asset="../unsafe.svg">`

    const resolved = resolveUploadedDocumentAssets(
      { html, assetPaths: { [fileName]: `/app/assets/${fileName}`, '../unsafe.svg': '/unsafe.svg' } },
      (path) => `asset://localhost/${path}?x=1&y="2"`,
    )

    expect(resolved).toContain(`src="asset://localhost//app/assets/${fileName}?x=1&amp;y=&quot;2&quot;"`)
    expect(resolved.match(/src=/g)).toHaveLength(1)
  })
})
