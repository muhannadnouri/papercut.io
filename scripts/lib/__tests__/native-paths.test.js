import {
  describe,
  expect,
  it,
} from "vitest"
import { resolve } from "node:path"

import { nativeCacheDir } from "../native-paths.js"
import { SRC_TAURI_DIR } from "../paths.js"
import { windowsDesktopEnv } from "../desktop/windows.js"

describe("native archive paths", () => {
  it("keeps default caches and resolves relative custom targets from the wrapper cwd", () => {
    expect(nativeCacheDir("sherpa", "archive", {})).toBe(resolve(SRC_TAURI_DIR, "target/sherpa/archive/lib"))
    expect(nativeCacheDir("piper", "archive", { CARGO_TARGET_DIR: "target with spaces" })).toBe(resolve("target with spaces/piper/archive/lib"))
  })

  it("keeps the Windows ORT loader on the supplied Sherpa runtime", () => {
    expect(windowsDesktopEnv({ SHERPA_ONNX_LIB_DIR: "/prepared" }).ORT_LIB_LOCATION).toBe("/prepared")
    expect(windowsDesktopEnv({ CARGO_TARGET_DIR: "/custom" }).ORT_LIB_LOCATION).toContain(resolve("/custom", "sherpa-onnx-prebuilt"))
    expect(windowsDesktopEnv({
      ORT_LIB_LOCATION: "/explicit",
      SHERPA_ONNX_LIB_DIR: "/prepared",
    }).ORT_LIB_LOCATION).toBe("/explicit")
  })
})
