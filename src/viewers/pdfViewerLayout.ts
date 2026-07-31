interface PdfViewerLayoutTarget {
  currentScaleValue: string
  update: () => void
}

/**
 * Mirror PDF.js's resize contract while preserving explicit user zoom.
 *
 * Fit-width depends only on viewport width, while fit-page also depends on
 * height. `update()` then refreshes PDF.js's bounded visible-page queue.
 */
export function syncPdfViewerLayout(
  viewer: PdfViewerLayoutTarget,
  widthChanged: boolean,
) {
  const scale = viewer.currentScaleValue
  if (scale === 'page-fit' || (widthChanged && scale === 'page-width')) {
    viewer.currentScaleValue = scale
  }
  viewer.update()
}
