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
  PIPER_SOURCE_REVISION,
  SHERPA_REQUIRED_LIBS,
} from "../constants.js"
import {
  ANDROID_NDK_VERSION,
  ANDROID_ORT_AAR,
  SHERPA_ANDROID_ABIS,
  SHERPA_ANDROID_RUNTIME_ROOT,
  SHERPA_ANDROID_SOURCE_REVISION,
  SHERPA_ANDROID_STAGED_LIBS,
  SHERPA_DEFAULT_ANDROID_ABI,
  SHERPA_GENERATED_ANDROID_JNI_LIBS,
} from "./constants.js"
import { buildAndroidSourceLibs } from "./source.js"

// Explicit release inputs are flat; developer/emulator inputs are isolated by ABI.
export function androidSherpaLibDir(abi = SHERPA_DEFAULT_ANDROID_ABI) {
  if (process.env.SHERPA_ONNX_LIB_DIR) {
    return resolve(process.env.SHERPA_ONNX_LIB_DIR)
  }
  return join(SHERPA_ANDROID_RUNTIME_ROOT, "jniLibs", abi)
}

// An explicit source directory fails closed; ordinary development builds the same pinned recipe on cache miss.
export async function ensureAndroidSherpaLibs({ force = false, abi = SHERPA_DEFAULT_ANDROID_ABI } = {}) {
  if (!process.env.SHERPA_ONNX_LIB_DIR && process.env.SHERPA_REQUIRE_SOURCE === "1") {
    throw new Error("SHERPA_REQUIRE_SOURCE is set; supply prepared source libraries via SHERPA_ONNX_LIB_DIR")
  }
  const directory = androidSherpaLibDir(abi)
  if (!process.env.SHERPA_ONNX_LIB_DIR) {
    try {
      if (force) {
        throw new Error("Explicit rebuild")
      }
      await validateSource(directory, abi)
    } catch {
      await buildAndroidSourceLibs({
        abi,
        destination: directory,
      })
    }
  }
  await validateSource(directory, abi)
  await syncGeneratedJniLibs(abi)
}

// Check both source identity and output bytes before either Cargo or Gradle consumes them.
async function validateSource(directory, abi) {
  if (!await hasRequiredLibs(directory)) {
    throw new Error("SHERPA_ONNX_LIB_DIR is missing required source libraries; rebuild; refusing a prebuilt fallback")
  }
  const receipt = JSON.parse(await readFile(join(directory, "source-receipt.json"), "utf8"))
  assert.equal(receipt.tools?.ndk, ANDROID_NDK_VERSION, "Rebuild the Android source libraries with NDK " + ANDROID_NDK_VERSION)
  assert.equal(receipt.tools?.api, 26)
  assert.equal(receipt.tools?.abi, abi)
  assert.equal(receipt.sources?.sherpa, SHERPA_ANDROID_SOURCE_REVISION, "Rebuild with the pinned Sherpa revision")
  assert.equal(receipt.sources?.piper, PIPER_SOURCE_REVISION, "Rebuild with the pinned Piper revision")
  assert.deepEqual(receipt.sources?.ortAar, ANDROID_ORT_AAR, "Rebuild with the pinned ONNX Runtime AAR")
  for (const lib of [...SHERPA_REQUIRED_LIBS, "libpiper_phonemize_core.so"]) {
    const bytes = await readFile(join(directory, lib))
    assert(bytes.length > 0, "Empty source library: " + lib)
    assert.equal(createHash("sha256").update(bytes).digest("hex"), receipt.artifacts?.[lib]?.sha256, "Android source library differs from its receipt: " + lib)
  }
}

export async function clearAndroidSherpaLibs() {
  await syncGeneratedJniLibs()
}

// Restored caches can retain empty files/directories; existence alone is not a usable native link input.
async function hasRequiredLibs(directory) {
  for (const lib of [...SHERPA_REQUIRED_LIBS, "libpiper_phonemize_core.so"]) {
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
