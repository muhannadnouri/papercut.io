import { existsSync } from "node:fs"
import { cp, mkdir, rm, stat } from "node:fs/promises"
import { join } from "node:path"
import {
  SHERPA_LINUX_CUDA_ARCHIVE,
  SHERPA_LINUX_CUDA_LIB_DIR,
  SHERPA_LINUX_CUDA_PROVIDER_LIBS,
  SHERPA_LINUX_CUDA_ROOT,
  SHERPA_LINUX_CUDA_SHA256,
  SHERPA_LINUX_CUDA_URL,
  SHERPA_LINUX_PREBUILT_ROOT,
  SHERPA_LINUX_OPTIONAL_LIBS,
  SHERPA_LINUX_SHARED_OUT_DIR,
  SHERPA_LINUX_SHARED_SOURCE_DIR,
  SHERPA_REQUIRED_LIBS,
} from "./constants.js"
import { downloadFile } from "../download.js"
import { extractTar } from "../archive.js"

const CUDA_REQUIRED_LIBS = [...SHERPA_REQUIRED_LIBS, ...SHERPA_LINUX_CUDA_PROVIDER_LIBS]

// Download and verify the official CUDA 12/cuDNN 9 sherpa archive on demand.
export async function ensureLinuxCudaSherpaLibs() {
  if (await hasAllLibs(SHERPA_LINUX_CUDA_LIB_DIR, CUDA_REQUIRED_LIBS)) return

  await mkdir(SHERPA_LINUX_PREBUILT_ROOT, { recursive: true })
  await downloadFile({
    url: SHERPA_LINUX_CUDA_URL,
    dest: SHERPA_LINUX_CUDA_ARCHIVE,
    sha256: SHERPA_LINUX_CUDA_SHA256,
    label: "sherpa-linux-cuda-libs",
  })
  await rm(SHERPA_LINUX_CUDA_ROOT, { recursive: true, force: true })
  await extractTar({
    archive: SHERPA_LINUX_CUDA_ARCHIVE,
    destination: SHERPA_LINUX_PREBUILT_ROOT,
    compression: "bzip2",
  })

  if (!await hasAllLibs(SHERPA_LINUX_CUDA_LIB_DIR, CUDA_REQUIRED_LIBS)) {
    throw new Error("Downloaded sherpa-onnx Linux CUDA archive is missing required shared libraries")
  }
}

// Shared-link desktop builds must bundle sherpa libs next to the installed app.
export async function copyLinuxSherpaLibs({ platform = process.env.TAURI_ENV_PLATFORM, linkMode = process.env.PAPERCUT_NATIVE_TTS_LINK } = {}) {
  if (platform && platform !== "linux") return
  if (linkMode === "static") return

  const isCuda = process.env.PAPERCUT_SHERPA_VARIANT === "cuda"
  const sourceDir = isCuda ? SHERPA_LINUX_CUDA_LIB_DIR : SHERPA_LINUX_SHARED_SOURCE_DIR
  const copyLibs = [
    ...SHERPA_REQUIRED_LIBS,
    ...SHERPA_LINUX_OPTIONAL_LIBS,
    ...(isCuda ? SHERPA_LINUX_CUDA_PROVIDER_LIBS : []),
  ]
  await assertLinuxSharedLibsExist(sourceDir, isCuda ? CUDA_REQUIRED_LIBS : SHERPA_REQUIRED_LIBS)
  await rm(SHERPA_LINUX_SHARED_OUT_DIR, { recursive: true, force: true })
  await mkdir(SHERPA_LINUX_SHARED_OUT_DIR, { recursive: true })

  for (const lib of copyLibs) {
    const source = join(sourceDir, lib)
    if (existsSync(source)) {
      await cp(source, join(SHERPA_LINUX_SHARED_OUT_DIR, lib), { force: true })
    }
  }

  console.log("[sherpa-linux-libs] bundled shared libraries from " + sourceDir)
}

// Fail with build guidance instead of producing an installer missing runtime libs.
async function assertLinuxSharedLibsExist(sourceDir, requiredLibs) {
  if (!await hasAllLibs(sourceDir, requiredLibs)) {
    throw new Error(
      "Missing required sherpa-onnx libraries at " + sourceDir +
      ". Build with the matching desktop command so its native archive is prepared before bundling.",
    )
  }
}

async function hasAllLibs(sourceDir, requiredLibs) {
  for (const lib of requiredLibs) {
    try {
      const info = await stat(join(sourceDir, lib))
      if (!info.isFile()) return false
    } catch {
      return false
    }
  }
  return true
}
