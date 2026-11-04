import assert from "node:assert/strict"
import { execFileSync } from "node:child_process"
import { createHash } from "node:crypto"
import { readFileSync } from "node:fs"
import { join } from "node:path"
import {
  fileURLToPath,
  pathToFileURL,
} from "node:url"
import { ROOT } from "../../lib/paths.js"
import { SOURCE_CACHE } from "../source-build.js"

const mode = process.argv[2]
if (!mode) {
  // Separate processes prevent PDF.js's global fake-worker cache from reusing the reference worker.
  const results = ["reference", "source"].map(choice => JSON.parse(execFileSync(process.execPath,
    [fileURLToPath(import.meta.url), choice], {
      encoding: "utf8",
      maxBuffer: 1024 * 1024,
      timeout: 120000,
    })))
  assert.deepEqual(results[1].pages, results[0].pages, "Source-built PDF rendering differs from the pinned npm reference")
  assert.deepEqual(results[1].colors, results[0].colors, "Source-built QCMS color conversions differ")
  assert(results[1].loaded.includes("openjpeg.wasm") && results[1].loaded.includes("jbig2.wasm"), "Fixtures must exercise both source-built image decoders")
  console.log("Source-built PDF worker: three pixel-identical PDF fixtures, QCMS conversions and QuickJS initialization passed")
} else {
  assert(["reference", "source"].includes(mode), "Expected reference or source mode")
  // Keep both runs on the same rasterizer so comparisons isolate the PDF/codec builds.
  const { createCanvas, DOMMatrix, ImageData, Path2D } = await import("@napi-rs/canvas")
  Object.assign(globalThis, {
    DOMMatrix,
    ImageData,
    Path2D,
  })
  // Exercise the shipping browser build on Node 24 using PDF.js's locked polyfills for newer browser APIs.
  await import(pathToFileURL(join(SOURCE_CACHE, "pdf-worker-work/node_modules/core-js/actual/typed-array/to-hex.js")))
  await import(pathToFileURL(join(SOURCE_CACHE, "pdf-worker-work/node_modules/core-js/actual/map/get-or-insert-computed.js")))
  const messages = []
  console.log = console.warn = (...args) => {
    messages.push(args.join(" "))
  }
  const base = mode === "reference" ? join(ROOT, "node_modules/pdfjs-dist") : join(SOURCE_CACHE, "pdf-worker-work/build/generic")
  const resources = mode === "reference" ? base : join(base, "web")
  const api = await import(pathToFileURL(join(base, "build/pdf.mjs")))
  assert.equal(api.version, "6.2.108")
  api.GlobalWorkerOptions.workerSrc = join(base, "build/pdf.worker.mjs")
  const loaded = []
  // Local reads record which decoders ran; a successful JS fallback must not mask broken WASM.
  class BinaryDataFactory {
    constructor(options) {
      Object.assign(this, options)
    }
    async fetch({ kind, filename }) {
      if (kind === "wasmUrl") {
        loaded.push(filename)
      }
      return new Uint8Array(readFileSync(join(this[kind], filename)))
    }
  }
  // Both PDF versions use one rasterizer, including internal image scratch canvases.
  class CanvasFactory {
    create(width, height) {
      const canvas = createCanvas(width, height)
      return {
        canvas,
        context: canvas.getContext("2d"),
      }
    }
    reset(entry, width, height) {
      entry.canvas.width = width
      entry.canvas.height = height
    }
    destroy(entry) {
      entry.canvas = null
      entry.context = null
    }
  }
  const pages = []
  for (const name of ["jp2k-resetprob.pdf", "jbig2_symbol_offset.pdf", "cmykjpeg.pdf"]) {
    const task = api.getDocument({
      data: new Uint8Array(readFileSync(join(SOURCE_CACHE, "pdf.js/test/pdfs", name))),
      standardFontDataUrl: join(resources, "standard_fonts/"),
      wasmUrl: join(resources, "wasm/"),
      useWorkerFetch: false,
      useSystemFonts: false,
      disableFontFace: true,
      BinaryDataFactory,
      CanvasFactory,
    })
    const document = await task.promise
    const page = await document.getPage(1)
    const viewport = page.getViewport({ scale: 1 })
    const canvas = createCanvas(Math.ceil(viewport.width), Math.ceil(viewport.height))
    const context = canvas.getContext("2d")
    await page.render({
      canvasContext: context,
      viewport,
    }).promise
    const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data
    pages.push({
      name,
      width: canvas.width,
      height: canvas.height,
      sha256: createHash("sha256").update(pixels).digest("hex"),
    })
    await task.destroy()
  }
  assert(!messages.some(message => /Unable to load.*wasm|failed to.*wasm|wasm.*failed/i.test(message)), messages.join("\n"))
  // QCMS is disabled by PDF.js's Node fetch path, so exercise its ICC conversion API directly as well.
  const qcmsBase = mode === "reference" ? join(SOURCE_CACHE, "pdf.js/external/qcms") : join(SOURCE_CACHE, "source-pdf")
  const qcms = await import(pathToFileURL(join(qcmsBase, "qcms.js")))
  qcms.initSync({ module: readFileSync(join(qcmsBase, "qcms_bg.wasm")) })
  const transformer = qcms.qcms_transformer_from_memory(
    readFileSync(join(ROOT, "node_modules/pdfjs-dist/iccs/CGATS001Compat-v2-micro.icc")), qcms.DataType.CMYK, qcms.Intent.Perceptual)
  assert(transformer, "Failed to initialize the ICC profile")
  const colors = [[0, 0, 0, 0], [0, 0, 0, 255], [255, 0, 0, 0], [17, 95, 31, 143]].map(color => qcms.qcms_convert_four(transformer, ...color))
  qcms.qcms_drop_transformer(transformer)
  const sandboxBase = mode === "reference" ? join(SOURCE_CACHE, "pdf.js/external/quickjs") : join(SOURCE_CACHE, "source-pdf")
  const { default: factory } = await import(pathToFileURL(join(sandboxBase, "quickjs-eval.js")))
  const sandbox = await factory({ wasmBinary: readFileSync(join(sandboxBase, "quickjs-eval.wasm")) })
  assert.equal(sandbox.ccall("init", "number", ["string", "number"], ["(() => { if (2 + 2 !== 4) throw new Error('arithmetic'); return () => {}; })()", 0]), 1)
  sandbox.ccall("nukeSandbox", null, [], [])
  process.stdout.write(JSON.stringify({
    pages,
    colors,
    loaded: [...new Set(loaded)],
    messages,
  }) + "\n")
}
