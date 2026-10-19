import { existsSync } from "node:fs"
import { join } from "node:path"
import { runTauriAndroidBuild } from "./lib/android/build-env.js"
import {
  SHERPA_DEFAULT_ANDROID_ABI,
  SHERPA_DEFAULT_ANDROID_RUST_TARGET,
  SHERPA_DEFAULT_ANDROID_TAURI_TARGET
} from "./lib/android/constants.js"
import {
  androidSherpaLibDir,
  ensureAndroidSherpaLibs
} from "./lib/android/sherpa.js"

const nativeTts = process.argv.includes("--native-tts")
const extraArgs = process.argv.slice(2).filter((arg) => arg !== "--native-tts")

if (nativeTts) {
  await runNativeTtsAndroidBuild(extraArgs)
} else {
  const args = extraArgs.length > 0
    ? extraArgs
    : ["android", "build", "--apk", "--debug"]
  await runTauriAndroidBuild(args)
}

// Shipping TTS/search APKs reuse the same staged ORT; custom arguments retain explicit feature selection.
async function runNativeTtsAndroidBuild(extraArgs) {
  const abi = SHERPA_DEFAULT_ANDROID_ABI
  const rustTarget = SHERPA_DEFAULT_ANDROID_RUST_TARGET
  const tauriTarget = SHERPA_DEFAULT_ANDROID_TAURI_TARGET
  const libDir = androidSherpaLibDir(abi)

  await ensureAndroidSherpaLibs()

  if (!existsSync(join(libDir, "libsherpa-onnx-c-api.so"))) {
    throw new Error("Missing Android sherpa native libraries at " + libDir)
  }

  const args = extraArgs.length > 0
    ? extraArgs
    : ["android", "build", "--apk", "--debug", "--target", tauriTarget, "--features", "native-tts-shared,experimental-search-embeddings"]

  console.log("[sherpa-android-build] SHERPA_ONNX_LIB_DIR=" + libDir)
  console.log("[sherpa-android-build] runtime model download is enabled; model assets are not packaged into the APK")
  console.log("[sherpa-android-build] target=" + rustTarget)
  await runTauriAndroidBuild(args, {
    SHERPA_ONNX_LIB_DIR: libDir,
    ORT_LIB_LOCATION: libDir,
  })
}
