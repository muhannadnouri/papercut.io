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
  onMoveLibraryItems: vi.fn(),
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
    onMoveLibraryItems: undefined,
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
  const shelf = box('Select Shelf')
  expect(box('Select Saved book')).toBeNull()
  await act(() => shelf.click())
  expect(shelf.checked).toBe(true)
  const count = () => host.querySelector('.uploaded-library-selection-count')?.textContent
  expect(count()).toBe('Folders (1)')
  await act(() => button('.uploaded-library-chevron').click())
  expect(box('Select Archive').checked).toBe(true)
  expect(box('Select Saved book').disabled).toBe(true)
  await act(() => shelf.click())
  await act(() => box('Select Saved book').click())
  expect(count()).toBe('1 document selected')
  expect(shelf.indeterminate).toBe(true)
  expect(box('Select Archive').checked).toBe(false)
  const bulk = () => button('.uploaded-library-selection-actions button')
  expect(bulk().textContent).toBe('Select All')
  await act(() => bulk().click())
  expect(shelf.indeterminate).toBe(true)
  expect(box('Select Saved book').checked).toBe(true)
  await act(() => bulk().click())
  expect(count()).toBe('0 documents selected')
  expect(shelf.checked).toBe(false)
  expect(box('Select Archive').checked).toBe(false)
})

it.each([true, false])('moves explicit folders intact and individual files separately (folder selection: %s)', async selectFolder => {
  const moveDocs = vi.fn()
  const outside = {
    ...uploads[0],
    title: 'Other book',
    url: '/uploads/cc.html',
  }
  const a = {
    id: 'a',
    name: 'A',
    depth: 0,
    sortOrder: 0,
    createdAtMs: 0,
    updatedAtMs: 0,
  }
  await render({
    allDocuments: [...uploads, outside],
    documentFilter: selectFolder ? 'book' : 'saved',
    docFilterLower: selectFolder ? 'book' : 'saved',
    groupedDocs: [{
      author: 'Uploads',
      docs: selectFolder ? [uploads[0], outside] : [...uploads],
    }],
    onMoveLibraryItems: moveDocs,
    libraryOrganization: {
      folders: [a, {
        ...a,
        id: 'nested',
        name: 'Nested',
        parentId: 'a',
        depth: 1,
      }, {
        ...a,
        id: 'b',
        name: 'B',
      }],
      documentLocations: [
        {
          documentId: 'aa',
          folderId: 'a',
          sortOrder: 0,
        },
        {
          documentId: 'bb',
          folderId: 'nested',
          sortOrder: 0,
        },
      ],
    },
  })
  await act(() => button(manage).click())
  if (selectFolder) {
    expect(host.textContent).not.toContain(uploads[1].title)
    await act(() => button('[aria-label="Select A"]').click())
    await act(() => button('.uploaded-library-selection-actions button').click())
    expect(button('.uploaded-library-selection-actions button').textContent).toBe('Deselect All')
  } else {
    for (const doc of uploads) {
      await act(() => button('[aria-label="Select ' + doc.title + '"]').click())
    }
  }
  await act(() => button('.uploaded-library-move-trigger').click())
  const rows = [...host.querySelectorAll<HTMLButtonElement>('.uploaded-library-destination')]
  expect(rows.find(row => row.textContent === 'A')?.disabled).toBe(selectFolder)
  await act(() => rows.find(row => row.textContent === 'B')!.click())
  await act(() => button('.app-dialog-submit').click())
  const confirmation = host.querySelector('[role="dialog"]')!
  expect(confirmation.textContent?.includes('including items hidden by the current filter')).toBe(selectFolder)
  expect(Array.from(confirmation.querySelectorAll('li'), item => item.textContent).sort())
    .toEqual(selectFolder ? ['A', outside.title] : uploads.map(doc => doc.title).sort())
  expect(moveDocs).not.toHaveBeenCalled()
  await act(() => button('.app-dialog-submit').click())
  if (selectFolder) {
    expect(moveDocs).toHaveBeenCalledExactlyOnceWith(['cc'], ['a'], 'b')
  } else {
    expect(moveDocs).toHaveBeenCalledExactlyOnceWith(expect.arrayContaining(['aa', 'bb']), [], 'b')
  }
})

async function renderFilteredShelf(overrides: Partial<typeof props>) {
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
    ...overrides,
  })
  await act(() => button(manage).click())
  expect(host.textContent).toContain(uploads[0].title)
  expect(host.textContent).not.toContain(uploads[1].title)
  await act(() => button('[aria-label="Select Saved book"]').click())
  expect(host.querySelector('.uploaded-library-selection-count')?.textContent).toBe('1 document selected')
}

