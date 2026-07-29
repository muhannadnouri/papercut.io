export const PDF_MIN_ZOOM = 25
export const PDF_MAX_ZOOM = 400

export function clampPdfPage(input: string, currentPage: number, pages: number) {
  const parsed = Number.parseInt(input, 10)
  const value = Number.isNaN(parsed) ? currentPage : parsed
  return Math.min(pages, Math.max(1, value))
}

export function clampPdfZoom(input: string, currentZoom: number) {
  const parsed = Number.parseInt(input, 10)
  const value = Number.isNaN(parsed) ? currentZoom : parsed
  return Math.min(PDF_MAX_ZOOM, Math.max(PDF_MIN_ZOOM, Math.round(value)))
}
