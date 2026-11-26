import { createHash } from "node:crypto"
import {
  cp,
  link,
  mkdir,
  readFile,
  realpath,
  rm,
  stat,
  writeFile,
} from "node:fs/promises"
import { join } from "node:path"

import { source } from "../../fdroid/source-build.js"
import { PIPER_SOURCE_REVISION } from "../constants.js"
import { downloadFile } from "../download.js"
import { ROOT } from "../paths.js"
import { run } from "../process.js"
import {
  ORT_IOS_ARCHIVE,
  ORT_IOS_URL,
  ORT_IOS_SHA256,
  SHERPA_IOS_ARCHIVE,
  SHERPA_IOS_DEFAULT_SLICE,
  SHERPA_IOS_DEVICE_SLICE,
  SHERPA_IOS_ONNXRUNTIME_XCFRAMEWORK,
  SHERPA_IOS_RUST_LIBS,
  SHERPA_IOS_RUNTIME_ROOT,
  SHERPA_IOS_SHA256,
  SHERPA_IOS_SHERPA_XCFRAMEWORK,
  SHERPA_IOS_SIMULATOR_ARM64_SLICE,
  SHERPA_IOS_SIMULATOR_UPSTREAM_SLICE,
  SHERPA_IOS_URL,
} from "./constants.js"

export function iosSherpaLibDir(slice = SHERPA_IOS_DEFAULT_SLICE) {
  return join(SHERPA_IOS_RUNTIME_ROOT, "cargo-libs", slice)
}

// Download and prepare the official iOS XCFramework archive for Cargo.
// macOS also prepares the simulator slice because only macOS has `lipo`.
export async function ensureIosSherpaLibs({ force = false, includeSimulator = process.platform === "darwin" } = {}) {
  await mkdir(SHERPA_IOS_RUNTIME_ROOT, { recursive: true })
  if (force) {
    await rm(SHERPA_IOS_RUNTIME_ROOT, {
      recursive: true,
      force: true,
    })
    await mkdir(SHERPA_IOS_RUNTIME_ROOT, { recursive: true })
  }

  if (!await hasIosArchiveLibs()) {
    await downloadFile({
      url: SHERPA_IOS_URL,
      dest: SHERPA_IOS_ARCHIVE,
      force,
      sha256: SHERPA_IOS_SHA256,
      label: "sherpa-ios-libs",
    })
    await extractIosArchive()
    await rm(SHERPA_IOS_ARCHIVE, { force: true })
  }

  if (!await hasIosArchiveLibs()) {
    throw new Error("Downloaded sherpa-onnx iOS archive is missing required XCFramework static libraries")
  }

  const piper = await piperFramework()
  await syncDeviceCargoLibs(piper)
  await verifyCargoLibs(SHERPA_IOS_DEVICE_SLICE)
  if (includeSimulator) {
    await syncSimulatorArm64CargoLibs(piper)
    await verifyCargoLibs(SHERPA_IOS_SIMULATOR_ARM64_SLICE)
  } else {
    console.log("[sherpa-ios-libs] simulator thinning skipped; lipo is only available on macOS")
  }
  console.log("[sherpa-ios-libs] ready " + join(SHERPA_IOS_RUNTIME_ROOT, "cargo-libs"))
}

// Pin the extracted bytes too: a restored directory or empty library is not a verified archive cache.
async function hasIosArchiveLibs() {
  try {
    const receipt = JSON.parse(await readFile(join(SHERPA_IOS_RUNTIME_ROOT, ".archive-receipt.json"), "utf8"))
    return receipt.sherpa === SHERPA_IOS_SHA256 && receipt.ort === ORT_IOS_SHA256 &&
      JSON.stringify(receipt.files) === JSON.stringify(await archiveFileHashes())
  } catch {
    return false
  }
}

async function archiveFileHashes() {
  const hashes = []
  for (const slice of [SHERPA_IOS_DEVICE_SLICE, SHERPA_IOS_SIMULATOR_UPSTREAM_SLICE]) {
    for (const file of [sherpaArchiveLib(slice), onnxRuntimeArchiveLib(slice)]) {
      const bytes = await readFile(file)
      if (!bytes.length) {
        throw new Error("Empty iOS native archive: " + file)
      }
      hashes.push(createHash("sha256").update(bytes).digest("hex"))
    }
  }
  return hashes
}

async function extractIosArchive() {
  await run("unzip", ["-o", SHERPA_IOS_ARCHIVE, "-d", SHERPA_IOS_RUNTIME_ROOT])
  await downloadFile({
    url: ORT_IOS_URL,
    dest: ORT_IOS_ARCHIVE,
    sha256: ORT_IOS_SHA256,
    label: "ort-ios-libs",
  })
  await run("unzip", ["-o", ORT_IOS_ARCHIVE, "-d", SHERPA_IOS_RUNTIME_ROOT])
  await writeFile(join(SHERPA_IOS_RUNTIME_ROOT, ".archive-receipt.json"), JSON.stringify({
    sherpa: SHERPA_IOS_SHA256,
    ort: ORT_IOS_SHA256,
    files: await archiveFileHashes(),
  }) + "\n")
}

