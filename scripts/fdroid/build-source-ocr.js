import {
  basename,
  join,
} from "node:path"
import {
  cp,
  copyFile,
  mkdir,
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

const revision = "acffef2b66eb44a31df297e11d905f4b39001068"
const upstream = await source("tesseract-source", "https://github.com/naptha/tesseract.js-core.git", revision, true)
const env = {
  ...emscriptenEnv(),
  GIT_CEILING_DIRECTORIES: SOURCE_CACHE,
}
for (const tool of ["autoconf", "automake", "libtool", "pkg-config"]) {
  output(tool, ["--version"])
}
const work = join(SOURCE_CACHE, "ocr-work")
// Upstream's autotools scripts edit tracked files; build a disposable copy so source pins stay auditable.
await rm(work, {
  recursive: true,
  force: true,
})
await cp(upstream, work, {
  recursive: true,
  filter: path => basename(path) !== ".git",
})
const variables = join(work, "build-scripts/var.sh")
const original = await readFile(variables, "utf8")
// Limit local CPU use without changing the upstream compiler flags or shipping engine variants.
await writeFile(variables, original.replace(/^PROC=.*$/m, "PROC=2"))
for (const dependency of ["zlib", "libtiff", "openlibm", "giflib", "libpng", "libjpeg", "libwebp", "leptonica"]) {
  await run("bash", ["build-scripts/build-" + dependency + ".sh"], {
    cwd: work,
    env,
  })
}
const build = join(work, "third_party/tesseract/build")
const destination = join(SOURCE_CACHE, "source-ocr")
await mkdir(destination, { recursive: true })
const files = []
for (const [variant, sse, avx] of [["lstm", 0, 0], ["simd-lstm", 1, 0], ["relaxedsimd-lstm", 1, 1]]) {
  await run("cmake", ["-S", join(work, "third_party/tesseract"), "-B", build,
    "-DCMAKE_TOOLCHAIN_FILE=" + join(env.EMSDK, "upstream/emscripten/cmake/Modules/Platform/Emscripten.cmake"),
    "-DCMAKE_PREFIX_PATH=" + join(work, "dep"), "-DLeptonica_DIR=" + join(work, "third_party/leptonica/build"),
    "-DCMAKE_CXX_FLAGS=-O3 --closure 1", "-DOPENMP_BUILD=OFF", "-DBUILD_TRAINING_TOOLS=OFF",
    "-DGRAPHICS_DISABLED=ON", "-DWASM_BUILD=ON", "-DDISABLED_LEGACY_ENGINE=1",
    "-DHAVE_SSE4_1=" + sse, "-DHAVE_AVX=" + avx, "-DHAVE_AVX2=" + avx,
  ], {
    cwd: work,
    env,
  })
  const name = "tesseract-core-" + variant
  // Build only the three LSTM variants staged by Papercut, rather than upstream's extra legacy cores.
  await run("cmake", ["--build", build, "--target", name, name + ".wasm", "--parallel", "2"], { env })
  for (const suffix of [".wasm", ".wasm.js"]) {
    const file = name + suffix
    await copyFile(join(build, "bin", file), join(destination, file))
    files.push(file)
  }
}
// Generated Emscripten loaders are CommonJS in Node; browser worker loading is unaffected by this test metadata.
await writeFile(join(destination, "package.json"), '{"type":"commonjs"}\n')
await receipt(destination, {
  tesseract: revision,
  submodules: output("git", ["submodule", "status", "--recursive"], upstream).trim(),
}, files, {
  emscripten: "4.0.15",
  cmake: output("cmake", ["--version"]).split("\n")[0],
})
await run(process.execPath, ["scripts/fdroid/__tests__/check-source-ocr.js"])
console.log("Source-built OCR cores (not staged into the app): " + destination)