it('browses one level at a time and moves only after the destination and documents are confirmed', async () => {
  const move = vi.fn().mockRejectedValueOnce(new Error('Move failed')).mockResolvedValue(undefined)
  await renderFilteredShelf({ onMoveLibraryItems: move })
  const open = () => button('.uploaded-library-task-actions button')
  const dialog = () => host.querySelector('[role="dialog"]')
  const path = () => dialog()?.querySelector('.uploaded-library-move-path')?.textContent
  const destinations = () => Array.from(dialog()!.querySelectorAll<HTMLButtonElement>('.uploaded-library-destination'))
  const descend = () => act(() => destinations()[0].click())
  const confirm = () => act(() => button('.app-dialog-submit').click())

  expect(open().textContent).toBe('Move')
  expect(open().firstElementChild?.getAttribute('aria-hidden')).toBe('true')
  open().focus()
  await act(() => open().click())
  expect(path()).toBe('Root')
  expect(destinations().map(item => item.textContent)).toEqual(['Shelf'])
  await descend()
  expect(path()).toBe('Root / Shelf')
  expect(button('.app-dialog-submit').disabled).toBe(true)
  await confirm()
  expect(move).not.toHaveBeenCalled()
  expect(destinations().map(item => item.textContent)).toEqual(['Archive'])
  await descend()
  expect(path()).toBe('Root / Shelf / Archive')
  expect(destinations()).toHaveLength(0)
  const crumbs = Array.from(dialog()!.querySelectorAll<HTMLButtonElement>('.uploaded-library-move-path button'))
  expect(crumbs.map(item => item.textContent)).toEqual(['Root', 'Shelf'])
  expect(dialog()?.querySelector('bdi[aria-current="location"]')?.textContent).toBe('Archive')
  await act(() => crumbs[1].click())
  expect(path()).toBe('Root / Shelf')
  await act(() => button('.app-dialog-close').click())
  expect(document.activeElement).toBe(open())
  expect(move).not.toHaveBeenCalled()

  await act(() => open().click())
  expect(path()).toBe('Root')
  await descend()
  await descend()
  await confirm()
  expect(dialog()?.querySelector('h2')?.textContent).toBe('Move these documents to this folder?')
  expect(Array.from(dialog()!.querySelectorAll('li'), item => item.textContent)).toEqual([uploads[0].title])
  expect(path()).toBe('Root / Shelf / Archive')
  expect(move).not.toHaveBeenCalled()
  await act(() => button('.app-dialog-actions .app-dialog-cancel').click())
  expect(move).not.toHaveBeenCalled()
  expect(destinations()).toHaveLength(0)
  await confirm()
  await confirm()
  expect(dialog()?.querySelector('[role="alert"]')?.textContent).toBe('Move failed')
  expect(path()).toBe('Root / Shelf / Archive')
  await confirm()
  expect(move.mock.calls).toEqual([[['aa'], [], 'child'], [['aa'], [], 'child']])
  expect(dialog()).toBeNull()
  expect(host.querySelector('.uploaded-library-selection-count')?.textContent).toBe('0 documents selected')

  await act(() => button('[aria-label="Select Saved book"]').click())
  await act(() => open().click())
  await confirm()
  expect(move).toHaveBeenCalledTimes(2)
  await confirm()
  expect(move).toHaveBeenLastCalledWith(['aa'], [], null)
})

it('lists every selected document before a bulk move', async () => {
  const move = vi.fn()
  const shelf = {
    id: 'shelf',
    name: 'Shelf',
    depth: 0,
    sortOrder: 0,
    createdAtMs: 0,
    updatedAtMs: 0,
  }
  await render({
    onMoveLibraryItems: move,
    libraryOrganization: {
      folders: [shelf],
      documentLocations: [],
    },
  })
  await act(() => button(manage).click())
  await act(() => button('.uploaded-library-selection-actions button').click())
  await act(() => button('.uploaded-library-move-trigger').click())
  expect(button('.app-dialog-submit').disabled).toBe(true)
  await act(() => button('.uploaded-library-destination').click())
  await act(() => button('.app-dialog-submit').click())
  expect(Array.from(host.querySelectorAll('.uploaded-library-move-documents li'), li => li.textContent).sort())
    .toEqual(uploads.map(doc => doc.title).sort())
  expect(move).not.toHaveBeenCalled()
  await act(() => button('.app-dialog-submit').click())
  expect(move).toHaveBeenCalledExactlyOnceWith(['aa', 'bb'], [], 'shelf')
})

it('confirms and submits only incoming documents for a mixed selection at Root', async () => {
  const move = vi.fn()
  await render({
    onMoveLibraryItems: move,
    libraryOrganization: {
      folders: [{
        id: 'shelf',
        name: 'Shelf',
        depth: 0,
        sortOrder: 0,
        createdAtMs: 0,
        updatedAtMs: 0,
      }],
      documentLocations: [{
        documentId: 'aa',
        folderId: 'shelf',
        sortOrder: 0,
      }],
    },
  })
  await act(() => button(manage).click())
  await act(() => button('.uploaded-library-selection-actions button').click())
  await act(() => button('.uploaded-library-move-trigger').click())
  await act(() => button('.app-dialog-submit').click())
  expect(Array.from(host.querySelectorAll('.uploaded-library-move-documents li'), li => li.textContent))
    .toEqual([uploads[0].title])
  expect(move).not.toHaveBeenCalled()
  await act(() => button('.app-dialog-submit').click())
  expect(move).toHaveBeenCalledExactlyOnceWith(['aa'], [], null)
})

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

it('confirms the full filtered folder scope and deletes only after confirmation', async () => {
  const deleteFolder = vi.fn().mockResolvedValue(null)
  await renderFilteredShelf({ onDeleteLibraryFolder: deleteFolder })
  await deleteShelf()
  const dialog = host.querySelector('[role="dialog"]')!
  const details = Array.from(dialog.querySelectorAll('dt'), term => [
    term.textContent, term.nextElementSibling?.textContent,
  ])
  expect(Object.fromEntries(details)).toMatchObject({
    Folder: 'Shelf',
    'Nested folders': '1',
    Documents: '2',
  })
  await act(() => button('.app-dialog-cancel').click())
  expect(deleteFolder).not.toHaveBeenCalled()
  await deleteShelf()
  await act(() => button('.app-dialog-submit-danger').click())
  expect(deleteFolder).toHaveBeenCalledExactlyOnceWith('parent')
})

it('blocks folder deletion when a filtered-out descendant has saved audio', async () => {
  const deleteFolder = vi.fn()
  const viewAudiobooks = vi.fn()
  await renderFilteredShelf({
    savedAudiobookDocumentUrls: new Set([uploads[1].url]),
    onDeleteLibraryFolder: deleteFolder,
    onViewAudiobooks: viewAudiobooks,
  })
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
