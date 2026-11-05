import assert from "node:assert/strict"
import { createHash } from "node:crypto"
import {
  existsSync,
  readdirSync,
  statSync,
} from "node:fs"
import {
  readFile,
  writeFile,
  mkdir,
} from "node:fs/promises"
import { execFileSync } from "node:child_process"
import {
  basename,
  dirname,
  join,
  relative,
} from "node:path"
import { parseArgs } from "node:util"
import { ROOT } from "../lib/paths.js"
import { isMain } from "../lib/process.js"

// Assembles the shipped Android third-party notice file from pinned local sources.
// This is a developer-run generator: ordinary builds package the committed output
// unchanged, so regenerating is explicit and its diff is reviewable.

const CACHE = join(ROOT, ".cache/android-production")
const OUTPUT = join(ROOT, "src-tauri/licensing/android-third-party-notices.md")

// Exact pins match the source recipes and the Maven artifact pin in build.gradle.kts.
const PINS = {
  sherpa: "142807252687d81b40d6315f23470a1512a00de3",
  ort: "8f0278c77bf44b0cc83c098c6c722b92a36ac4b5",
  ortAar: "com.microsoft.onnxruntime:onnxruntime-android:1.27.0 sha256 077dec5e2d821234c7dc0aba584bec8f999854b546c754cab93a90741c56fbeb",
  tesseract: "acffef2b66eb44a31df297e11d905f4b39001068",
  pdfjs: "0365cbde028bd92e58f2dab1bb70cd30ac7acfd7",
  tessdata: "naptha/tessdata 806cd9adc8c6e8abc11c782db1818c990576bebc",
}

// Deep find is bounded: license files sit at checkout roots or vendored dirs.
export function findLicenseFiles(directory, depth = 3) {
  if (depth < 0 || !existsSync(directory)) {
    return []
  }
  const found = []
  for (const entry of readdirSync(directory)) {
    const path = join(directory, entry)
    if (!statSync(path).isFile()) {
      continue
    }
    if (/^(copying|license|notice)/i.test(entry)) {
      found.push(path)
    }
  }
  if (found.length) {
    return found
  }
  for (const entry of readdirSync(directory)) {
    const path = join(directory, entry)
    if (statSync(path).isDirectory()) {
      found.push(...findLicenseFiles(path, depth - 1))
    }
  }
  return found
}

function rel(path) {
  return relative(ROOT, path)
}

