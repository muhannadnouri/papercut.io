// @vitest-environment jsdom
import {
  act,
  useState,
  type ComponentProps,
} from 'react'
import {
  createRoot,
  type Root,
} from 'react-dom/client'
import {
  afterEach,
  beforeEach,
  expect,
  it,
  vi,
} from 'vitest'
import '../../i18n'
import { DocumentsPanel } from './DocumentsPanel'

const uploads = [
  {
    title: 'Saved book',
    url: '/uploads/aa.html',
    format: 'epub',
    source: 'upload',
  },
  {
    title: 'Saved document',
    url: '/uploads/bb.html',
    format: 'html',
    source: 'upload',
  },
] as const
const props: ComponentProps<typeof DocumentsPanel> = {
  allDocuments: [...uploads],
  collapsedAuthors: new Set(),
  docFilterLower: 'saved',
  documentFilter: 'Saved',
  documentsLoading: false,
  groupedDocs: [{
    author: 'Uploads',
    docs: [...uploads],
  }],
  libraryOrganization: {
    folders: [],
    documentLocations: [],
  },
  showDocuments: true,
  onFilterChange: vi.fn(),
  onToggleAuthor: vi.fn(),
  onToggleShow: vi.fn(),
  onViewDocument: vi.fn(),
  onCreateLibraryFolder: vi.fn(),
  onDeleteDocuments: vi.fn(),
  onDeleteLibraryFolder: vi.fn(),
  onMoveLibraryDocuments: vi.fn(),
  onRenameLibraryFolder: vi.fn(),
}

let root: Root
let host: HTMLDivElement
beforeEach(() => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  vi.stubGlobal('CSS', { escape: (value: string) => value.replace(/[^a-zA-Z0-9_-]/g, character => `\\${character}`) })
  host = document.body.appendChild(document.createElement('div'))
  root = createRoot(host)
})
afterEach(async () => {
  await act(() => root.unmount())
  host.remove()
  localStorage.clear()
  vi.unstubAllGlobals()
})
async function render(overrides: Partial<typeof props> = {}) {
  await act(() => root.render(<DocumentsPanel {...props} {...overrides} />))
}
function button(selector: string) {
  return host.querySelector<HTMLButtonElement>(selector)!
}
const manage = '.document-manage-btn'
const done = '.uploaded-library-done'

it.each(['books', 'documents'])('returns from Manage to the filtered %s gallery and its trigger', async category => {
  localStorage.setItem('papercut.library-view.v1', 'gallery')
  await render()
  if (category === 'documents') {
    await act(() => button('.library-gallery-category:last-child').click())
  }
  expect(button(manage).textContent).toBe('Manage')
  await act(() => button(manage).click())
  expect(document.activeElement).toBe(button(done))
  expect(host.querySelector('.uploaded-library')?.textContent).toContain('Saved book')
  expect(host.querySelector('.uploaded-library')?.textContent).toContain('Saved document')
  await act(() => button(done).click())
  expect(document.activeElement).toBe(button(manage))
  expect(button('.library-gallery-category.active').textContent?.toLowerCase()).toContain(category)
  expect(host.querySelector('input')?.value).toBe('Saved')
  expect(localStorage.getItem('papercut.library-view.v1')).toBe('gallery')
})

it('returns focus to the active category when the last matching upload disappears', async () => {
  await render()
  await act(() => button(manage).click())
  await render({
    allDocuments: [],
    groupedDocs: [],
  })
  expect(button(manage)).toBeNull()
  expect(document.activeElement).toBe(button('.library-gallery-category.active'))
})

