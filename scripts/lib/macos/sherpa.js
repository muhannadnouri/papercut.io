import { existsSync } from "node:fs"
import { spawnSync } from "node:child_process"
import { cp, mkdir, readdir, stat } from "node:fs/promises"
import { join } from "node:path"
import {
  SHERPA_MACOS_OPTIONAL_LIBS,
  SHERPA_MACOS_REQUIRED_LIBS,
  SHERPA_MACOS_SHARED_OUT_DIR,
  SHERPA_MACOS_SHARED_SOURCE_DIR,
} from "./constants.js"

// Shared-link macOS desktop builds bundle sherpa dylibs as Tauri resources so
// the app's @loader_path/../Resources rpath resolves inside the .app bundle.
export async function copyMacosSherpaLibs({ platform = process.env.TAURI_ENV_PLATFORM, linkMode = process.env.PAPERCUT_NATIVE_TTS_LINK } = {}) {
  if (platform && platform !== "macos") return
  if (linkMode === "static") return

  await assertMacosSharedLibsExist()
  await mkdir(SHERPA_MACOS_SHARED_OUT_DIR, { recursive: true })

  const copied = []
  for (const lib of await macosDylibsToBundle()) {
    const source = join(SHERPA_MACOS_SHARED_SOURCE_DIR, lib)
    const target = join(SHERPA_MACOS_SHARED_OUT_DIR, lib)
    await cp(source, target, { force: true, dereference: true })
    copied.push(target)
  }

  signMacosDylibs(copied)
  console.log("[sherpa-macos-libs] bundled shared libraries from " + SHERPA_MACOS_SHARED_SOURCE_DIR)
}

async function macosDylibsToBundle() {
  const entries = await readdir(SHERPA_MACOS_SHARED_SOURCE_DIR, { withFileTypes: true })
  const dylibs = entries
    .filter((entry) => entry.isFile() || entry.isSymbolicLink())
    .map((entry) => entry.name)
    .filter((name) => name.endsWith(".dylib"))
    .sort()

  return Array.from(new Set([...SHERPA_MACOS_REQUIRED_LIBS, ...SHERPA_MACOS_OPTIONAL_LIBS, ...dylibs]))
    .filter((lib) => existsSync(join(SHERPA_MACOS_SHARED_SOURCE_DIR, lib)))
}

function signMacosDylibs(paths) {
  const identity = process.env.APPLE_SIGNING_IDENTITY
  if (process.platform !== "darwin" || !identity || paths.length === 0) return

  for (const path of paths) {
    const result = spawnSync("codesign", ["--force", "--options", "runtime", "--timestamp", "--sign", identity, path], {
      stdio: "inherit",
    })

    if (result.error) {
      throw new Error("Failed to start codesign for " + path + ": " + result.error.message)
    }
    if (result.status !== 0) {
      throw new Error("codesign failed for " + path + " with exit code " + result.status)
    }
  }
}

// Fail with build guidance instead of producing an installer missing runtime libs.
async function assertMacosSharedLibsExist() {
  for (const lib of SHERPA_MACOS_REQUIRED_LIBS) {
    try {
      const info = await stat(join(SHERPA_MACOS_SHARED_SOURCE_DIR, lib))
      if (!info.isFile()) throw new Error(lib + " is not a file")
    } catch (err) {
      throw new Error(
        "Missing " + lib + " at " + SHERPA_MACOS_SHARED_SOURCE_DIR + ". Build with --features native-tts-shared so sherpa-onnx downloads its macOS shared library archive before bundling. " + err,
      )
    }
  }
}
