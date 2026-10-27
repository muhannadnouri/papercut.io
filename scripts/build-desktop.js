import {
  currentDesktopPlatform,
  desktopBuildEnv,
  prepareDesktopBuild,
  prepareDesktopBundleResources,
} from "./lib/desktop/platform.js"
import {
  runSync,
  exitFromResult,
} from "./lib/process.js"
import { tauriCommand } from "./lib/tauri.js"

const isStatic = process.argv.includes("--static")
const bundles = desktopBundles()
const linkMode = isStatic ? "static" : "shared"
// Shipping builds include capability; user consent still controls model loading.
const feature = isStatic ? "native-tts-static" : "native-tts-shared,experimental-search-embeddings"
const platform = currentDesktopPlatform()

prepareDesktopBuild(platform, { isStatic })
const env = desktopBuildEnv(platform, {
  ...process.env,
  PAPERCUT_NATIVE_TTS_LINK: linkMode,
})

await prepareDesktopBundleResources(platform, {
  linkMode,
  feature,
  env,
})
runTauriBuild(env, bundles)

// Shared shipping packages reuse TTS's ORT runtime; the explicit static alternative stays lexical-only.
function runTauriBuild(env, bundles) {
  const tauriArgs = ["build", "--features", feature]
  if (bundles) {
    tauriArgs.push("--bundles", bundles)
  }
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