// Upstream's pinned recipe produces the core/eSpeak/ucd aggregate without any local C++ changes.
async function piperFramework() {
  if (process.platform !== "darwin") {
    throw new Error("Preparing the native Piper iOS archive requires macOS/Xcode")
  }
  const checkout = await source("piper-phonemize-1.5.9", "https://github.com/csukuangfj/piper-phonemize.git", PIPER_SOURCE_REVISION)
  const framework = join(checkout, "build-ios/piper-phonemize.xcframework")
  // Source identity is checked above on every build; the upstream script rebuilds missing outputs with CMake.
  await run("bash", [join(ROOT, "scripts/ios/build-piper.sh"), checkout], { cwd: checkout })
  return framework
}

// The sherpa device archive is thin, but ORT 1.28.2 wraps its arm64 static archive
// in a one-architecture Mach-O universal container that Cargo cannot link.
async function syncDeviceCargoLibs(piper) {
  const target = await resetCargoLibDir(SHERPA_IOS_DEVICE_SLICE)
  await linkOrCopy(sherpaArchiveLib(SHERPA_IOS_DEVICE_SLICE), join(target, "libsherpa-onnx.a"))
  await thinArchive(onnxRuntimeArchiveLib(SHERPA_IOS_DEVICE_SLICE), join(target, "libonnxruntime.a"), "arm64")
  await thinArchive(join(piper, SHERPA_IOS_DEVICE_SLICE, "PiperPhonemizeC.framework/PiperPhonemizeC"), join(target, "libpiper_phonemize.a"), "arm64")
}

// Upstream simulator archives are universal; Rust wants a thin arm64 archive for aarch64-sim CI.
async function syncSimulatorArm64CargoLibs(piper) {
  const target = await resetCargoLibDir(SHERPA_IOS_SIMULATOR_ARM64_SLICE)
  await thinArchive(sherpaArchiveLib(SHERPA_IOS_SIMULATOR_UPSTREAM_SLICE), join(target, "libsherpa-onnx.a"), "arm64")
  await thinArchive(onnxRuntimeArchiveLib(SHERPA_IOS_SIMULATOR_UPSTREAM_SLICE), join(target, "libonnxruntime.a"), "arm64")
  await thinArchive(join(piper, SHERPA_IOS_SIMULATOR_UPSTREAM_SLICE, "PiperPhonemizeC.framework/PiperPhonemizeC"), join(target, "libpiper_phonemize.a"), "arm64")
}

async function resetCargoLibDir(slice) {
  const target = iosSherpaLibDir(slice)
  await rm(target, {
    recursive: true,
    force: true,
  })
  await mkdir(target, { recursive: true })
  return target
}

// Convert Mach-O universal archives into normal ar archives before Cargo sees them.
async function thinArchive(source, target, arch) {
  await rm(target, { force: true })
  const resolvedSource = await realpath(source)
  if (!await isFatArchive(resolvedSource)) {
    await linkOrCopy(resolvedSource, target)
    return
  }

  try {
    await run("lipo", ["-thin", arch, resolvedSource, "-output", target], { label: "lipo -thin " + arch + " " + resolvedSource })
  } catch (err) {
    throw new Error("Failed to thin iOS universal archive for Rust: " + (err instanceof Error ? err.message : String(err)))
  }
}

function sherpaArchiveLib(slice) {
  return join(SHERPA_IOS_SHERPA_XCFRAMEWORK, slice, "SherpaOnnxC.framework", "SherpaOnnxC")
}

function onnxRuntimeArchiveLib(slice) {
  return join(SHERPA_IOS_ONNXRUNTIME_XCFRAMEWORK, slice, "onnxruntime.framework", "onnxruntime")
}

async function linkOrCopy(source, target) {
  await rm(target, { force: true })
  const resolvedSource = await realpath(source)
  try {
    await link(resolvedSource, target)
  } catch {
    await cp(resolvedSource, target, { force: true })
  }
}

// Fail early if a future sherpa release changes archive shape and would break Rust linking later.
async function verifyCargoLibs(slice) {
  const dir = iosSherpaLibDir(slice)
  for (const lib of SHERPA_IOS_RUST_LIBS) {
    const file = join(dir, lib)
    if (!await isFile(file)) {
      throw new Error("Missing prepared iOS Rust library: " + file)
    }
    if (await isFatArchive(file)) {
      throw new Error("Prepared iOS Rust library is still a universal/fat archive: " + file)
    }
    if (!await isArArchive(file)) {
      throw new Error("Prepared iOS Rust library is not a thin ar archive: " + file)
    }
  }
}

async function isFile(file) {
  try {
    return (await stat(file)).isFile()
  } catch {
    return false
  }
}

// Mach-O universal archives start with CAFEBABE/CAFEBABF, not the `!<arch>` ar magic Rust expects.
async function isFatArchive(file) {
  const bytes = await readMagic(file)
  return bytes.equals(Buffer.from([0xca, 0xfe, 0xba, 0xbe])) || bytes.equals(Buffer.from([0xca, 0xfe, 0xba, 0xbf]))
}

async function isArArchive(file) {
  const bytes = await readFile(file, { encoding: null })
  return bytes.subarray(0, 8).equals(Buffer.from("!<arch>\n"))
}

async function readMagic(file) {
  const bytes = await readFile(file, { encoding: null })
  return bytes.subarray(0, 4)
}
