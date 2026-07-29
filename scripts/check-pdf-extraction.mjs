import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { getDocument } from 'pdfjs-dist/legacy/build/pdf.mjs'

const expectedText = [
  'P01 BASIC LATIN',
  'P02 INLINE FORMAT PRESERVED',
  'P06 LEFT 1',
  'P06 LEFT 2',
  'P06 RIGHT 1',
  'P06 RIGHT 2',
]

const path = process.argv[2]
if (!path) throw new Error('Usage: node scripts/check-pdf-extraction.mjs <fixture.pdf>')

const loadingTask = getDocument({
  data: new Uint8Array(await readFile(path)),
  standardFontDataUrl: fileURLToPath(new URL('../node_modules/pdfjs-dist/standard_fonts/', import.meta.url)),
})
const pdf = await loadingTask.promise

try {
  assert.equal(pdf.numPages, 1)
  const page = await pdf.getPage(1)
  const content = await page.getTextContent({ disableNormalization: true })
  const items = content.items.filter((item) => 'str' in item)
  const text = items
    .map((item) => item.str + (item.hasEOL ? '\n' : ''))
    .join('')
    .replace(/\s+/g, ' ')
    .trim()

  let cursor = 0
  for (const expected of expectedText) {
    const offset = text.indexOf(expected, cursor)
    assert.notEqual(offset, -1, `Missing or reordered ${JSON.stringify(expected)} in ${JSON.stringify(text)}`)
    assert.equal(
      text.split(expected).length - 1,
      1,
      `Duplicated ${JSON.stringify(expected)} in ${JSON.stringify(text)}`,
    )
    cursor = offset + expected.length
  }

  const viewport = page.getViewport({ scale: 1 })
  assert.ok(viewport.width > 0 && viewport.height > 0, 'PDF page dimensions must be positive')
  for (const item of items) {
    assert.ok(
      [...item.transform, item.width, item.height].every(Number.isFinite),
      `Non-finite PDF.js text coordinates for ${JSON.stringify(item.str)}`,
    )
  }

  page.cleanup()
  console.log(`PDF.js extraction check passed: ${path}`)
} finally {
  await loadingTask.destroy()
}