// Fixed component blocks; texts are deduplicated by content, so repeated
// licenses (MIT, Apache-2.0, BSD) render once with merged attribution.
const COMPONENTS = [
  {
    title: "GNU General Public License v3.0",
    required: true,
    entries: [{
      file: join(CACHE, "sherpa-build/_deps/espeak_ng-src/COPYING"),
      attribution: "eSpeak NG (fetched and pinned by sherpa-onnx v1.13.4 cmake)",
    }, {
      file: join(CACHE, "sherpa-build/_deps/espeak_ng-src/COPYING.UCD"),
      attribution: "eSpeak NG Unicode data notice",
    }, {
      file: join(CACHE, "sherpa-build/_deps/espeak_ng-src/src/ucd-tools/COPYING"),
      attribution: "espeak-ng ucd-tools",
    }],
  },
  {
    title: "Apache License 2.0",
    required: true,
    entries: [{
      file: join(CACHE, "sherpa-onnx/LICENSE"),
      attribution: "sherpa-onnx " + PINS.sherpa.slice(0, 12),
    }, {
      file: join(CACHE, "onnxruntime/LICENSE"),
      attribution: "ONNX Runtime source " + PINS.ort.slice(0, 12) + "; runtime AAR " + PINS.ortAar,
    }, {
      file: join(CACHE, "sherpa-build/_deps/openfst-src/COPYING"),
      attribution: "OpenFst (Sherpa fetched dependency)",
    }, {
      file: join(CACHE, "sherpa-build/_deps/kaldifst-src/LICENSE"),
      attribution: "kaldifst (Sherpa fetched dependency)",
    }, {
      file: join(CACHE, "sherpa-build/_deps/kaldi_native_fbank-src/LICENSE"),
      attribution: "kaldi-native-fbank (Sherpa fetched dependency)",
    }, {
      file: join(CACHE, "sherpa-build/_deps/kaldi_decoder-src/LICENSE"),
      attribution: "kaldi-decoder (Sherpa fetched dependency)",
    }, {
      file: join(CACHE, "sherpa-build/_deps/simple-sentencepiece-src/LICENSE"),
      attribution: "simple-sentencepiece (Sherpa fetched dependency)",
    }, {
      file: join(CACHE, "sherpa-build/_deps/espeak_ng-src/COPYING.APACHE"),
      attribution: "eSpeak NG dual-licensed components",
    }, {
      file: join(ROOT, "node_modules/pdfjs-dist/LICENSE"),
      attribution: "PDF.js " + PINS.pdfjs.slice(0, 12),
    }, {
      file: join(CACHE, "pdfjs-jbig2/LICENSE"),
      attribution: "PDF.js JBIG2 wrapper",
    }, {
      file: join(CACHE, "tesseract-source/LICENSE"),
      attribution: "Tesseract " + PINS.tesseract.slice(0, 12),
    }, {
      file: join(ROOT, "node_modules/tesseract.js/LICENSE.md"),
      attribution: "tesseract.js npm wrapper",
    }, {
      file: join(ROOT, "node_modules/tesseract.js-core/LICENSE"),
      attribution: "tesseract.js-core WASM cores",
    }, {
      file: join(ROOT, "public/fonts/reader/droid-arabic-naskh/LICENSE-APACHE-2.0.txt"),
      attribution: "Droid Sans Arabic Naskh font",
    }],
  },
  {
    title: "ONNX Runtime third-party notices",
    required: true,
    entries: [{
      file: join(CACHE, "onnxruntime/ThirdPartyNotices.txt"),
      attribution: "ONNX Runtime " + PINS.ort.slice(0, 12),
    }],
  },
  {
    title: "MIT License",
    required: true,
    notes: [
      "sentencepiece-related notices ride with the simple-sentencepiece Apache text in the Apache section.",
    ],
    entries: [{
      file: join(ROOT, "LICENSE.md"),
      attribution: "Papercut original code (repository root; the F-Droid metadata declares GPL-3.0-only for the combined Android distribution)",
    }, {
      file: join(CACHE, "sherpa-build/_deps/piper_phonemize-src/LICENSE.md"),
      attribution: "Piper phonemize (Sherpa fetched dependency)",
    }, {
      file: join(CACHE, "sherpa-build/_deps/json-src/LICENSE.MIT"),
      attribution: "nlohmann JSON (Sherpa fetched dependency)",
    }, {
      file: join(CACHE, "supertonic-license-evidence/LICENSE"),
      attribution: "Supertonic 3 sample code (Supertone Inc.); weights are separately OpenRAIL-M licensed with use restrictions — https://huggingface.co/Supertone/supertonic-3/blob/main/LICENSE — declared via the NonFreeAssets anti-feature",
    }, {
      file: join(CACHE, "sherpa-build/_deps/piper_phonemize-src/licenses/uni-algo/LICENSE.md"),
      attribution: "uni-algo (Piper phonemize dependency)",
    }, {
      file: join(CACHE, "quickjs-source/LICENSE"),
      attribution: "QuickJS (PDF.js sandbox engine)",
    }, {
      file: join(CACHE, "pdfjs-qcms/LICENSE"),
      attribution: "PDF.js QCMS wrapper (source checkout)",
    }],
  },
  {
    title: "BSD and BSD-style licenses",
    required: true,
    entries: [{
      file: join(CACHE, "sherpa-build/_deps/eigen-src/COPYING.BSD"),
      attribution: "Eigen (Sherpa fetched dependency) BSD components",
    }, {
      file: join(CACHE, "sherpa-build/_deps/eigen-src/COPYING.MINPACK"),
      attribution: "Eigen MINPACK components",
    }, {
      file: join(CACHE, "sherpa-build/_deps/kissfft-src/COPYING"),
      attribution: "KissFFT (Sherpa fetched dependency) BSD license",
    }, {
      file: join(CACHE, "pdfium-source/LICENSE"),
      attribution: "PDFium (JBIG2/CCITT decoder dependency)",
    }],
    dirEntries: [{
      dir: join(CACHE, "tesseract-source/third_party"),
      attribution: "Tesseract.js core build image/vendored libraries",
    }],
  },
  {
    title: "Mozilla Public License 2.0",
    required: true,
    entries: [{
      file: join(CACHE, "sherpa-build/_deps/eigen-src/COPYING.MPL2"),
      attribution: "Eigen (Sherpa fetched dependency) MPL2 components; MPL-covered files are provided through the corresponding-source archive",
    }],
  },
  {
    title: "SIL Open Font License",
    required: true,
    entries: [
      ["atkinson-hyperlegible", "literata", "noto-naskh-arabic", "readex-pro", "scheherazade-new"].map(family => ({
        file: join(ROOT, "public/fonts/reader", family, "OFL.txt"),
        attribution: family + " font",
      })),
    ].flat(),
  },
  {
    title: "PDF standard fonts (Foxit and Liberation)",
    required: true,
    entries: [{
      file: join(ROOT, "node_modules/pdfjs-dist/standard_fonts/LICENSE_FOXIT"),
      attribution: "Foxit PDF standard font files",
    }, {
      file: join(ROOT, "node_modules/pdfjs-dist/standard_fonts/LICENSE_LIBERATION"),
      attribution: "Liberation PDF standard font files",
    }],
  },
]

