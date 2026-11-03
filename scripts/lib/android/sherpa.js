import {
  cp,
  mkdir,
  rm,
  stat,
} from "node:fs/promises"
import {
  join,
  resolve,
} from "node:path"
import {
  SHERPA_ANDROID_ABIS,
  SHERPA_ANDROID_ARCHIVE,
  SHERPA_ANDROID_RUNTIME_ROOT,
  SHERPA_ANDROID_SHA256,
  SHERPA_ANDROID_URL,
  SHERPA_DEFAULT_ANDROID_ABI,
  SHERPA_GENERATED_ANDROID_JNI_LIBS,
  SHERPA_REQUIRED_LIBS,
} from "./constants.js"
import { downloadFile } from "../download.js"
import { extractTar } from "../archive.js"

// Source builds supply one flat arm64 directory; archive ABIs also support the separate emulator inference checks.
export function androidSherpaLibDir(abi = SHERPA_DEFAULT_ANDROID_ABI) {
  if (process.env.SHERPA_ONNX_LIB_DIR) {
    return resolve(process.env.SHERPA_ONNX_LIB_DIR)
  }
  return join(SHERPA_ANDROID_RUNTIME_ROOT, "jniLibs", abi)
}

// An explicit source directory must fail closed: falling back would silently put prebuilt binaries in an F-Droid build.
export async function ensureAndroidSherpaLibs({ force = false } = {}) {
  if (process.env.SHERPA_ONNX_LIB_DIR) {
    if (!await hasRequiredLibs(androidSherpaLibDir())) {
      throw new Error("SHERPA_ONNX_LIB_DIR is missing required Android libraries; refusing a prebuilt fallback")
    }
    await syncGeneratedJniLibs([SHERPA_DEFAULT_ANDROID_ABI])
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

  await syncGeneratedJniLibs()
  console.log("[sherpa-android-libs] ready " + join(SHERPA_ANDROID_RUNTIME_ROOT, "jniLibs"))
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
  for (const lib of SHERPA_REQUIRED_LIBS) {
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
async function syncGeneratedJniLibs(abis = SHERPA_ANDROID_ABIS) {
  for (const abi of abis) {
    const source = androidSherpaLibDir(abi)
    const target = join(SHERPA_GENERATED_ANDROID_JNI_LIBS, abi)
    await mkdir(target, { recursive: true })
    // Papercut calls the C API; neither it nor ORT links Sherpa's C++/JNI wrappers.
    for (const unused of ["libsherpa-onnx-cxx-api.so", "libsherpa-onnx-jni.so"]) {
      await rm(join(target, unused), { force: true })
    }
    for (const lib of SHERPA_REQUIRED_LIBS) {
      await cp(join(source, lib), join(target, lib), { force: true })
    }
  }
}