it('keeps filter focus when typing through zero matches and backspacing to recover', async () => {
  function FilteredPanel() {
    const [query, setQuery] = useState('Saved')
    const matches = uploads.filter(doc => doc.title.toLowerCase().includes(query.toLowerCase()))
    return <DocumentsPanel {...props} documentFilter={query} docFilterLower={query.toLowerCase()}
      groupedDocs={[{
        author: 'Uploads',
        docs: matches,
      }]} onFilterChange={setQuery} />
  }
  await act(() => root.render(<FilteredPanel />))
  await act(() => button(manage).click())
  const input = host.querySelector<HTMLInputElement>('.document-filter-input')!
  input.focus()
  const setValue = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!
  for (const query of ['Savedx', 'Saved']) {
    await act(() => {
      setValue.call(input, query)
      input.dispatchEvent(new Event('input', { bubbles: true }))
    })
    expect(document.activeElement).toBe(input)
    expect(Boolean(button(manage))).toBe(query === 'Saved')
  }
})

it('keeps the final deletion notice dismissible without offering List Manage', async () => {
  localStorage.setItem('papercut.library-view.v1', 'list')
  const onDismiss = vi.fn()
  await render()
  await act(() => button(manage).click())
  await render({
    allDocuments: [],
    groupedDocs: [],
    documentFilter: '',
    docFilterLower: '',
    importStatuses: [{
      status: 'deleted',
      message: 'Deleted upload',
      manageContext: true,
      onDismiss,
    }],
  })
  expect(host.querySelector('.uploaded-library [role=status]')?.textContent).toContain('Deleted upload')
  expect(button(manage)).toBeNull()
  await act(() => button('.document-import-dismiss').click())
  expect(onDismiss).toHaveBeenCalledOnce()
})

it('offers Manage for an empty folder only when it is visible and organizing is supported', async () => {
  const empty = {
    allDocuments: [],
    groupedDocs: [],
    libraryOrganization: {
      folders: [{
        id: 'a',
        name: 'Empty folder',
        parentId: null,
        depth: 0,
        sortOrder: 0,
        createdAtMs: 0,
        updatedAtMs: 0,
      }],
      documentLocations: [],
    },
  }
  await render(empty)
  expect(button(manage)).toBeNull()
  await render({
    ...empty,
    docFilterLower: '',
    documentFilter: '',
  })
  await act(() => button(manage).click())
  expect(host.querySelector('.uploaded-library')?.textContent).toContain('Empty folder')
  await render({
    ...empty,
    onMoveLibraryDocuments: undefined,
  })
  expect(button(manage)).toBeNull()
})

it.each(['gallery', 'list'])('shows only uploads in Manage and restores the %s browser', async view => {
  const bundled = {
    title: 'Papercut guide',
    url: '/documents/Papercut/guide.html',
    source: 'bundled',
  } as const
  const docs = [...uploads, bundled]
  localStorage.setItem('papercut.library-view.v1', view)
  localStorage.setItem('papercut.library-gallery-category.v1', 'documents')
  await render({
    allDocuments: docs,
    groupedDocs: [{
      author: 'Library',
      docs,
    }],
  })
  expect(host.querySelector('.uploaded-library-toolbar-actions')).toBeNull()
  expect(host.textContent).toContain(bundled.title)
  if (view === 'list') {
    await act(() => button('.uploaded-library-heading').click())
  }
  const switcher = button('.library-view-toggle')
  const switchLabel = switcher.getAttribute('aria-label')
  button(manage).focus()
  await act(() => button(manage).click())
  expect(document.activeElement).toBe(button(done))
  expect(host.textContent).toContain(uploads[0].title)
  expect(host.textContent).not.toContain(bundled.title)
  expect(button(manage).disabled).toBe(true)
  expect(button('.library-view-toggle')).toBe(switcher)
  expect(switcher.disabled).toBe(true)
  expect(switcher.getAttribute('aria-label')).toBe(switchLabel)
  await act(() => switcher.click())
  await act(() => button(done).click())
  expect(document.activeElement).toBe(button(manage))
  expect(host.textContent).toContain(bundled.title)
  expect(switcher.disabled).toBe(false)
  expect(button(manage).disabled).toBe(false)
  expect(localStorage.getItem('papercut.library-view.v1')).toBe(view)
})

