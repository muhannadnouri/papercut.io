// @vitest-environment jsdom
import {
  act,
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
const manage = '.library-gallery-manage'
const done = '.uploaded-library-edit-btn[aria-pressed="true"]'

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

it('leaves ordinary List management in List', async () => {
  localStorage.setItem('papercut.library-view.v1', 'list')
  await render()
  await act(() => button('.uploaded-library-edit-btn[aria-pressed="false"]').click())
  await act(() => button(done).click())
  expect(button('.uploaded-library-edit-btn[aria-pressed="false"]')).toBeTruthy()
  expect(host.querySelector('.library-gallery-view')).toBeNull()
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
