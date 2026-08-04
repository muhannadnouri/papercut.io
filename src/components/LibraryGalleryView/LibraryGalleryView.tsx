import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { AuthorGroup } from '../../hooks/useDocumentFilters'
import type { DocumentInfo } from '../../types/search'
import { getUploadedDocumentCover } from '../../uploads/DocumentUploads'
import { BookmarkIndicator } from '../BookmarkIndicator/BookmarkIndicator'
import { BundledDocumentTree } from '../BundledDocumentTree/BundledDocumentTree'
import { splitDocumentGroupsBySource } from '../DocumentBrowser/documentGroups'
import { DocumentList } from '../DocumentList/DocumentList'
import { isBookDocument } from './libraryCategories'
import './LibraryGalleryView.css'

export type LibraryGalleryCategory = 'books' | 'documents'

interface LibraryGalleryViewProps {
  category: LibraryGalleryCategory
  bookmarkedDocumentUrls: ReadonlySet<string>
  collapsedAuthors: Set<string>
  docFilterLower: string
  groupedDocs: AuthorGroup[]
  savedAudiobookDocumentUrls: ReadonlySet<string>
  documentOpening?: boolean
  mutationDisabled?: boolean
  openingDocumentUrl?: string
  emptyMessage: string
  onCategoryChange: (category: LibraryGalleryCategory) => void
  onDeleteDocument?: (doc: DocumentInfo) => void | Promise<void>
  onRecognizeDocument?: (documentUrl: string) => void | Promise<boolean>
  onToggleAuthor: (author: string) => void
  onViewDocument: (url: string) => void
}

/** Present cover-friendly formats visually while reusing the existing dense
 * document list for formats that do not have useful cover art. */
export function LibraryGalleryView({
  category,
  bookmarkedDocumentUrls,
  collapsedAuthors,
  docFilterLower,
  groupedDocs,
  savedAudiobookDocumentUrls,
  documentOpening = false,
  mutationDisabled = false,
  openingDocumentUrl,
  emptyMessage,
  onCategoryChange,
  onDeleteDocument,
  onRecognizeDocument,
  onToggleAuthor,
  onViewDocument,
}: LibraryGalleryViewProps) {
  const { t } = useTranslation()
  const bookGroups = filterGroups(groupedDocs, isBookDocument)
  const documentGroups = filterGroups(groupedDocs, (doc) => !isBookDocument(doc))
  const { bundledDocs, nonBundledGroups } = splitDocumentGroupsBySource(documentGroups)
  const books = bookGroups.flatMap((group) => group.docs)
  const documentCount = documentGroups.reduce((total, group) => total + group.docs.length, 0)

  return (
    <div className="library-gallery-view">
      <div className="library-gallery-categories" role="group" aria-label={t('library.documents.categoryLabel')}>
        <CategoryButton
          active={category === 'books'}
          label={t('library.documents.books', { count: books.length })}
          onClick={() => onCategoryChange('books')}
        />
        <CategoryButton
          active={category === 'documents'}
          label={t('library.documents.documentCategory', { count: documentCount })}
          onClick={() => onCategoryChange('documents')}
        />
      </div>

      {category === 'books' ? (
        books.length > 0 ? (
          <div className="library-book-grid">
            {books.map((doc) => (
              <BookCard
                key={doc.url}
                doc={doc}
                hasBookmark={bookmarkedDocumentUrls.has(doc.url)}
                hasSavedAudio={savedAudiobookDocumentUrls.has(doc.url)}
                opening={openingDocumentUrl === doc.url}
                disabled={documentOpening || mutationDisabled}
                bookmarkLabel={t('library.documents.bookmarked')}
                openingLabel={t('common.opening')}
                savedAudioLabel={t('library.documents.savedAudioAvailable')}
                recognizeEnglishLabel={t('library.documents.recognizeEnglishText')}
                textRecognitionRequiredLabel={t('library.documents.textRecognitionRequired')}
                onOpen={onViewDocument}
                onRecognize={onRecognizeDocument}
              />
            ))}
          </div>
        ) : (
          <p className="no-results">{groupedDocs.length === 0 ? emptyMessage : t('library.documents.emptyBooks')}</p>
        )
      ) : (
        <>
          {bundledDocs.length > 0 && (
            <BundledDocumentTree
              documents={bundledDocs}
              filterActive={docFilterLower.length > 0}
              documentOpening={documentOpening}
              openingDocumentUrl={openingDocumentUrl}
              onViewDocument={onViewDocument}
            />
          )}

          {(nonBundledGroups.length > 0 || bundledDocs.length === 0) && (
            <DocumentList
              groupedDocs={nonBundledGroups}
              collapsedAuthors={collapsedAuthors}
              docFilterLower={docFilterLower}
              emptyMessage={groupedDocs.length === 0 ? emptyMessage : t('library.documents.emptyCategoryDocuments')}
              onToggleAuthor={onToggleAuthor}
              onViewDocument={onViewDocument}
              onDeleteDocument={onDeleteDocument}
              deleteDisabled={documentOpening || mutationDisabled}
              openingDocumentUrl={openingDocumentUrl}
              viewDisabled={documentOpening}
            />
          )}
        </>
      )}
    </div>
  )
}

