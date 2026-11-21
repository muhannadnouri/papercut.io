import {
  describe,
  expect,
  it,
  vi,
} from 'vitest'
import * as core from '@tauri-apps/api/core'
import {
  getUploadedReaderDocument,
  getUploadedDocumentSource,
  resolveUploadedDocumentAssets,
} from './DocumentUploads'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  convertFileSrc: vi.fn(),
}))

describe('resolveUploadedDocumentAssets', () => {
  it.each(['png', 'jpg', 'gif', 'webp', 'svg'])('resolves only declared generated %s names through the asset protocol', (extension) => {
    const fileName = `image-${'a'.repeat(64)}.${extension}`
    const html = `<img data-papercut-asset="${fileName}" loading="lazy"><img data-papercut-asset="image-${'b'.repeat(64)}.${extension}"><img data-papercut-asset="../unsafe.svg">`

    const resolved = resolveUploadedDocumentAssets(
      {
        html,
        assetPaths: {
          [fileName]: `/app/assets/${fileName}`,
          '../unsafe.svg': '/unsafe.svg',
        },
      },
      (path) => `asset://localhost/${path}?x=1&y="2"`,
    )

    expect(resolved).toContain(`src="asset://localhost//app/assets/${fileName}?x=1&amp;y=&quot;2&quot;"`)
    expect(resolved.match(/src=/g)).toHaveLength(1)
  })
})

describe('uploaded reader source', () => {
  it('requests the outline for the reader while keeping HTML-only callers on the string contract', async () => {
    const outline = [{
      label: 'Contents',
      target: 'chapter-0',
      children: [],
    }]
    const invoke = vi.mocked(core.invoke)
    invoke.mockResolvedValueOnce({
      html: '<p>Reader</p>',
      assetPaths: {},
      outline,
    })
    invoke.mockResolvedValueOnce({
      html: '<p>Source</p>',
      assetPaths: {},
    })
    const url = '/uploads/book/source.html'
    await expect(getUploadedReaderDocument(url)).resolves.toEqual({
      html: '<p>Reader</p>',
      outline,
    })
    await expect(getUploadedDocumentSource(url)).resolves.toBe('<p>Source</p>')
    expect(invoke.mock.calls).toEqual([
      ['document_uploads_get_source', {
        request: { documentUrl: url },
        includeOutline: true,
      }],
      ['document_uploads_get_source', {
        request: { documentUrl: url },
        includeOutline: false,
      }],
    ])
  })
})
