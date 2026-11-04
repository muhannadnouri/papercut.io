import assert from "node:assert/strict"
import {
  basename,
  join,
} from "node:path"
import {
  cp,
  mkdir,
  readdir,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises"
import { run } from "../lib/process.js"
import {
  source,
  output,
  receipt,
  emscriptenEnv,
  SOURCE_CACHE,
} from "./source-build.js"

// Historical wrapper pins preserve the current codecs; JBIG2's later build-only fixes leave decoder sources unchanged.
const wrappers = {
  openjpeg: "8bb19dc1aeecb104ce2ed3493d41fe131fc681e0",
  qcms: "2ae4ee72334782928210ba4050e3e77d9b2d35be",
  jbig2: "0b32db6d0eaf94844d46e38828ad4e0b7e36cb55",
  quickjs: "b62f7cd527363ca2c1fe7467f274bc9acbf78c24",
}
const libraries = {
  openjpeg: ["https://github.com/uclouvain/openjpeg.git", "6c4a29b00211eb0430fa0e5e890f1ce5c80f409f"],
  jbig2: ["https://pdfium.googlesource.com/pdfium", "0455e822ded1a5537d826703988e986a33d2d4a1"],
  quickjs: ["https://github.com/bellard/quickjs.git", "3d5e064e9dd67c70f7962836505a7fa067bf0a4e"],
}
const env = {
  ...emscriptenEnv(),
  GIT_CEILING_DIRECTORIES: SOURCE_CACHE,
}
const destination = join(SOURCE_CACHE, "source-pdf")
await mkdir(destination, { recursive: true })
const built = []
for (const [part, revision] of Object.entries(wrappers)) {
  const original = await source("pdfjs-" + part, "https://github.com/mozilla/pdf.js." + part + ".git", revision)
  const work = join(SOURCE_CACHE, "pdf-wrapper-work-" + part)
  await rm(work, {
    recursive: true,
    force: true,
  })
  await cp(original, work, {
    recursive: true,
    filter: path => basename(path) !== ".git",
  })
  if (part === "qcms") {
    // Use upstream's lock, never its Dockerfile's `cargo update` or a floating wasm-bindgen CLI.
    const bindgen = join(SOURCE_CACHE, "wasm-tools/bin/wasm-bindgen")
    assert.equal(output(bindgen, ["--version"]).trim(), "wasm-bindgen 0.2.100", "Install the locked wasm-bindgen CLI described in the guide")
    await run("cargo", ["+1.98.1", "build", "--release", "--locked", "--target", "wasm32-unknown-unknown"], {
      cwd: work,
      env: {
        ...env,
        CARGO_TARGET_DIR: join(SOURCE_CACHE, "qcms-build"),
      },
    })
    await run(bindgen, ["--target", "web", "--no-typescript", "--omit-default-module-path", "--out-name", "qcms", "--out-dir", destination,
      join(SOURCE_CACHE, "qcms-build/wasm32-unknown-unknown/release/qcms_pdf_js.wasm")], { env })
    await cp(join(work, "js"), destination, { recursive: true })
    built.push("qcms.js", "qcms_bg.wasm", "qcms_utils.js")
    continue
  }
  const [url, pin] = libraries[part]
  const originalLibrary = await source(part === "jbig2" ? "pdfium-source" : part + "-source", url, pin)
  const library = join(SOURCE_CACHE, "pdf-library-work-" + part)
  await rm(library, {
    recursive: true,
    force: true,
  })
  await cp(originalLibrary, library, {
    recursive: true,
    filter: path => basename(path) !== ".git",
  })
  for (const patch of (await readdir(work)).filter(file => file.endsWith(".patch")).sort()) {
    await run("patch", ["-p1", "--forward", "--input", join(work, patch)], { cwd: library })
  }
  // Replace only upstream's shared /tmp cache location, keeping each build's compiler cache private.
  for (const script of ["compile.sh", ...(part === "openjpeg" ? ["compile_lib.sh"] : [])]) {
    const file = join(work, script)
    await writeFile(file, (await readFile(file, "utf8")).replaceAll('/tmp/em_cache_${BUILD_TYPE}', '${PAPERCUT_EM_CACHE}/${BUILD_TYPE}'))
  }
  const buildEnv = {
    ...env,
    INPUT: join(work, "src"),
    OUTPUT: destination,
    OPENJPEG: library,
    PDFIUM: library,
    QUICKJS: library,
    PAPERCUT_EM_CACHE: join(SOURCE_CACHE, "pdf-em-cache"),
  }
  if (part === "openjpeg") {
    for (const type of ["wasm", "js"]) {
      const typeEnv = {
        ...buildEnv,
        BUILD_TYPE: type,
      }
      await run("sh", ["compile_lib.sh"], {
        cwd: work,
        env: typeEnv,
      })
      await run("sh", ["compile.sh"], {
        cwd: work,
        env: typeEnv,
      })
    }
    built.push("openjpeg.js", "openjpeg.wasm", "openjpeg_nowasm_fallback.js")
  } else if (part === "jbig2") {
    for (const type of ["wasm", "js"]) {
      await run("sh", ["compile.sh"], {
        cwd: work,
        env: {
          ...buildEnv,
          BUILD_TYPE: type,
        },
      })
    }
    built.push("jbig2.js", "jbig2.wasm", "jbig2_nowasm_fallback.js")
  } else {
    await run("sh", ["compile.sh"], {
      cwd: work,
      env: buildEnv,
    })
    built.push("quickjs-eval.js", "quickjs-eval.wasm")
  }
}
for (const file of built.filter(file => file.endsWith(".wasm"))) {
  assert(WebAssembly.validate(await readFile(join(destination, file))), "Invalid generated WASM: " + file)
}
await receipt(destination, {
  pdfjs: "6.2.108",
  wrappers,
  libraries,
}, built, {
  emscripten: "4.0.15",
  rust: "1.98.1",
  wasmBindgen: "0.2.100",
})
// PDF.js bundles codec glue into its worker; these pairs require a matching worker rebuild before app integration.
console.log("Source-built codec/loader pairs (not staged into the app): " + destination)

const pdfRevision = "0365cbde028bd92e58f2dab1bb70cd30ac7acfd7"
const pdfSource = await source("pdf.js", "https://github.com/mozilla/pdf.js.git", pdfRevision)
// PDF.js derives 6.2.108 from Git history; a one-commit checkout silently produces the wrong build number.
if (output("git", ["rev-parse", "--is-shallow-repository"], pdfSource).trim() === "true") {
  await run("git", ["fetch", "--depth=200", "origin", pdfRevision], { cwd: pdfSource })
}
output("git", ["cat-file", "-e", "eddd70a2ca1054ad2e0792972c3f2774b89f0cd2^{commit}"], pdfSource)
const worker = join(SOURCE_CACHE, "pdf-worker-work")
await rm(worker, {
  recursive: true,
  force: true,
})
await run("git", ["clone", "--shared", pdfSource, worker])
await run("git", ["checkout", "--detach", pdfRevision], { cwd: worker })
for (const part of Object.keys(wrappers)) {
  for (const name of built.filter(file => file.startsWith(part === "qcms" ? "qcms" : part))) {
    await cp(join(destination, name), join(worker, "external", part, name))
  }
}
// Build the worker with matching generated glue, rather than swapping WASM under the npm worker's old import ABI.
await run("npm", ["ci", "--ignore-scripts", "--no-audit", "--no-fund"], {
  cwd: worker,
  env: {
    ...process.env,
    PUPPETEER_SKIP_DOWNLOAD: "true",
  },
})
await run(process.execPath, ["node_modules/gulp/bin/gulp.js", "generic"], { cwd: worker })
assert.equal(JSON.parse(await readFile(join(worker, "build/version.json"), "utf8")).version, "6.2.108")
await receipt(join(worker, "build/generic"), {
  pdfjs: pdfRevision,
  wrappers,
  libraries,
},
["build/pdf.mjs", "build/pdf.worker.mjs", "build/pdf.sandbox.mjs"], {
  emscripten: "4.0.15",
  rust: "1.98.1",
  wasmBindgen: "0.2.100",
})
await run(process.execPath, ["scripts/fdroid/__tests__/check-source-pdf.js"])
