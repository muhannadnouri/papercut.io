import { SHERPA_VERSION } from "../constants.js"
import { nativeCacheDir } from "../native-paths.js"

export function prepareWindowsDesktopBuild() {
  // Windows currently uses Tauri's standard MSI/NSIS packaging path.
}

export function windowsDesktopEnv(baseEnv) {
  // Reuse sherpa's runtime instead of ort-sys downloading a second ONNX Runtime.
  return {
    ...baseEnv,
    ORT_LIB_LOCATION: baseEnv.ORT_LIB_LOCATION ?? baseEnv.SHERPA_ONNX_LIB_DIR ?? nativeCacheDir(
      "sherpa-onnx-prebuilt",
      `sherpa-onnx-v${SHERPA_VERSION}-win-x64-shared-MT-Release-lib`, baseEnv,
    ),
  }
}
