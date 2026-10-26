import { existsSync } from "node:fs"
import {
  cp,
  mkdir,
  rm,
  stat,
} from "node:fs/promises"
import { join } from "node:path"
import {
  SHERPA_ANDROID_ABIS,
  SHERPA_ANDROID_ARCHIVE,
  SHERPA_ANDROID_COPY_LIBS,
  SHERPA_ANDROID_RUNTIME_ROOT,
  SHERPA_ANDROID_SHA256,
  SHERPA_ANDROID_URL,
  SHERPA_DEFAULT_ANDROID_ABI,
  SHERPA_GENERATED_ANDROID_JNI_LIBS,
  SHERPA_REQUIRED_LIBS,
} from "./constants.js"
import { downloadFile } from "../download.js"
import { extractTar } from "../archive.js"

// Rust build needs ABI-specific native library directory via SHERPA_ONNX_LIB_DIR.
export function androidSherpaLibDir(abi = SHERPA_DEFAULT_ANDROID_ABI) {
  return join(SHERPA_ANDROID_RUNTIME_ROOT, "jniLibs", abi)
}

// Prepare official Android shared libs once, then sync them into Tauri's Gradle tree.
export async function ensureAndroidSherpaLibs({ force = false } = {}) {
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

// Verify every supported ABI has libs needed by native TTS before building.
async function hasAllAndroidRuntimeLibs() {
  for (const abi of SHERPA_ANDROID_ABIS) {
    for (const lib of SHERPA_REQUIRED_LIBS) {
      try {
        const info = await stat(join(SHERPA_ANDROID_RUNTIME_ROOT, "jniLibs", abi, lib))
        if (!info.isFile()) {
          return false
        }
      } catch {
        return false
      }
    }
  }
  return true
}

async function extractAndroidArchive() {
  console.log("[sherpa-android-libs] extracting " + SHERPA_ANDROID_ARCHIVE)
  await extractTar({
    archive: SHERPA_ANDROID_ARCHIVE,
    destination: SHERPA_ANDROID_RUNTIME_ROOT,
    compression: "bzip2",
  })
}

// Tauri's generated Android project is what Gradle packages into the APK.
async function syncGeneratedJniLibs() {
  for (const abi of SHERPA_ANDROID_ABIS) {
    const source = join(SHERPA_ANDROID_RUNTIME_ROOT, "jniLibs", abi)
    const target = join(SHERPA_GENERATED_ANDROID_JNI_LIBS, abi)
    await mkdir(target, { recursive: true })
    for (const lib of SHERPA_ANDROID_COPY_LIBS) {
      const sourceFile = join(source, lib)
      if (existsSync(sourceFile)) {
        await cp(sourceFile, join(target, lib), { force: true })
      }
    }
  }
}