// Prose sections are verbatim owner-approved text from the review draft.
const STATEMENT = `## Combined work

This Android application incorporates eSpeak NG and Unicode tools through
Sherpa ONNX/Piper phonemization. These components carry GPLv3 terms, with
later-version permission where granted. The combined distribution is therefore
declared GPL-3.0-only in F-Droid metadata and must satisfy applicable GPL
terms. Papercut's original source retains MIT notices; other dependencies
retain their notices. Matching corresponding source and build instructions
accompany each release.

## Models and voices

- Piper Kareem (Arabic voice): disclosure — the pinned archive's model card
  points dataset licensing to AliMokhammad/arabicttstrain, which cannot be
  retrieved, and records fine-tuning from the lessac voice; repository MIT
  metadata does not settle these rights. Kept as an optional download with
  this status disclosed in the listing, pending an upstream rights statement.
- Supertonic 3: the pinned archive distinguishes MIT sample code from
  OpenRAIL-M weights that carry use restrictions. Kept as an optional download
  and declared with the F-Droid NonFreeAssets anti-feature.
- Kokoro: Apache-2.0 archive; its bundled eSpeak data/dictionaries and
  CppJieba dictionaries are covered by the GPL and MIT notices above.
- E5 embeddings (Small/Base/Large): all pinned model cards declare MIT; the
  MIT text above covers them; hash-pinned ONNX/tokenizer assets keep per-file
  notices.
- English and Arabic OCR trained data byte-match ${PINS.tessdata} and carry
  its Apache-2.0 source license, stated separately from the npm wrappers'
  MIT metadata.

## Rust, JavaScript and Maven dependencies

Per-package attributions below come from the pinned lockfiles and package
metadata; Maven dependencies (AndroidX, Material, CameraX, the ONNX Runtime
AAR and others) are declared in their POMs, and the resolved graph is
auditable with the standard refresh commands recorded in the source
inventory. License texts shared verbatim across packages are printed once
with the packages grouped beneath them.

## Corresponding source

Each release ships the exact pinned source revisions, patches, build scripts
and this notice as a corresponding-source archive with build instructions, so
the GPL terms of the combined distribution are satisfied.
`

async function kareemCard() {
  const card = await readFile(join(CACHE, "kareem-notices.txt"), "utf8")
  return "Piper Kareem model card (verbatim, from the pinned archive):\n\n" + card
}

async function componentTexts() {
  const blocks = []
  const missing = []
  for (const component of COMPONENTS) {
    const sources = []
    for (const entry of component.entries ?? []) {
      assert(existsSync(entry.file), "Missing license source: " + rel(entry.file))
      sources.push({
        text: (await readFile(entry.file, "utf8")).trimEnd() + "\n",
        attribution: entry.attribution,
      })
    }
    for (const entry of component.dirEntries ?? []) {
      for (const file of findLicenseFiles(entry.dir, 2)) {
        sources.push({
          text: (await readFile(file, "utf8")).trimEnd() + "\n",
          attribution: entry.attribution + ": " + basename(dirname(file)) + "/" + basename(file),
        })
      }
    }
    if (component.required && !sources.length) {
      missing.push(component.title)
    }
    blocks.push({
      title: component.title,
      sources,
      notes: component.notes,
    })
  }
  return {
    blocks,
    missing,
  }
}

export function groupBySha(sources) {
  const groups = new Map()
  for (const source of sources) {
    const sha = createHash("sha256").update(source.text).digest("hex")
    if (!groups.has(sha)) {
      groups.set(sha, {
        text: source.text,
        attribution: [],
      })
    }
    groups.get(sha).attribution.push(source.attribution)
  }
  return [...groups.values()]
}

