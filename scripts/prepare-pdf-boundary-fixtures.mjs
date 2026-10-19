import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { copyFile, mkdir, open, readFile, stat, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { getDocument, InvalidPDFException, PasswordException } from 'pdfjs-dist/legacy/build/pdf.mjs'

const OUTPUT_DIR = join(tmpdir(), 'papercut-pdf-boundary-fixtures')
const MAX_PDF_BYTES = 250 * 1024 * 1024
const PASSWORD_FIXTURE = {
  url: 'https://raw.githubusercontent.com/mozilla/pdf.js/master/test/pdfs/pr6531_2.pdf',
  sha256: 'e85d22b832a61be1d302a811e29c5df5ccd2a3795633178449d0b2e0e2451118',
}

await mkdir(OUTPUT_DIR, { recursive: true })

const passwordPath = join(OUTPUT_DIR, 'password-protected.pdf')
const malformedPath = join(OUTPUT_DIR, 'malformed.pdf')
const pageLimitPath = join(OUTPUT_DIR, 'over-2000-pages.pdf')
const sizeLimitPath = join(OUTPUT_DIR, 'over-250mb.pdf')
const validPath = join(OUTPUT_DIR, 'valid.pdf')

await downloadVerified(PASSWORD_FIXTURE, passwordPath)
await writeFile(malformedPath, '%PDF-1.7\n1 0 obj\n<< /Type /Catalog')
await writeFile(pageLimitPath, blankPdf(2_001))
await writeSparsePdf(sizeLimitPath, MAX_PDF_BYTES + 1)
await copyFile(new URL('./fixtures/pdf/extraction-inline-columns.pdf', import.meta.url), validPath)

await expectPdfError(passwordPath, PasswordException)
await expectPdfError(malformedPath, InvalidPDFException)
assert.equal(await pdfPageCount(pageLimitPath), 2_001)
assert.equal((await stat(sizeLimitPath)).size, MAX_PDF_BYTES + 1)
assert.equal(await pdfPageCount(validPath), 1)

console.log(`Prepared and verified PDF boundary fixtures in ${OUTPUT_DIR}`)
for (const path of [passwordPath, malformedPath, pageLimitPath, sizeLimitPath, validPath]) {
  console.log(`- ${path}`)
}

/** Download the canonical encrypted fixture and reject upstream drift. */
async function downloadVerified(fixture, destination) {
  const response = await fetch(fixture.url)
  if (!response.ok) throw new Error(`Failed to download ${fixture.url}: HTTP ${response.status}`)
  const bytes = Buffer.from(await response.arrayBuffer())
  const sha256 = createHash('sha256').update(bytes).digest('hex')
  assert.equal(sha256, fixture.sha256, `Unexpected SHA-256 for ${fixture.url}`)
  await writeFile(destination, bytes)
}

/** Build a compact standards-compliant PDF whose blank pages exercise only the page cap. */
function blankPdf(pageCount) {
  const objects = [
    '<< /Type /Catalog /Pages 2 0 R >>',
    `<< /Type /Pages /Count ${pageCount} /Kids [${Array.from(
      { length: pageCount },
      (_, index) => `${index + 3} 0 R`,
    ).join(' ')}] >>`,
    ...Array.from(
      { length: pageCount },
      () => '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << >> >>',
    ),
  ]
  let pdf = '%PDF-1.7\n'
  const offsets = [0]
  for (let index = 0; index < objects.length; index += 1) {
    offsets.push(Buffer.byteLength(pdf))
    pdf += `${index + 1} 0 obj\n${objects[index]}\nendobj\n`
  }
  const xref = Buffer.byteLength(pdf)
  pdf += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`
  pdf += offsets.slice(1).map((offset) => `${String(offset).padStart(10, '0')} 00000 n \n`).join('')
  pdf += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`
  return pdf
}

/** Create a logically oversized sparse file without consuming 250 MB of repository storage. */
async function writeSparsePdf(path, bytes) {
  const file = await open(path, 'w')
  try {
    await file.write('%PDF-1.7\n')
    await file.truncate(bytes)
  } finally {
    await file.close()
  }
}

async function pdfPageCount(path) {
  const task = getDocument({ data: new Uint8Array(await readFile(path)) })
  try {
    return (await task.promise).numPages
  } finally {
    await task.destroy()
  }
}

async function expectPdfError(path, ErrorType) {
  const task = getDocument({ data: new Uint8Array(await readFile(path)) })
  try {
    await assert.rejects(task.promise, ErrorType)
  } finally {
    await task.destroy()
  }
}
