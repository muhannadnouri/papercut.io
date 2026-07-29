import { mkdirSync } from "node:fs"
import { copyMacosSherpaLibs } from "../macos/sherpa.js"
import { runSync, exitFromResult } from "../process.js"
import { SHERPA_MACOS_SHARED_OUT_DIR, SHERPA_MACOS_SHARED_SOURCE_DIR } from "../macos/constants.js"

export function prepareMacosDesktopBuild() {
  // Tauri validates configured resource paths before bundle hooks run, so the
  // gitignored staging directory must exist even before the beforeBundleCommand copies dylibs.
  mkdirSync(SHERPA_MACOS_SHARED_OUT_DIR, { recursive: true })
}

export async function prepareMacosBundleResources({ linkMode, feature, env }) {
  if (linkMode !== "shared") return

  // Tauri resolves bundle.resources before beforeBundleCommand has reliably
  // populated gitignored staging dirs. Run a fast Cargo check first so
  // sherpa-onnx-sys downloads the macOS dylibs, then stage them before the
  // Tauri build process starts scanning resources.
  const result = runSync("cargo", ["check", "--manifest-path", "src-tauri/Cargo.toml", "--features", feature], { env })
  if (result.error || result.status !== 0) {
    exitFromResult(result, "[desktop-build] Failed to prepare macOS shared dylibs: ")
  }

  await copyMacosSherpaLibs({ platform: "macos", linkMode })
}

// ORT_LIB_LOCATION points ort-sys at the sherpa-onnx packaged ONNX Runtime so
// desktop shares one libonnxruntime across sherpa and Libtashkeel, matching Linux.
export function macosDesktopEnv(baseEnv) {
  return {
    ...baseEnv,
    ORT_LIB_LOCATION: baseEnv.ORT_LIB_LOCATION ?? SHERPA_MACOS_SHARED_SOURCE_DIR,
  }
}
