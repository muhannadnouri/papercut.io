import { existsSync } from "node:fs"
import { join } from "node:path"
import { ROOT } from "../lib/paths.js"
import {
  androidProductionEnv,
  runTauriAndroidBuild,
} from "../lib/android/build-env.js"
import {
  SHERPA_DEFAULT_ANDROID_ABI,
  SHERPA_DEFAULT_ANDROID_RUST_TARGET,
  SHERPA_DEFAULT_ANDROID_TAURI_TARGET,
} from "../lib/android/constants.js"
import {
  androidSherpaLibDir,
  clearAndroidSherpaLibs,
  ensureAndroidSherpaLibs,
} from "../lib/android/sherpa.js"

const nativeTts = process.argv.includes("--native-tts")
const production = process.argv.includes("--production")
const extraArgs = process.argv.slice(2).filter((arg) => !["--native-tts", "--production"].includes(arg))

if (production && (!nativeTts || extraArgs.length > 0)) {
  throw new Error("--production requires --native-tts and uses the fixed arm64 shipping features; use custom arguments without --production")
}

if (nativeTts) {
  await runNativeTtsAndroidBuild(extraArgs)
} else {
  await clearAndroidSherpaLibs()
  const args = extraArgs.length > 0
    ? extraArgs
    : ["android", "build", "--apk", "--debug"]
  await runTauriAndroidBuild(args)
}

// Production uses fixed arm64/TTS/search arguments; custom Tauri args replace defaults and can otherwise drop features.
async function runNativeTtsAndroidBuild(extraArgs) {
  process.env.SHERPA_ONNX_LIB_DIR ||= join(ROOT, ".cache/android-production/source-arm64")
  process.env.SHERPA_REQUIRE_SOURCE = "1"
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
    : ["android", "build", "--apk", ...(production ? [] : ["--debug"]), "--target", tauriTarget, "--features", "native-tts-shared,experimental-search-embeddings"]

  console.log("[sherpa-android-build] SHERPA_ONNX_LIB_DIR=" + libDir)
  console.log("[sherpa-android-build] voice and embedding models are downloaded on demand; Arabic preprocessing is embedded")
  console.log("[sherpa-android-build] target=" + rustTarget)
  await runTauriAndroidBuild(args, {
    ...(production ? {
      ...androidProductionEnv(),
      PAPERCUT_PDF_OCR_FROM_SOURCE: "1",
    } : {}),
    SHERPA_ONNX_LIB_DIR: libDir,
    ORT_LIB_LOCATION: libDir,
  })
}