async function packageGraphs() {
  const cargo = JSON.parse(execFileSync("cargo", [
    "metadata", "--manifest-path", join(ROOT, "src-tauri/Cargo.toml"), "--locked",
    "--offline", "--filter-platform", "aarch64-linux-android",
    "--features", "native-tts-shared,experimental-search-embeddings",
    "--format-version", "1",
  ], {
    cwd: ROOT,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  }))
  const rust = new Map()
  const undeclared = []
  for (const pkg of cargo.packages) {
    const key = pkg.name + " " + pkg.version
    if (rust.has(key)) {
      continue
    }
    if (!pkg.license) {
      undeclared.push(key)
      continue
    }
    rust.set(key, pkg.license)
  }

  const lock = JSON.parse(await readFile(join(ROOT, "package-lock.json"), "utf8"))
  const npm = new Map()
  for (const [path, info] of Object.entries(lock.packages ?? {})) {
    if (!path.startsWith("node_modules/") || info.dev) {
      continue
    }
    const dir = join(ROOT, path)
    // Optional platform-specific packages may not be installed on this host.
    let license = "not installed on this host"
    if (existsSync(join(dir, "package.json"))) {
      const manifest = JSON.parse(await readFile(join(dir, "package.json"), "utf8"))
      license = manifest.license ?? "unknown"
    }
    npm.set(path.slice("node_modules/".length) + " " + info.version, license)
  }
  return {
    rust,
    npm,
    undeclared,
  }
}

export function packageAttribution(graph, kind) {
  const groups = new Map()
  for (const [pkg, license] of graph) {
    const id = license || "(no declared license)"
    if (!groups.has(id)) {
      groups.set(id, [])
    }
    groups.get(id).push(pkg)
  }
  return [...groups.entries()]
    .sort((a, b) => a[0].localeCompare(b[0]))
    .map(([id, pkgs]) => "- " + kind + " packages under " + id + ": " + pkgs.sort().join(", "))
}

async function main() {
  const { values } = parseArgs({ options: { check: { type: "boolean" } } })
  const [{ blocks, missing }, graphs, kareem] = await Promise.all([componentTexts(), packageGraphs(), kareemCard()])
  assert(!missing.length, "Required license sources missing: " + missing.join(", "))

  const out = []
  out.push("# Android third-party notices\n")
  out.push("Generated by `scripts/android/prepare-license-notices.js` from pinned")
  out.push("sources; ordinary builds package this committed file unchanged. Pins:")
  out.push("Sherpa " + PINS.sherpa + ", ORT " + PINS.ort + ", ORT AAR " + PINS.ortAar + ",")
  out.push("Tesseract " + PINS.tesseract + ", PDF.js " + PINS.pdfjs + ".\n")
  out.push(STATEMENT.trimEnd())
  out.push("\n## Piper Kareem model card\n")
  out.push(kareem.trimEnd())

  for (const block of blocks) {
    out.push("\n## " + block.title + "\n")
    for (const note of block.notes ?? []) {
      out.push("Note: " + note + "\n")
    }
    for (const group of groupBySha(block.sources)) {
      out.push("Attribution:\n")
      for (const line of group.attribution) {
        out.push("- " + line)
      }
      out.push("\n```\n" + group.text.trimEnd() + "\n```\n")
    }
  }

  out.push("\n## Package attribution by declared license\n")
  const rustLines = packageAttribution(graphs.rust, "Rust")
  const npmLines = packageAttribution(graphs.npm, "npm")
  out.push(...rustLines, ...npmLines)
  if (graphs.undeclared.length) {
    out.push("- Rust packages with no declared license in their manifests (repository-original code in the repository's MIT scope per the Android guide): "
      + graphs.undeclared.sort().join(", "))
  }

  const text = out.join("\n")
  if (values.check) {
    const existing = await readFile(OUTPUT, "utf8")
    assert.equal(existing, text, "Notices file is stale; rerun npm run prepare:license-notices")
    console.log("Notices file is current: " + rel(OUTPUT))
    return
  }
  await mkdir(dirname(OUTPUT), { recursive: true })
  await writeFile(OUTPUT, text)
  console.log("Wrote " + rel(OUTPUT) + " (" + text.length + " bytes, " + text.split("\n").length + " lines)")
}

if (isMain(import.meta.url)) {
  await main()
}
