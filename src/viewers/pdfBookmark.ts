import type {
  EventBus,
  PDFViewer as PdfJsViewer,
} from 'pdfjs-dist/legacy/web/pdf_viewer.mjs'
import type {
  PdfBookmarkLocation,
  ViewerBookmarkApi,
  ViewerBookmarkLocation,
} from './types'

type ViewAreaEvent = {
  location: PdfBookmarkLocation
}

const BOOKMARK_VISIBILITY_MARGIN_PX = 8

/**
 * Keep bookmark state in PDF page coordinates reported by PDF.js.
 *
 * Subscribers share this tracker, so removing one UI listener must not detach
 * the underlying `updateviewarea` listener needed by the remaining controls.
 */
export function createPdfBookmarkApi(
  pdfViewer: PdfJsViewer,
  container: HTMLElement,
  eventBus: EventBus,
): ViewerBookmarkApi {
  let currentLocation: PdfBookmarkLocation = {
    pageNumber: pdfViewer.currentPageNumber,
    left: 0,
    top: 0,
  }
  const listeners = new Set<() => void>()
  const updateLocation = ({ location }: ViewAreaEvent) => {
    currentLocation = {
      pageNumber: location.pageNumber,
      left: location.left,
      top: location.top,
    }
    listeners.forEach((listener) => listener())
  }
  eventBus.on('updateviewarea', updateLocation)
  pdfViewer.update()

  return {
    capture: () => currentLocation,
    isCurrent: (location) => {
      if (!isPdfLocation(location)) return false
      const pageView = pdfViewer.getPageView(location.pageNumber - 1)
      if (!pageView) return false
      const [, pointY] = pageView.viewport.convertToViewportPoint(location.left, location.top)
      const pageBounds = pageView.div.getBoundingClientRect()
      const containerBounds = container.getBoundingClientRect()
      const bookmarkY = pageBounds.top + pointY
      return bookmarkY >= containerBounds.top - BOOKMARK_VISIBILITY_MARGIN_PX &&
        bookmarkY <= containerBounds.bottom + BOOKMARK_VISIBILITY_MARGIN_PX
    },
    isPastStart: () => pdfViewer.currentPageNumber > 1 || container.scrollTop > 180,
    restore: (location) => {
      if (!isPdfLocation(location)) return
      pdfViewer.scrollPageIntoView({
        pageNumber: Math.min(pdfViewer.pagesCount, Math.max(1, location.pageNumber)),
        destArray: [null, { name: 'XYZ' }, location.left, location.top, null],
        ignoreDestinationZoom: true,
      })
    },
    scrollToTop: () => {
      pdfViewer.currentPageNumber = 1
      container.scrollTo({ top: 0, behavior: 'smooth' })
    },
    subscribe: (listener) => {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
  }
}

function isPdfLocation(location: ViewerBookmarkLocation): location is PdfBookmarkLocation {
  return 'pageNumber' in location
}
