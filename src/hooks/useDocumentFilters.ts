import { useState, useCallback, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import type { DocumentInfo } from '../types/search'
import { bundledDocumentFolderNames } from '../components/DocumentBrowser/bundledDocuments'
import { deriveAuthor, UNCATEGORIZED } from '../utils/documentUtils'

export interface AuthorGroup {
  author: string
  docs: DocumentInfo[]
}

interface UseDocumentFiltersOptions {
  includeDocument?: (doc: DocumentInfo) => boolean
}

interface UseDocumentFiltersReturn {
  selectedFilters: Set<string>
  showDocuments: boolean
  documentFilter: string
  collapsedAuthors: Set<string>
  groupedDocs: AuthorGroup[]
  docFilterLower: string
  filterTitleByUrl: Map<string, string>
  toggleFilter: (url: string) => void
  clearFilters: () => void
  removeFilter: (url: string) => void
  toggleAuthor: (author: string) => void
  toggleAllInGroup: (docs: DocumentInfo[]) => void
  setShowDocuments: React.Dispatch<React.SetStateAction<boolean>>
  setDocumentFilter: React.Dispatch<React.SetStateAction<string>>
}

export function useDocumentFilters(
  allDocuments: DocumentInfo[],
  options: UseDocumentFiltersOptions = {},
): UseDocumentFiltersReturn {
  const { t, i18n } = useTranslation()
  const [selectedFilters, setSelectedFilters] = useState<Set<string>>(new Set())
  const [showDocuments, setShowDocuments] = useState(true)
  const [documentFilter, setDocumentFilter] = useState('')
  const [collapsedAuthors, setCollapsedAuthors] = useState<Set<string>>(new Set())

  const { includeDocument } = options
  const locale = i18n.resolvedLanguage ?? i18n.language
  const docFilterLower = documentFilter.trim().toLocaleLowerCase(locale)
  const collator = useMemo(
    () => new Intl.Collator(locale, { numeric: true, sensitivity: 'base' }),
    [locale],
  )

  const filterTitleByUrl = useMemo(() => {
    const entries = allDocuments
      .filter((doc) => !includeDocument || includeDocument(doc))
      .map((doc) => [doc.url, doc.title] as const)
    return new Map(entries)
  }, [allDocuments, includeDocument])

  const groupedDocs = useMemo<AuthorGroup[]>(() => {
    const groups = new Map<string, DocumentInfo[]>()
    for (const doc of allDocuments) {
      if (includeDocument && !includeDocument(doc)) continue

      const derivedAuthor = deriveAuthor(doc.url)
      const author = doc.source === 'audiobook-upload'
        ? t('library.groups.importedAudiobooks')
        : doc.source === 'upload'
          ? t('library.groups.userUploads')
          : derivedAuthor === UNCATEGORIZED
            ? t('library.groups.uncategorized')
            : derivedAuthor
      const bundledFolderMatches = doc.source === 'bundled' &&
        bundledDocumentFolderNames(doc.url).some((folder) => (
          folder.toLocaleLowerCase(locale).includes(docFilterLower)
        ))
      if (
        docFilterLower.length > 0 &&
        !doc.title.toLocaleLowerCase(locale).includes(docFilterLower) &&
        !author.toLocaleLowerCase(locale).includes(docFilterLower) &&
        !bundledFolderMatches
      ) {
        continue
      }
      const list = groups.get(author)
      if (list) list.push(doc)
      else groups.set(author, [doc])
    }
    return Array.from(groups.entries())
      .map(([author, docs]) => ({
        author,
        docs: docs.slice().sort((a, b) => collator.compare(a.title, b.title)),
      }))
      .sort((a, b) => {
        const uncategorized = t('library.groups.uncategorized')
        if (a.author === uncategorized) return 1
        if (b.author === uncategorized) return -1
        return collator.compare(a.author, b.author)
      })
  }, [allDocuments, collator, docFilterLower, includeDocument, locale, t])

  const toggleFilter = useCallback((url: string) => {
    setSelectedFilters((prev) => {
      const next = new Set(prev)
      if (next.has(url)) next.delete(url)
      else next.add(url)
      return next
    })
  }, [])

  const clearFilters = useCallback(() => {
    setSelectedFilters(new Set())
  }, [])

  const removeFilter = useCallback((url: string) => {
    setSelectedFilters((prev) => {
      if (!prev.has(url)) return prev
      const next = new Set(prev)
      next.delete(url)
      return next
    })
  }, [])

  const toggleAuthor = useCallback((author: string) => {
    setCollapsedAuthors((prev) => {
      const next = new Set(prev)
      if (next.has(author)) next.delete(author)
      else next.add(author)
      return next
    })
  }, [])

  const toggleAllInGroup = useCallback((docs: DocumentInfo[]) => {
    setSelectedFilters((prev) => {
      const next = new Set(prev)
      const allSelected = docs.every((d) => next.has(d.url))
      if (allSelected) docs.forEach((d) => next.delete(d.url))
      else docs.forEach((d) => next.add(d.url))
      return next
    })
  }, [])

  return {
    selectedFilters,
    showDocuments,
    documentFilter,
    collapsedAuthors,
    groupedDocs,
    docFilterLower,
    filterTitleByUrl,
    toggleFilter,
    clearFilters,
    removeFilter,
    toggleAuthor,
    toggleAllInGroup,
    setShowDocuments,
    setDocumentFilter,
  }
}