function CategoryButton({ active, label, onClick }: { active: boolean; label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      className={active ? 'library-gallery-category active' : 'library-gallery-category'}
      aria-pressed={active}
      onClick={onClick}
    >
      {label}
    </button>
  )
}

function BookCard({
  doc,
  hasBookmark,
  hasSavedAudio,
  opening,
  disabled,
  bookmarkLabel,
  openingLabel,
  savedAudioLabel,
  recognizeEnglishLabel,
  textRecognitionRequiredLabel,
  onOpen,
  onRecognize,
}: {
  doc: DocumentInfo
  hasBookmark: boolean
  hasSavedAudio: boolean
  opening: boolean
  disabled: boolean
  bookmarkLabel: string
  openingLabel: string
  savedAudioLabel: string
  recognizeEnglishLabel: string
  textRecognitionRequiredLabel: string
  onOpen: (url: string) => void
  onRecognize?: (documentUrl: string) => void | Promise<boolean>
}) {
  const placeholderClass = `library-book-cover placeholder-${titleColor(doc.title)}`
  const coverRef = useRef<HTMLSpanElement>(null)
  const [cover, setCover] = useState<string | null>(null)

  // Avoid decoding every retained cover in a large library; nearby cards load
  // shortly before scrolling brings them into view.
  useEffect(() => {
    if (doc.source !== 'upload' || !doc.coverMediaType) return
    let cancelled = false
    const load = () => {
      void getUploadedDocumentCover(doc.url)
        .then((value) => {
          if (!cancelled) setCover(value)
        })
        .catch(() => undefined)
    }
    if (!('IntersectionObserver' in window)) {
      load()
      return () => { cancelled = true }
    }
    const observer = new IntersectionObserver((entries) => {
      if (!entries.some((entry) => entry.isIntersecting)) return
      observer.disconnect()
      load()
    }, { rootMargin: '200px' })
    if (coverRef.current) observer.observe(coverRef.current)
    return () => {
      cancelled = true
      observer.disconnect()
    }
  }, [doc.coverMediaType, doc.source, doc.url])

  return (
    <div className={opening ? 'library-book-card opening' : 'library-book-card'}>
      <button
        type="button"
        className="library-book-open"
        disabled={disabled}
        aria-label={opening ? openingLabel : doc.title}
        onClick={() => onOpen(doc.url)}
      >
        <span ref={coverRef} className={placeholderClass}>
          {cover
            ? <img className="library-book-cover-image" src={cover} alt="" decoding="async" />
            : <bdi>{doc.title}</bdi>}
          {hasSavedAudio && (
            <span
              className="library-book-audio"
              aria-label={savedAudioLabel}
              title={savedAudioLabel}
            >
              <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                <path d="M4 13v-1a8 8 0 0 1 16 0v1M6.5 12.5H5.75A1.75 1.75 0 0 0 4 14.25v3A1.75 1.75 0 0 0 5.75 19h.75zM17.5 12.5h.75A1.75 1.75 0 0 1 20 14.25v3A1.75 1.75 0 0 1 18.25 19h-.75z" />
              </svg>
            </span>
          )}
          {hasBookmark && (
            <BookmarkIndicator
              className="bookmark-indicator-cover"
              label={bookmarkLabel}
            />
          )}
        </span>
        <bdi className="library-book-title">{doc.title}</bdi>
      </button>
      <span className="library-book-footer">
        <span className="library-book-format">
          {opening ? openingLabel : (doc.format ?? 'EPUB').toUpperCase()}
        </span>
        {doc.textStatus === 'recognition-required' && (
          onRecognize ? (
            <button
              type="button"
              className="library-book-recognize"
              disabled={disabled}
              title={textRecognitionRequiredLabel}
              onClick={() => void onRecognize(doc.url)}
            >
              {recognizeEnglishLabel}
            </button>
          ) : (
            <span className="library-book-text-status">
              {textRecognitionRequiredLabel}
            </span>
          )
        )}
      </span>
    </div>
  )
}

function filterGroups(groups: AuthorGroup[], include: (doc: DocumentInfo) => boolean): AuthorGroup[] {
  return groups
    .map((group) => ({ ...group, docs: group.docs.filter(include) }))
    .filter((group) => group.docs.length > 0)
}

function titleColor(title: string): number {
  let hash = 0
  for (const character of title) hash = (hash * 31 + character.codePointAt(0)!) >>> 0
  return hash % 6
}
