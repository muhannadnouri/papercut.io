import assert from "node:assert/strict"
import { createHash } from "node:crypto"
import {
  cp,
  mkdir,
  readFile,
  rm,
  stat,
} from "node:fs/promises"
import {
  join,
  resolve,
} from "node:path"
import {
  ANDROID_NDK_VERSION,
  ANDROID_ORT_AAR,
  SHERPA_ANDROID_ABIS,
  SHERPA_ANDROID_ARCHIVE,
  SHERPA_ANDROID_RUNTIME_ROOT,
  SHERPA_ANDROID_SHA256,
  SHERPA_ANDROID_SOURCE_REVISION,
  SHERPA_ANDROID_URL,
  SHERPA_ANDROID_STAGED_LIBS,
  SHERPA_DEFAULT_ANDROID_ABI,
  SHERPA_GENERATED_ANDROID_JNI_LIBS,
} from "./constants.js"
import { SHERPA_REQUIRED_LIBS } from "../constants.js"
import { downloadFile } from "../download.js"
import { extractTar } from "../archive.js"

// App builds supply a flat arm64 directory; the archive cache serves the optional emulator fixture.
// Only Sherpa's own library is staged: libonnxruntime.so comes from the Gradle Maven dependency instead.
export function androidSherpaLibDir(abi = SHERPA_DEFAULT_ANDROID_ABI) {
  if (process.env.SHERPA_ONNX_LIB_DIR) {
    return resolve(process.env.SHERPA_ONNX_LIB_DIR)
  }
  return join(SHERPA_ANDROID_RUNTIME_ROOT, "jniLibs", abi)
}

// An explicit source directory must fail closed: falling back would silently put prebuilt binaries in an F-Droid build.
export async function ensureAndroidSherpaLibs({ force = false } = {}) {
  if (!process.env.SHERPA_ONNX_LIB_DIR && process.env.SHERPA_REQUIRE_SOURCE === "1") {
    throw new Error("SHERPA_REQUIRE_SOURCE is set; stage the source-built Sherpa library via SHERPA_ONNX_LIB_DIR instead of downloading the prebuilt archive")
  }
  if (process.env.SHERPA_ONNX_LIB_DIR) {
    const directory = androidSherpaLibDir()
    if (!await hasRequiredLibs(directory)) {
      throw new Error("SHERPA_ONNX_LIB_DIR is missing required Android libraries; run scripts/fdroid/build-android-source-libs.js on Linux; refusing a prebuilt fallback")
    }
    const receipt = JSON.parse(await readFile(join(directory, "source-receipt.json"), "utf8"))
    assert.equal(receipt.tools?.ndk, ANDROID_NDK_VERSION,
      "Rebuild the Android source libraries with NDK " + ANDROID_NDK_VERSION)
    assert.equal(receipt.tools?.api, 26, "Android source libraries require API 26")
    assert.equal(receipt.sources?.sherpa, SHERPA_ANDROID_SOURCE_REVISION, "Rebuild the Android source libraries with the pinned Sherpa revision")
    assert.deepEqual(receipt.sources?.ortAar, ANDROID_ORT_AAR, "Rebuild the Android source libraries with the pinned ONNX Runtime AAR")
    for (const lib of SHERPA_REQUIRED_LIBS) {
      const bytes = await readFile(join(directory, lib))
      assert(bytes.length > 0, "Empty Android source library: " + lib)
      assert.equal(createHash("sha256").update(bytes).digest("hex"), receipt.artifacts?.[lib]?.sha256,
        "Android source library differs from its receipt: " + lib)
    }
    await syncGeneratedJniLibs(SHERPA_DEFAULT_ANDROID_ABI)
    return
  }
  await mkdir(SHERPA_ANDROID_RUNTIME_ROOT, { recursive: true })
  if (force) {
    await rm(join(SHERPA_ANDROID_RUNTIME_ROOT, "jniLibs"), {
      recursive: true,
      force: true,
    })
  }

  if (!await hasAllAndroidRuntimeLibs()) {
    await downloadFile({
      url: SHERPA_ANDROID_URL,
      dest: SHERPA_ANDROID_ARCHIVE,
      force,
      sha256: SHERPA_ANDROID_SHA256,
      label: "sherpa-android-libs",
    })
    await extractAndroidArchive()
    await rm(SHERPA_ANDROID_ARCHIVE, { force: true })
  }

  if (!await hasAllAndroidRuntimeLibs()) {
    throw new Error("Downloaded sherpa-onnx Android archive is missing required shared libraries")
  }

  console.log("[sherpa-android-libs] ready " + join(SHERPA_ANDROID_RUNTIME_ROOT, "jniLibs"))
}

export async function clearAndroidSherpaLibs() {
  await syncGeneratedJniLibs()
}

// Keep emulator ABIs cached even though production APKs are filtered to arm64.
async function hasAllAndroidRuntimeLibs() {
  for (const abi of SHERPA_ANDROID_ABIS) {
    if (!await hasRequiredLibs(androidSherpaLibDir(abi))) {
      return false
    }
  }
  return true
}

// Restored caches can retain empty files/directories; existence alone is not a usable native link input.
async function hasRequiredLibs(directory) {
  for (const lib of SHERPA_ANDROID_STAGED_LIBS) {
    try {
      const info = await stat(join(directory, lib))
      if (!info.isFile() || info.size === 0) {
        return false
      }
    } catch {
      return false
    }
  }
  return true
}

// The upstream archive already starts at jniLibs/; stripping its first component breaks Gradle's ABI layout.
async function extractAndroidArchive() {
  console.log("[sherpa-android-libs] extracting " + SHERPA_ANDROID_ARCHIVE)
  await extractTar({
    archive: SHERPA_ANDROID_ARCHIVE,
    destination: SHERPA_ANDROID_RUNTIME_ROOT,
    compression: "bzip2",
  })
}

// Gradle packages these files separately from Rust linking, so both paths must receive the same runtime bytes.
async function syncGeneratedJniLibs(stagedAbi) {
  for (const abi of SHERPA_ANDROID_ABIS) {
    const source = androidSherpaLibDir(abi)
    const target = join(SHERPA_GENERATED_ANDROID_JNI_LIBS, abi)
    // Papercut calls the C API; neither it nor ORT links Sherpa's C++/JNI wrappers.
    // A stale archive-era libonnxruntime.so would collide with the Gradle AAR copy at packaging time.
    for (const unused of ["libsherpa-onnx-cxx-api.so", "libsherpa-onnx-jni.so", "libonnxruntime.so"]) {
      await rm(join(target, unused), { force: true })
    }
    for (const lib of SHERPA_ANDROID_STAGED_LIBS) {
      if (abi === stagedAbi) {
        await mkdir(target, { recursive: true })
        await cp(join(source, lib), join(target, lib), { force: true })
      } else {
        await rm(join(target, lib), { force: true })
      }
    }
  }
}
