import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { getDocument, OPS } from 'pdfjs-dist/legacy/build/pdf.mjs'

const fixtureDir = new URL('./fixtures/pdf/', import.meta.url)
const manifest = JSON.parse(await readFile(new URL('ocr-readiness.json', fixtureDir), 'utf8'))
assert.equal(manifest.schemaVersion, 1)
assert.ok(manifest.fixtures.length <= 20, 'OCR readiness corpus exceeds the 20-fixture limit')

const maxFixtureBytes = 5 * 1024 * 1024
const maxFixturePages = 20

const imageOperations = new Set([
  OPS.paintImageMaskXObject,
  OPS.paintImageMaskXObjectGroup,
  OPS.paintImageMaskXObjectRepeat,
  OPS.paintImageXObject,
  OPS.paintImageXObjectRepeat,
  OPS.paintInlineImageXObject,
  OPS.paintInlineImageXObjectGroup,
])

/** Measure bounded signals only; production policy waits for real-corpus evidence. */
function classifyPage(textItems, operatorList, limits) {
  const text = textItems.map((item) => item.str).join(' ')
  const characters = [...text].filter((character) => !/\s/u.test(character))
  const words = text.trim() ? text.trim().split(/\s+/u) : []
  const alphanumeric = characters.filter((character) => /[\p{L}\p{N}]/u.test(character)).length
  const replacements = characters.filter((character) => character === '\uFFFD').length
  const alphanumericRatio = characters.length ? alphanumeric / characters.length : 0
  const replacementRatio = characters.length ? replacements / characters.length : 0
  const images = operatorList.fnArray.filter((operation) => imageOperations.has(operation)).length
  const usableText = characters.length >= limits.minimumCharacters
    && words.length >= limits.minimumWords
    && alphanumericRatio >= limits.minimumAlphanumericRatio
    && replacementRatio <= limits.maximumReplacementRatio

  let classification = 'review-required'
  if (usableText) classification = 'native-text'
  else if (images > 0) classification = 'recognition-required'
  else if (characters.length === 0) classification = 'blank'

  return {
    classification,
    characters: characters.length,
    words: words.length,
    images,
    alphanumericRatio: Number(alphanumericRatio.toFixed(3)),
    replacementRatio: Number(replacementRatio.toFixed(3)),
  }
}

for (const fixture of manifest.fixtures) {
  const fixtureBytes = await readFile(new URL(fixture.path, fixtureDir))
  assert.ok(fixtureBytes.length <= maxFixtureBytes, `${fixture.id} exceeds the 5 MB fixture limit`)
  const fixtureStartedAt = performance.now()
  const loadingTask = getDocument({
    data: new Uint8Array(fixtureBytes),
    standardFontDataUrl: fileURLToPath(new URL('../node_modules/pdfjs-dist/standard_fonts/', import.meta.url)),
  })
  const pdf = await loadingTask.promise
  try {
    assert.ok(pdf.numPages <= maxFixturePages, `${fixture.id} exceeds the 20-page fixture limit`)
    assert.equal(pdf.numPages, fixture.pages.length, `${fixture.id} page count`)
    for (let pageNumber = 1; pageNumber <= pdf.numPages; pageNumber += 1) {
      const page = await pdf.getPage(pageNumber)
      const [textContent, operatorList] = await Promise.all([
        page.getTextContent({ disableNormalization: true }),
        page.getOperatorList(),
      ])
      const textItems = textContent.items.filter((item) => 'str' in item)
      const result = classifyPage(textItems, operatorList, manifest.classifier)
      const expected = fixture.pages[pageNumber - 1]
      assert.equal(
        result.classification,
        expected.expected,
        `${fixture.id} page ${pageNumber} (${expected.content}): ${JSON.stringify(result)}`,
      )
      console.log(`${fixture.id} page ${pageNumber}: ${result.classification}`, result)
      page.cleanup()
    }
    console.log(`${fixture.id}: ${Math.round(performance.now() - fixtureStartedAt)} ms total`)
  } finally {
    await loadingTask.destroy()
  }
}

console.log('PDF OCR readiness benchmark passed')
