import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import { TransferPreparationMessage } from './LibraryTransferDialog'

vi.mock('react-i18next', async (importOriginal) => ({
  ...await importOriginal<typeof import('react-i18next')>(),
  useTranslation: () => ({ t: (key: string) => key }),
}))

vi.mock('../i18n', () => ({
  currentAppLocale: () => 'en',
  default: {},
}))

describe('library transfer preparation', () => {
  it('announces indeterminate package preparation', () => {
    const html = renderToStaticMarkup(<TransferPreparationMessage />)

    expect(html).toContain('role="status"')
    expect(html).toContain('aria-live="polite"')
    expect(html).toContain('class="spinner"')
    expect(html).toContain('libraryTransfer.preparingSelection')
  })
})
