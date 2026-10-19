import { join } from "node:path"
import { SHERPA_VERSION } from "../constants.js"
import { SRC_TAURI_DIR } from "../paths.js"

export function prepareWindowsDesktopBuild() {
  // Windows currently uses Tauri's standard MSI/NSIS packaging path.
}

export function windowsDesktopEnv(baseEnv) {
  // Reuse sherpa's runtime instead of ort-sys downloading a second ONNX Runtime.
  return {
    ...baseEnv,
    ORT_LIB_LOCATION: baseEnv.ORT_LIB_LOCATION ?? join(
      SRC_TAURI_DIR, "target", "sherpa-onnx-prebuilt",
      `sherpa-onnx-v${SHERPA_VERSION}-win-x64-shared-MT-Release-lib`, "lib",
    ),
  }
}
