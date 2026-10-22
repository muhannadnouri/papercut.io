// @vitest-environment jsdom
import { act } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { TranslatedDocumentsPanel } from './TranslatedDocumentsPanel'
import type { TranslatedDocumentInfo } from '../api/nativeTranslation'

vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key, i18n: { language: 'en' } }) }))
vi.mock('./TranslationReviewDialog', () => ({ TranslationReviewDialog: ({ document, onClose, onPublished }: {
  document: TranslatedDocumentInfo; onClose: () => void; onPublished: (info: TranslatedDocumentInfo) => Promise<void>
}) => <div role="dialog"><span>{document.id}</span><button onClick={onClose}>Close review</button><button onClick={() => { void onPublished({ ...document, status: 'ready' }) }}>Publish review</button></div> }))

const saved: TranslatedDocumentInfo = { id: 'abc123', title: 'Saved example', documentUrl: '/uploads/abc123.html', sourceDocumentUrl: '/uploads/def456.html', sourceLanguage: 'es', targetLanguage: 'en', modelId: 'opus-mt-es-en-ctranslate2', status: 'needs-review', createdAtMs: 1, updatedAtMs: 1 }
let container: HTMLDivElement, root: Root
const published = vi.fn(), opened = vi.fn(), deleted = vi.fn()
async function render(docs = [saved]) {
  await act(async () => root.render(<TranslatedDocumentsPanel translatedDocuments={docs} modelNameById={new Map()} deleteState={null} onDeleteTranslatedDocument={deleted} onOpenTranslatedDocument={opened} onReviewPublished={published} />))
}
beforeEach(async () => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  vi.clearAllMocks()
  container = document.createElement('div'); document.body.append(container); root = createRoot(container)
  await render()
})
afterEach(async () => { await act(async () => root.unmount()); container.remove(); vi.unstubAllGlobals() })

it('offers model-independent review on saved translations and keeps View and Delete separate', async () => {
  const review = container.querySelector<HTMLButtonElement>('button[aria-label="translation.review.openLabel"]')!
  expect(review.disabled).toBe(false)
  await act(async () => review.click())
  expect(container.querySelector('[role="dialog"]')?.textContent).toContain(saved.id)
  expect(opened).not.toHaveBeenCalled(); expect(deleted).not.toHaveBeenCalled()
  await act(async () => [...container.querySelectorAll('button')].find(b => b.textContent === 'Publish review')!.click())
  expect(published).toHaveBeenCalledExactlyOnceWith({ ...saved, status: 'ready' })
})

it('does not discard an open session when an external refresh removes the saved row', async () => {
  await act(async () => container.querySelector<HTMLButtonElement>('button[aria-label="translation.review.openLabel"]')!.click())
  await render([])
  expect(container.querySelector('[role="dialog"]')?.textContent).toContain(saved.id)
  await act(async () => [...container.querySelectorAll('button')].find(b => b.textContent === 'Close review')!.click())
  expect(container.querySelector('[role="dialog"]')).toBeNull()
  expect(opened).not.toHaveBeenCalled(); expect(deleted).not.toHaveBeenCalled()
})