it('returns to browsing when import starts and prevents re-entry while it is busy', async () => {
  await render()
  await act(() => button(manage).click())
  await render({
    importStatuses: [{
      status: 'importing',
      message: 'Importing',
    }],
  })
  expect(button(done)).toBeNull()
  expect(button(manage).disabled).toBe(true)
  expect(document.activeElement).toBe(button('.library-gallery-category.active'))
  await render()
  expect(button(manage).disabled).toBe(false)
})

it('routes gallery deletion through selection and confirmation in Manage', async () => {
  const batchDelete = vi.fn().mockResolvedValue(null)
  const singleDelete = vi.fn()
  await render({
    onDeleteDocuments: batchDelete,
    onDeleteDocument: singleDelete,
  })
  await act(() => button('.library-gallery-category:last-child').click())
  expect(button('.document-row-action-danger')).toBeNull()
  await act(() => button(manage).click())
  const remove = '.uploaded-library-batch-delete'
  expect(button(remove)).toBeNull()
  const selectDocument = host.querySelector<HTMLInputElement>('[aria-label="Select Saved document"]')!
  await act(() => selectDocument.click())
  await act(() => button(remove).click())
  expect(host.querySelector('[role="dialog"]')?.textContent).toContain('Delete selected documents?')
  await act(() => button('.app-dialog-cancel').click())
  expect(selectDocument.checked).toBe(true)
  expect(batchDelete).not.toHaveBeenCalled()
  await act(() => button(remove).click())
  await act(() => button('.app-dialog-submit-danger').click())
  expect(batchDelete).toHaveBeenCalledExactlyOnceWith([expect.objectContaining({ url: uploads[1].url })])
  expect(singleDelete).not.toHaveBeenCalled()
})

it('offers View Audiobooks instead of deleting a managed upload with saved audio', async () => {
  const batchDelete = vi.fn()
  const viewAudiobooks = vi.fn()
  await render({
    onDeleteDocuments: batchDelete,
    onViewAudiobooks: viewAudiobooks,
    savedAudiobookDocumentUrls: new Set([uploads[0].url]),
  })
  await act(() => button(manage).click())
  await act(() => host.querySelector<HTMLInputElement>('[aria-label="Select Saved book"]')!.click())
  await act(() => button('.uploaded-library-batch-delete').click())
  expect(button('.app-dialog-submit').textContent).toBe('View Audiobooks')
  await act(() => button('.app-dialog-submit').click())
  expect(viewAudiobooks).toHaveBeenCalledOnce()
  expect(batchDelete).not.toHaveBeenCalled()
})

it('keeps folder checkboxes and bulk actions in sync across collapsed branches', async () => {
  const folder = {
    id: 'parent',
    name: 'Shelf',
    depth: 0,
    sortOrder: 0,
    createdAtMs: 0,
    updatedAtMs: 0,
  }
  await render({
    documentFilter: '',
    docFilterLower: '',
    libraryOrganization: {
      folders: [folder, {
        ...folder,
        id: 'child',
        name: 'Archive',
        parentId: folder.id,
        depth: 1,
      }],
      documentLocations: [
        {
          documentId: 'aa',
          folderId: folder.id,
          sortOrder: 0,
        },
        {
          documentId: 'bb',
          folderId: 'child',
          sortOrder: 0,
        },
      ],
    },
  })
  await act(() => button(manage).click())
  const box = (label: string) => host.querySelector<HTMLInputElement>('[aria-label="' + label + '"]')!
  const shelf = box('Select all in Shelf')
  expect(box('Select Saved book')).toBeNull()
  await act(() => shelf.click())
  expect(shelf.checked).toBe(true)
  const count = () => host.querySelector('.uploaded-library-selection-count')?.textContent
  expect(count()).toBe('2 documents selected')
  await act(() => button('.uploaded-library-chevron').click())
  expect(box('Select all in Archive').checked).toBe(true)
  await act(() => box('Select Saved book').click())
  expect(count()).toBe('1 document selected')
  expect(shelf.indeterminate).toBe(true)
  expect(box('Select all in Archive').checked).toBe(true)
  const bulk = () => button('.uploaded-library-selection-actions button')
  expect(bulk().textContent).toBe('Select all documents')
  await act(() => bulk().click())
  expect(shelf.indeterminate).toBe(false)
  expect(box('Select Saved book').checked).toBe(true)
  await act(() => bulk().click())
  expect(count()).toBe('0 documents selected')
  expect(shelf.checked).toBe(false)
  expect(box('Select all in Archive').checked).toBe(false)
})

