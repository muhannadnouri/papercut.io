import { prepareLinuxDesktopBuild, linuxDesktopEnv } from "./linux.js"
import { prepareMacosDesktopBuild, prepareMacosBundleResources, macosDesktopEnv } from "./macos.js"
import { prepareWindowsDesktopBuild, windowsDesktopEnv } from "./windows.js"

export function currentDesktopPlatform() {
  if (process.platform === "linux") return "linux"
  if (process.platform === "win32") return "windows"
  if (process.platform === "darwin") return "macos"
  return "unknown"
}

// Keep OS-specific build quirks in one dispatch point.
export function prepareDesktopBuild(platform, options) {
  if (platform === "linux") return prepareLinuxDesktopBuild(options)
  if (platform === "windows") return prepareWindowsDesktopBuild(options)
  if (platform === "macos") return prepareMacosDesktopBuild(options)
}

export async function prepareDesktopBundleResources(platform, options) {
  if (platform === "macos") return prepareMacosBundleResources(options)
}

export function desktopBuildEnv(platform, baseEnv) {
  if (platform === "linux") return linuxDesktopEnv(baseEnv)
  if (platform === "windows") return windowsDesktopEnv(baseEnv)
  if (platform === "macos") return macosDesktopEnv(baseEnv)
  return { ...baseEnv }
}
