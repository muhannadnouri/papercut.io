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

// Wrapper flags stop at '--'; SDK runner arguments must remain untouched.
const rawArgs = process.argv.slice(2)
const runnerIndex = rawArgs.indexOf("--")
const buildArgs = runnerIndex < 0 ? rawArgs : rawArgs.slice(0, runnerIndex)
const runnerArgs = runnerIndex < 0 ? [] : rawArgs.slice(runnerIndex)
const nativeTts = buildArgs.includes("--native-tts")
const translationWorker = buildArgs.includes("--translation-worker")
const extraArgs = [...buildArgs.filter((arg) => arg !== "--native-tts" && arg !== "--translation-worker"), ...runnerArgs]
const args = extraArgs.length > 0
  ? extraArgs
  : nativeTts
    ? ["android", "build", "--apk", "--debug", "--target", SHERPA_DEFAULT_ANDROID_TAURI_TARGET, "--features", "native-tts-shared,experimental-search-embeddings"]
    : ["android", "build", "--apk", "--debug"]

// Tauri accepts repeated feature options; insert before any runner '--' without replacing custom features.
if (translationWorker) {
  if (args[0] !== "android" || args[1] !== "build") {
    throw new Error("--translation-worker requires an Android build command")
  }
  args.splice(2, 0, "--features", "native-translation")
}

if (nativeTts) {
  await runNativeTtsAndroidBuild(args)
} else {
  await runTauriAndroidBuild(args)
}

// Shipping TTS/search APKs reuse the same staged ORT; custom arguments retain explicit feature selection.
async function runNativeTtsAndroidBuild(args) {
  const abi = SHERPA_DEFAULT_ANDROID_ABI
  const rustTarget = SHERPA_DEFAULT_ANDROID_RUST_TARGET
  const libDir = androidSherpaLibDir(abi)

  await ensureAndroidSherpaLibs()

  if (!existsSync(join(libDir, "libsherpa-onnx-c-api.so"))) {
    throw new Error("Missing Android sherpa native libraries at " + libDir)
  }

  console.log("[sherpa-android-build] SHERPA_ONNX_LIB_DIR=" + libDir)
  console.log("[sherpa-android-build] runtime model download is enabled; model assets are not packaged into the APK")
  console.log("[sherpa-android-build] target=" + rustTarget)
  await runTauriAndroidBuild(args, {
    SHERPA_ONNX_LIB_DIR: libDir,
    ORT_LIB_LOCATION: libDir,
  })
}
