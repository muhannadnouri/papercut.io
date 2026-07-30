import { existsSync, mkdirSync } from "node:fs"
import { NODE_VERSION } from "../constants.js"
import {
  SHERPA_LINUX_CUDA_LIB_DIR,
  SHERPA_LINUX_SHARED_OUT_DIR,
  SHERPA_LINUX_SHARED_SOURCE_DIR,
} from "../linux/constants.js"
import { ensureLinuxCudaSherpaLibs } from "../linux/sherpa.js"
import { runSync, shQuote, exitFromResult } from "../process.js"

export async function prepareLinuxDesktopBuild({ isStatic, isCuda = false, bundles = "" }) {
  if (isFlatpak() && !process.env.PAPERCUT_HOST_BUILD) {
    delegateToHost({ isStatic, isCuda, bundles })
  }

  ensurePatchelf()
  ensureLinuxSharedResourceDir()
  if (isCuda) await ensureLinuxCudaSherpaLibs()
}

// Linux AppImage packaging needs NO_STRIP for hosts with newer ELF formats.
export function linuxDesktopEnv(baseEnv) {
  const sherpaLibDir = baseEnv.PAPERCUT_SHERPA_VARIANT === "cuda"
    ? SHERPA_LINUX_CUDA_LIB_DIR
    : SHERPA_LINUX_SHARED_SOURCE_DIR
  return {
    ...baseEnv,
    NO_STRIP: baseEnv.NO_STRIP ?? "1",
    ORT_LIB_LOCATION: baseEnv.ORT_LIB_LOCATION ?? sherpaLibDir,
    SHERPA_ONNX_LIB_DIR: baseEnv.SHERPA_ONNX_LIB_DIR ?? sherpaLibDir,
  }
}

// Detect sandboxed editor terminals that cannot see host WebKitGTK/GTK.
function isFlatpak() {
  return existsSync("/.flatpak-info") || Boolean(process.env.FLATPAK_ID)
}

// Re-run desktop packaging on the host so linuxdeploy can resolve host libs.
function delegateToHost({ isStatic, isCuda, bundles }) {
  const npmScript = isCuda ? "desktop:cuda" : isStatic ? "desktop:static" : "desktop"
  const bundleEnv = bundles ? `export PAPERCUT_DESKTOP_BUNDLES=${shQuote(bundles)}` : ""
  const command = `
set -eu
cd ${shQuote(process.cwd())}
# tauri-env.sh sets Flatpak pkg-config paths for direct in-sandbox Tauri
# commands. The delegated desktop build runs on the host, so remove those
# paths before compiling and before linuxdeploy probes host WebKitGTK/GTK.
unset PKG_CONFIG_PATH
unset PKG_CONFIG_SYSROOT_DIR
unset PKG_CONFIG_LIBDIR
export PAPERCUT_HOST_BUILD=1
export NO_STRIP=1
${bundleEnv}
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && . "$NVM_DIR/nvm.sh"
if command -v nvm >/dev/null 2>&1; then
  nvm use ${NODE_VERSION} >/dev/null
fi
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
npm run ${npmScript}
`.trim()

  console.log("[desktop-build] Flatpak environment detected; running the desktop build on the host OS so linuxdeploy can resolve WebKitGTK.")
  const result = runSync("flatpak-spawn", ["--host", "bash", "-lc", command])
  exitFromResult(result, "[desktop-build] Failed to start host build: ")
}

// Tauri's GStreamer AppImage plugin patches each bundled plugin's runtime path.
function ensurePatchelf() {
  const result = runSync("patchelf", ["--version"], { stdio: "ignore" })
  if (!result.error && result.status === 0) return

  console.error("[desktop-build] patchelf is required to bundle AppImage audio. Install the documented Linux build dependencies and retry.")
  process.exit(1)
}

// Linux config always declares the shared-lib resource dir, even for static builds.
function ensureLinuxSharedResourceDir() {
  // Tauri validates configured resource paths before bundle hooks run, so the
  // gitignored directory must exist even when the selected link mode is static.
  mkdirSync(SHERPA_LINUX_SHARED_OUT_DIR, { recursive: true })
}
