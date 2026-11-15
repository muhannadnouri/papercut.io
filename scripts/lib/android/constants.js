import { join } from "node:path"
import {
  SHERPA_VERSION,
} from "../constants.js"
import {
  fromRuntime,
  SRC_TAURI_DIR,
} from "../paths.js"

export { SHERPA_VERSION }

export const ANDROID_RUST_TOOLCHAIN = "1.98.1"
export const ANDROID_NDK_VERSION = "30.0.16248370"
export const PAGEFIND_WASM_PATH_PREFIX = "/build/papercut-pagefind"
export const PAGEFIND_SOURCE_REVISION = "8c3de15a6b5ab603985d98062e748463daf42d65"
export const SHERPA_ANDROID_SOURCE_REVISION = "af2d8d81dbfa61c4cea4c568da78f3982edca005"
export const ANDROID_ORT_AAR = {
  version: "1.27.0",
  url: "https://repo1.maven.org/maven2/com/microsoft/onnxruntime/onnxruntime-android/1.27.0/onnxruntime-android-1.27.0.aar",
  sha256: "077dec5e2d821234c7dc0aba584bec8f999854b546c754cab93a90741c56fbeb",
}

// Gradle supplies libonnxruntime.so from the pinned Maven AAR; only the Sherpa and Piper C libraries are staged.
export const SHERPA_ANDROID_STAGED_LIBS = ["libsherpa-onnx-c-api.so", "libpiper_phonemize_core.so"]

export const JDK_VERSION = "17"
export const JDK_RELEASE = "17.0.19+10"
export const JDK_ROOT = fromRuntime("jdk")
export const JDK_HOME = join(JDK_ROOT, "temurin-" + JDK_VERSION)
export const JDK_ARCHIVE_NAME = "OpenJDK17U-jdk_x64_linux_hotspot_17.0.19_10.tar.gz"
export const JDK_ARCHIVE = join(JDK_ROOT, JDK_ARCHIVE_NAME)
export const JDK_RELEASE_TAG = "jdk-" + encodeURIComponent(JDK_RELEASE)
export const JDK_DOWNLOAD_URL = "https://github.com/adoptium/temurin17-binaries/releases/download/" +
  JDK_RELEASE_TAG +
  "/" +
  JDK_ARCHIVE_NAME
export const JDK_SHA256 = "d8afc263758141a66e0e3aafc321e783f7016696f4eaea067d340a269037d331"

export const SHERPA_ANDROID_RUNTIME_ROOT = fromRuntime("sherpa-onnx-android")
export const SHERPA_ANDROID_ABIS = ["arm64-v8a", "armeabi-v7a", "x86", "x86_64"]
export const SHERPA_DEFAULT_ANDROID_ABI = "arm64-v8a"
export const SHERPA_DEFAULT_ANDROID_RUST_TARGET = "aarch64-linux-android"
export const SHERPA_DEFAULT_ANDROID_TAURI_TARGET = "aarch64"
export const SHERPA_GENERATED_ANDROID_JNI_LIBS = join(
  SRC_TAURI_DIR,
  "gen",
  "android",
  "app",
  "src",
  "main",
  "jniLibs",
)
