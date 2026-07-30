import { currentDesktopPlatform, desktopBuildEnv, prepareDesktopBuild, prepareDesktopBundleResources } from "./lib/desktop/platform.js"
import { runSync, exitFromResult } from "./lib/process.js"
import { tauriCommand } from "./lib/tauri.js"

const isStatic = process.argv.includes("--static")
const isCuda = process.argv.includes("--cuda")
const bundles = desktopBundles()
const linkMode = isStatic ? "static" : "shared"
const feature = isStatic ? "native-tts-static" : "native-tts-shared"
const platform = currentDesktopPlatform()

validateVariant()
await prepareDesktopBuild(platform, { isStatic, isCuda, bundles })
const env = desktopBuildEnv(platform, {
  ...process.env,
  PAPERCUT_NATIVE_TTS_LINK: linkMode,
  PAPERCUT_SHERPA_VARIANT: isCuda ? "cuda" : "cpu",
  PAPERCUT_SHERPA_DEFAULT_PROVIDER: isCuda ? "cuda" : "cpu",
})

await prepareDesktopBundleResources(platform, { linkMode, feature, env })
runTauriBuild(env, bundles)

// CUDA is an explicit Linux x64 artifact so normal installers stay CPU-only.
function validateVariant() {
  if (!isCuda) return
  if (isStatic) {
    console.error("[desktop-build] --cuda requires the shared native-TTS build; remove --static.")
    process.exit(1)
  }
  if (platform !== "linux" || process.arch !== "x64") {
    console.error("[desktop-build] --cuda is currently supported only on Linux x64.")
    process.exit(1)
  }
}

// Build with the selected native-TTS link mode using platform-specific env.
function runTauriBuild(env, bundles) {
  const tauriArgs = ["build", "--features", feature]
  if (bundles) tauriArgs.push("--bundles", bundles)
  const { command, args } = tauriCommand(tauriArgs)
  const result = runSync(command, args, { env })
  exitFromResult(result, "[desktop-build] Failed to start Tauri build: ")
}

// Keep release builds unchanged; allow packaging spikes to build one Linux bundle.
function desktopBundles() {
  const index = process.argv.indexOf("--bundles")
  if (index >= 0) {
    const value = process.argv[index + 1]
    if (!value || value.startsWith("--")) {
      console.error("[desktop-build] --bundles requires a Tauri bundle list, for example: --bundles appimage")
      process.exit(1)
    }
    return value
  }
  return process.env.PAPERCUT_DESKTOP_BUNDLES || ""
}
