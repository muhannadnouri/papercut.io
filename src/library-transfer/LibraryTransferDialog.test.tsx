import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import { LibrarySendFlow, TransferPreparationMessage } from './LibraryTransferDialog'

vi.mock('react-i18next', async (importOriginal) => ({
  ...await importOriginal<typeof import('react-i18next')>(),
  useTranslation: () => ({ t: (key: string) => key }),
}))

vi.mock('../i18n', () => ({
  currentAppLocale: () => 'en',
  default: {},
}))

describe('library transfer send flow', () => {
  it('announces indeterminate package preparation', () => {
    const html = renderToStaticMarkup(<TransferPreparationMessage />)

    expect(html).toContain('role="status"')
    expect(html).toContain('aria-live="polite"')
    expect(html).toContain('class="spinner"')
    expect(html).toContain('libraryTransfer.preparingSelection')
  })

  it('prioritizes pairing instructions while waiting for the receiving device', () => {
    const html = renderToStaticMarkup(
      <LibrarySendFlow
        preparing={false}
        locale="en"
        onExit={() => undefined}
        status={{
          state: 'waiting',
          address: '192.168.1.20:49152',
          code: 'ABCD-EFGH',
          documents: 2,
          audiobooks: 1,
          packageBytes: 4096,
          bytesTransferred: 0,
        }}
      />,
    )

    expect(html).toContain('<ol>')
    expect(html).toContain('libraryTransfer.connectStepOpen')
    expect(html).toContain('libraryTransfer.connectStepEnter')
    expect(html).toContain('192.168.1.20:49152')
    expect(html).toContain('ABCD-EFGH')
    expect(html).toContain('libraryTransfer.changeSelection')
  })
})
