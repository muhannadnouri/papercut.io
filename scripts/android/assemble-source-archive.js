import assert from "node:assert/strict"
import {
  cp,
  mkdir,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises"
import { existsSync } from "node:fs"
import {
  basename,
  dirname,
  join,
} from "node:path"
import { parseArgs } from "node:util"
import { ROOT } from "../lib/paths.js"
import {
  run,
  isMain,
} from "../lib/process.js"
import { downloadFile } from "../lib/download.js"
import {
  output,
  source,
} from "../fdroid/source-build.js"

// Assembles the per-release corresponding-source archive required by the GPL
// portions of the combined Android distribution: the pinned eSpeak NG (with
// ucd-tools) and Piper phonemize archives fetched by Sherpa, the MPL-covered
// Rust crate sources, the shipped notices file and a manifest pointing at the
// release tag for the application source. Apache/MIT/BSD components carry no
// source obligation and are fetchable from public registries via the locks.
// Developer-run at release time; the archive is attached to the release by the owner.

const CACHE = join(ROOT, ".cache/android-production")
// Staging stays a subdirectory of the output folder so tar never archives its own output.
const STAGING = join(CACHE, "source-archive/work")
const PINS = {
  sherpa: "142807252687d81b40d6315f23470a1512a00de3",
  ortAar: "com.microsoft.onnxruntime:onnxruntime-android:1.27.0 sha256 077dec5e2d821234c7dc0aba584bec8f999854b546c754cab93a90741c56fbeb",
}

// Sherpa's cmake files are the authoritative pin source for the GPL archives.
export function extractArchivePins(cmakeText, component) {
  const url = cmakeText.match(new RegExp(String.raw`set\(${component}_URL\s+"([^"]+)"`))
  const hash = cmakeText.match(new RegExp(String.raw`set\(${component}_HASH\s+"SHA256=([a-f0-9]{64})"`))
  assert(url && hash, `${component} archive pin not found in Sherpa cmake`)
  return {
    url: url[1],
    sha256: hash[1],
  }
}

async function mplCrates() {
  const cargo = JSON.parse(output("cargo", [
    "metadata", "--manifest-path", join(ROOT, "src-tauri/Cargo.toml"), "--locked",
    "--offline", "--filter-platform", "aarch64-linux-android",
    "--features", "native-tts-shared,experimental-search-embeddings",
    "--format-version", "1",
  ]))
  const seen = new Set()
  const crates = []
  for (const pkg of cargo.packages) {
    const key = pkg.name + " " + pkg.version
    if (!pkg.license?.includes("MPL") || seen.has(key)) {
      continue
    }
    seen.add(key)
    const dir = dirname(pkg.manifest_path)
    assert(existsSync(dir), "MPL crate source not in local registry: " + dir)
    crates.push({
      name: key,
      dir,
    })
  }
  return crates
}

async function main() {
  const { values } = parseArgs({ options: { out: { type: "string" } } })
  const version = JSON.parse(await readFile(join(ROOT, "package.json"), "utf8")).version
  const commit = output("git", ["rev-parse", "HEAD"]).trim()

  const sherpa = await source("sherpa-onnx", "https://github.com/k2-fsa/sherpa-onnx.git", PINS.sherpa)
  const pinSpecs = [
    {
      component: "espeak_ng",
      label: "eSpeak NG (includes ucd-tools)",
      cmake: "espeak-ng-for-piper.cmake",
    },
    {
      component: "piper_phonemize",
      label: "Piper phonemize",
      cmake: "piper-phonemize.cmake",
    },
  ]
  const pins = []
  for (const spec of pinSpecs) {
    pins.push({
      component: spec.component,
      label: spec.label,
      ...extractArchivePins(await readFile(join(sherpa, "cmake", spec.cmake), "utf8"), spec.component),
    })
  }

  await rm(STAGING, {
    recursive: true,
    force: true,
  })
  await mkdir(join(STAGING, "gpl-archives"), { recursive: true })
  for (const pin of pins) {
    const archive = join(STAGING, "gpl-archives", basename(pin.url))
    await downloadFile({
      url: pin.url,
      dest: archive,
      sha256: pin.sha256,
      label: pin.component,
    })
  }

  const crates = await mplCrates()
  await mkdir(join(STAGING, "mpl-crates"), { recursive: true })
  for (const crate of crates) {
    await cp(crate.dir, join(STAGING, "mpl-crates", basename(crate.dir)), { recursive: true })
  }

  await mkdir(join(STAGING, "notices"), { recursive: true })
  await cp(join(ROOT, "src-tauri/licensing/android-third-party-notices.md"),
    join(STAGING, "notices/android-third-party-notices.md"))
  for (const lock of ["src-tauri/Cargo.lock", "package-lock.json"]) {
    await cp(join(ROOT, lock), join(STAGING, basename(lock)))
  }

  await writeFile(join(STAGING, "MANIFEST.md"), `# Papercut Android ${version} corresponding source

Application source: papercut.io repository at commit ${commit} (attach this
archive to the matching release tag). Build instructions: the source recipes
and build sections of docs/android-production-packaging.md in that commit.

GPL components (verbatim pinned archives fetched by Sherpa's cmake):

${pins.map(pin => `- ${pin.label}: ${pin.url} sha256 ${pin.sha256}`).join("\n")}

MPL-2.0 Rust crate sources (complete crate trees):

${crates.map(crate => "- " + crate.name).join("\n") || "- none"}

Everything else: ONNX Runtime ships from the pinned Maven artifact
(${PINS.ortAar}, Apache-2.0 — notices, not source, are required); Rust, npm
and Maven components are Apache/MIT/BSD-licensed and are fetched exactly by
the committed Cargo.lock and package-lock.json from crates.io, npm and Maven
Central. The shipped notice file is in notices/.
`)

  const out = values.out ?? join(CACHE, "source-archive", `papercut-android-${version}-sources.tar.bz2`)
  await mkdir(dirname(out), { recursive: true })
  // Deterministic flags keep repeated runs byte-identical for a given manifest.
  await run("tar", ["-cjf", out, "--sort=name", "--mtime=UTC 1970-01-01", "--owner=0", "--group=0", "-C", STAGING, "."])
  console.log("Corresponding-source archive: " + out)
}

if (isMain(import.meta.url)) {
  await main()
}
