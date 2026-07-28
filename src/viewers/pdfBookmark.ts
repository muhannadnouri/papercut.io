import type {
  EventBus,
  PDFViewer as PdfJsViewer,
} from 'pdfjs-dist/legacy/web/pdf_viewer.mjs'
import type { ViewerBookmarkApi, ViewerBookmarkLocation } from './types'

type ViewAreaEvent = {
  location: ViewerBookmarkLocation
}

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
  let currentLocation: ViewerBookmarkLocation = {
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
    isCurrent: (location) => (
      currentLocation.pageNumber === location.pageNumber &&
      Math.abs(currentLocation.left - location.left) <= 72 &&
      Math.abs(currentLocation.top - location.top) <= 72
    ),
    isPastStart: () => pdfViewer.currentPageNumber > 1 || container.scrollTop > 180,
    restore: (location) => {
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
