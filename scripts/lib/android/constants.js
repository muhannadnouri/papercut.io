import { join } from "node:path"
import { SHERPA_REQUIRED_LIBS, SHERPA_VERSION } from "../constants.js"
import { fromRuntime, SRC_TAURI_DIR } from "../paths.js"

export { SHERPA_REQUIRED_LIBS, SHERPA_VERSION }

export const JDK_VERSION = "17"
export const JDK_RELEASE = "17.0.19+10"
export const JDK_ROOT = fromRuntime("jdk")
export const JDK_HOME = join(JDK_ROOT, "temurin-" + JDK_VERSION)
export const JDK_ARCHIVE_NAME = "OpenJDK17U-jdk_x64_linux_hotspot_17.0.19_10.tar.gz"
export const JDK_ARCHIVE = join(JDK_ROOT, JDK_ARCHIVE_NAME)
export const JDK_RELEASE_TAG = "jdk-" + JDK_RELEASE.replace("+", "%2B")
export const JDK_DOWNLOAD_URL = "https://github.com/adoptium/temurin17-binaries/releases/download/" +
  JDK_RELEASE_TAG +
  "/" +
  JDK_ARCHIVE_NAME
export const JDK_SHA256 = "d8afc263758141a66e0e3aafc321e783f7016696f4eaea067d340a269037d331"

export const SHERPA_ANDROID_RUNTIME_ROOT = fromRuntime("sherpa-onnx-android")
export const SHERPA_ANDROID_ARCHIVE = join(
  SHERPA_ANDROID_RUNTIME_ROOT,
  "sherpa-onnx-v" + SHERPA_VERSION + "-android.tar.bz2",
)
export const SHERPA_ANDROID_URL = "https://github.com/k2-fsa/sherpa-onnx/releases/download/v" +
  SHERPA_VERSION +
  "/sherpa-onnx-v" +
  SHERPA_VERSION +
  "-android.tar.bz2"
export const SHERPA_ANDROID_SHA256 = "7983fc3de23f6e64148f2fb05fa94a2efaa8c0516cc1573383dc5c7d4d2a43b0"
export const SHERPA_ANDROID_ABIS = ["arm64-v8a", "armeabi-v7a", "x86", "x86_64"]
export const SHERPA_DEFAULT_ANDROID_ABI = "arm64-v8a"
export const SHERPA_DEFAULT_ANDROID_RUST_TARGET = "aarch64-linux-android"
export const SHERPA_DEFAULT_ANDROID_TAURI_TARGET = "aarch64"
export const SHERPA_ANDROID_COPY_LIBS = [
  "libsherpa-onnx-c-api.so",
  "libsherpa-onnx-cxx-api.so",
  "libonnxruntime.so",
  "libsherpa-onnx-jni.so",
]
export const SHERPA_GENERATED_ANDROID_JNI_LIBS = join(
  SRC_TAURI_DIR,
  "gen",
  "android",
  "app",
  "src",
  "main",
  "jniLibs",
)
