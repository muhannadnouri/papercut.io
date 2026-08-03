import assert from 'node:assert/strict'
import { deflateSync } from 'node:zlib'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'

const fixtureDir = fileURLToPath(new URL('./fixtures/pdf/', import.meta.url))
const checkOnly = process.argv.includes('--check')

function stream(dictionary, bytes) {
  return Buffer.concat([
    Buffer.from(`<<${dictionary}/Length ${bytes.length}>>\nstream\n`, 'ascii'),
    bytes,
    Buffer.from('\nendstream', 'ascii'),
  ])
}

function checkerboardImage() {
  const width = 24
  const height = 32
  const pixels = Buffer.alloc(width * height * 3)
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const offset = (y * width + x) * 3
      const dark = (Math.floor(x / 3) + Math.floor(y / 4)) % 2 === 0
      pixels.fill(dark ? 35 : 220, offset, offset + 3)
    }
  }
  return { width, height, bytes: deflateSync(pixels) }
}

/** Build a deterministic PDF without adding a fixture-generation dependency. */
function buildPdf(pageKinds) {
  const objects = [null]
  const reserve = () => objects.push(null) - 1
  const set = (id, value) => {
    objects[id] = Buffer.isBuffer(value) ? value : Buffer.from(value, 'ascii')
  }
  const catalogId = reserve()
  const pagesId = reserve()
  const fontId = reserve()
  const imageId = reserve()
  const image = checkerboardImage()

  set(fontId, '<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>')
  set(imageId, stream(
    `/Type/XObject/Subtype/Image/Width ${image.width}/Height ${image.height}`
      + '/ColorSpace/DeviceRGB/BitsPerComponent 8/Filter/FlateDecode',
    image.bytes,
  ))

  const pageIds = []
  for (const kind of pageKinds) {
    const pageId = reserve()
    const contentId = reserve()
    const nativeText = 'P10 native text remains searchable without recognition on this page.'
    const commands = {
      blank: '',
      image: 'q 420 0 0 560 96 116 cm /Im1 Do Q',
      native: `BT /F1 14 Tf 72 700 Td (${nativeText}) Tj ET`,
      'sparse-overlay': 'q 420 0 0 560 96 116 cm /Im1 Do Q BT /F1 1 Tf 2 2 Td (x x) Tj ET',
    }[kind]
    assert.notEqual(commands, undefined, `Unknown page kind: ${kind}`)
    set(contentId, stream('', Buffer.from(commands, 'ascii')))
    set(
      pageId,
      `<</Type/Page/Parent ${pagesId} 0 R/MediaBox[0 0 612 792]`
        + `/Resources<</Font<</F1 ${fontId} 0 R>>/XObject<</Im1 ${imageId} 0 R>>>>`
        + `/Contents ${contentId} 0 R>>`,
    )
    pageIds.push(pageId)
  }

  set(pagesId, `<</Type/Pages/Count ${pageIds.length}/Kids[${pageIds.map((id) => `${id} 0 R`).join(' ')}]>>`)
  set(catalogId, `<</Type/Catalog/Pages ${pagesId} 0 R>>`)

  const chunks = [Buffer.from('%PDF-1.7\n%\xE2\xE3\xCF\xD3\n', 'binary')]
  const offsets = [0]
  for (let id = 1; id < objects.length; id += 1) {
    offsets[id] = chunks.reduce((total, chunk) => total + chunk.length, 0)
    chunks.push(Buffer.from(`${id} 0 obj\n`, 'ascii'), objects[id], Buffer.from('\nendobj\n', 'ascii'))
  }
  const xrefOffset = chunks.reduce((total, chunk) => total + chunk.length, 0)
  const xref = [
    `xref\n0 ${objects.length}\n`,
    '0000000000 65535 f \n',
    ...offsets.slice(1).map((offset) => `${String(offset).padStart(10, '0')} 00000 n \n`),
    `trailer\n<</Size ${objects.length}/Root ${catalogId} 0 R>>\nstartxref\n${xrefOffset}\n%%EOF\n`,
  ].join('')
  chunks.push(Buffer.from(xref, 'ascii'))
  return Buffer.concat(chunks)
}

const fixtures = [
  ['ocr-image-only.pdf', buildPdf(['image'])],
  ['ocr-hybrid.pdf', buildPdf(['native', 'image', 'blank', 'sparse-overlay'])],
]

await mkdir(fixtureDir, { recursive: true })
for (const [name, bytes] of fixtures) {
  const path = `${fixtureDir}${name}`
  if (checkOnly) {
    assert.deepEqual(await readFile(path), bytes, `${name} is stale; regenerate the OCR fixtures`)
  } else {
    await writeFile(path, bytes)
  }
}

console.log(`${checkOnly ? 'Verified' : 'Generated'} ${fixtures.length} OCR readiness fixtures`)
