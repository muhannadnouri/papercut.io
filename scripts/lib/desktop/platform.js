import { resolve } from "node:path"

import { ROOT } from "../paths.js"
import {
  prepareLinuxDesktopBuild,
  linuxDesktopEnv,
} from "./linux.js"
import {
  prepareMacosDesktopBuild,
  prepareMacosBundleResources,
  macosDesktopEnv,
} from "./macos.js"
import {
  prepareWindowsDesktopBuild,
  windowsDesktopEnv,
} from "./windows.js"

export function currentDesktopPlatform() {
  if (process.platform === "linux") {
    return "linux"
  }
  if (process.platform === "win32") {
    return "windows"
  }
  if (process.platform === "darwin") {
    return "macos"
  }
  return "unknown"
}

// Keep OS-specific build quirks in one dispatch point.
export function prepareDesktopBuild(platform, options) {
  if (platform === "linux") {
    return prepareLinuxDesktopBuild(options)
  }
  if (platform === "windows") {
    return prepareWindowsDesktopBuild(options)
  }
  if (platform === "macos") {
    return prepareMacosDesktopBuild(options)
  }
}

export async function prepareDesktopBundleResources(platform, options) {
  if (platform === "macos") {
    return prepareMacosBundleResources(options)
  }
}

// Cargo checks run at ROOT while Tauri changes cwd; both must inherit the same absolute target.
export function desktopBuildEnv(platform, baseEnv) {
  const env = { ...baseEnv }
  if (env.CARGO_TARGET_DIR) {
    env.CARGO_TARGET_DIR = resolve(ROOT, env.CARGO_TARGET_DIR)
  }
  if (platform === "linux") {
    return linuxDesktopEnv(env)
  }
  if (platform === "windows") {
    return windowsDesktopEnv(env)
  }
  if (platform === "macos") {
    return macosDesktopEnv(env)
  }
  return env
}
