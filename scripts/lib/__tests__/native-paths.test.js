import {
  describe,
  expect,
  it,
} from "vitest"
import { resolve } from "node:path"

import { nativeCacheDir } from "../native-paths.js"
import {
  ROOT,
  SRC_TAURI_DIR,
} from "../paths.js"
import { windowsDesktopEnv } from "../desktop/windows.js"
import { desktopBuildEnv } from "../desktop/platform.js"

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

  it.each(["linux", "macos", "windows"])("normalizes %s child targets before Cargo or Tauri changes cwd", platform => {
    const baseEnv = {
      CARGO_TARGET_DIR: "target with spaces",
      ORT_LIB_LOCATION: "/explicit",
      SHERPA_ONNX_LIB_DIR: "/prepared-sherpa",
      PIPER_PHONEMIZE_LIB_DIR: "/prepared-piper",
    }
    const env = desktopBuildEnv(platform, baseEnv)
    const target = resolve(ROOT, "target with spaces")
    expect(env.CARGO_TARGET_DIR).toBe(target)
    expect(resolve(SRC_TAURI_DIR, env.CARGO_TARGET_DIR)).toBe(target)
    expect(nativeCacheDir("sherpa", "archive", env)).toBe(resolve(target, "sherpa/archive/lib"))
    expect(env.ORT_LIB_LOCATION).toBe("/explicit")
    expect(env.SHERPA_ONNX_LIB_DIR).toBe(baseEnv.SHERPA_ONNX_LIB_DIR)
    expect(env.PIPER_PHONEMIZE_LIB_DIR).toBe(baseEnv.PIPER_PHONEMIZE_LIB_DIR)
    expect(baseEnv.CARGO_TARGET_DIR).toBe("target with spaces")
    expect(desktopBuildEnv(platform, env).CARGO_TARGET_DIR).toBe(target)
    expect(desktopBuildEnv(platform, {}).CARGO_TARGET_DIR).toBeUndefined()
  })
})
