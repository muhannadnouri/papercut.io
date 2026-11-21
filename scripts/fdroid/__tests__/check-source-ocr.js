import assert from "node:assert/strict"
import { createRequire } from "node:module"
import { readFileSync } from "node:fs"
import { join } from "node:path"
import { gunzipSync } from "node:zlib"
import { ROOT } from "../../lib/paths.js"
import { SOURCE_CACHE } from "../source-build.js"

const require = createRequire(import.meta.url)
// Check every shipped core against actual OCR output; successful WASM compilation alone misses loader/data ABI errors.
for (const variant of ["lstm", "simd-lstm", "relaxedsimd-lstm"]) {
  const core = require(join(SOURCE_CACHE, "source-ocr/tesseract-core-" + variant + ".wasm.js"))
  const module = await core()
  const api = new module.TessBaseAPI()
  try {
    for (const language of ["eng", "ara"]) {
      const data = readFileSync(join(ROOT, "node_modules/@tesseract.js-data", language, "4.0.0_best_int", language + ".traineddata.gz"))
      module.FS.writeFile(language + ".traineddata", gunzipSync(data))
      assert.equal(api.Init(null, language, 1, undefined), 0)
      const image = language === "eng"
        ? join(SOURCE_CACHE, "tesseract-source/tests/unit/assets/images/small.png")
        : join(ROOT, "scripts/fixtures/pdf/source-ocr-arabic.png")
      if (language === "ara") {
        api.SetVariable("tessedit_pageseg_mode", "7")
      }
      module.FS.writeFile("/input", readFileSync(image))
      api.SetImageFile()
      assert.equal(api.GetUTF8Text().trim(), language === "eng" ? "Tesseract.js" : "مرحبا بكم في عالم القراءة")
      api.End()
      console.log(variant + " / " + language + ": OCR fixture passed")
    }
  } finally {
    module.destroy(api)
  }
}