it('blocks folder deletion when a filtered-out descendant has saved audio', async () => {
  const deleteFolder = vi.fn()
  const viewAudiobooks = vi.fn()
  const folder = {
    id: 'parent',
    name: 'Shelf',
    depth: 0,
    sortOrder: 0,
    createdAtMs: 0,
    updatedAtMs: 0,
  }
  await render({
    documentFilter: 'book',
    docFilterLower: 'book',
    groupedDocs: [{
      author: 'Uploads',
      docs: [uploads[0]],
    }],
    libraryOrganization: {
      folders: [folder, {
        ...folder,
        id: 'child',
        name: 'Archive',
        parentId: folder.id,
        depth: 1,
      }],
      documentLocations: [
        {
          documentId: 'aa',
          folderId: folder.id,
          sortOrder: 0,
        },
        {
          documentId: 'bb',
          folderId: 'child',
          sortOrder: 0,
        },
      ],
    },
    savedAudiobookDocumentUrls: new Set([uploads[1].url]),
    onDeleteLibraryFolder: deleteFolder,
    onViewAudiobooks: viewAudiobooks,
  })
  await act(() => button(manage).click())
  expect(host.textContent).toContain(uploads[0].title)
  expect(host.textContent).not.toContain(uploads[1].title)
  await act(() => button('[aria-label="Select all in Shelf"]').click())
  expect(host.querySelector('.uploaded-library-selection-count')?.textContent).toBe('1 document selected')
  async function deleteShelf() {
    const trigger = button('[aria-label="Folder actions for Shelf"]')
    expect(trigger.textContent).toBe('')
    expect(trigger.querySelector('svg[aria-hidden="true"]')).toBeTruthy()
    await act(() => trigger.click())
    const action = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')]
      .find(item => item.textContent === 'Delete Folder')
    expect(action).toBeDefined()
    await act(() => action!.click())
  }
  await deleteShelf()
  expect(host.querySelector('[role="dialog"]')?.textContent).toContain('Saved Audiobooks Use Documents in This Folder')
  await act(() => button('.app-dialog-cancel').click())
  expect(viewAudiobooks).not.toHaveBeenCalled()
  await deleteShelf()
  await act(() => button('.app-dialog-submit').click())
  expect(viewAudiobooks).toHaveBeenCalledOnce()
  expect(deleteFolder).not.toHaveBeenCalled()
})

it.each(['gallery', 'list'])('keeps single-document Delete available in the %s fallback', async view => {
  const singleDelete = vi.fn()
  localStorage.setItem('papercut.library-view.v1', view)
  localStorage.setItem('papercut.library-gallery-category.v1', 'documents')
  await render({
    onDeleteDocuments: undefined,
    onDeleteDocument: singleDelete,
  })
  expect(button(manage)).toBeNull()
  const row = [...host.querySelectorAll('.document-item-browse')].find(item => item.textContent?.includes(uploads[1].title))
  await act(() => row!.querySelector<HTMLButtonElement>('.document-row-action-danger')!.click())
  expect(singleDelete).toHaveBeenCalledExactlyOnceWith(uploads[1])
})
