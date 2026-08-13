import { describe, expect, it } from 'vitest'
import { resolveUploadedDocumentAssets } from './DocumentUploads'

describe('resolveUploadedDocumentAssets', () => {
  it('resolves only declared generated image names through the asset protocol', () => {
    const fileName = `image-${'a'.repeat(64)}.png`
    const html = `<img data-papercut-asset="${fileName}" loading="lazy"><img data-papercut-asset="image-${'b'.repeat(64)}.png">`

    const resolved = resolveUploadedDocumentAssets(
      { html, assetPaths: { [fileName]: '/app/assets/cover.png' } },
      (path) => `asset://localhost/${path}?x=1&y="2"`,
    )

    expect(resolved).toContain('src="asset://localhost//app/assets/cover.png?x=1&amp;y=&quot;2&quot;"')
    expect(resolved.match(/src=/g)).toHaveLength(1)
  })
})
