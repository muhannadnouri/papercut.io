import { existsSync } from "node:fs"
import {
  cp,
  mkdir,
  stat,
} from "node:fs/promises"
import { join } from "node:path"
import {
  SHERPA_LINUX_OPTIONAL_LIBS,
  SHERPA_LINUX_SHARED_OUT_DIR,
  SHERPA_LINUX_SHARED_SOURCE_DIR,
  SHERPA_REQUIRED_LIBS,
} from "./constants.js"

// Shared-link desktop builds must bundle sherpa libs next to the installed app.
export async function copyLinuxSherpaLibs({ platform = process.env.TAURI_ENV_PLATFORM, linkMode = process.env.PAPERCUT_NATIVE_TTS_LINK } = {}) {
  if (platform && platform !== "linux") {
    return
  }
  if (linkMode === "static") {
    return
  }

  await assertLinuxSharedLibsExist()
  await mkdir(SHERPA_LINUX_SHARED_OUT_DIR, { recursive: true })

  for (const lib of [...SHERPA_REQUIRED_LIBS, ...SHERPA_LINUX_OPTIONAL_LIBS]) {
    const source = join(SHERPA_LINUX_SHARED_SOURCE_DIR, lib)
    if (existsSync(source)) {
      await cp(source, join(SHERPA_LINUX_SHARED_OUT_DIR, lib), { force: true })
    }
  }

  console.log("[sherpa-linux-libs] bundled shared libraries from " + SHERPA_LINUX_SHARED_SOURCE_DIR)
}

// Fail with build guidance instead of producing an installer missing runtime libs.
async function assertLinuxSharedLibsExist() {
  for (const lib of SHERPA_REQUIRED_LIBS) {
    try {
      const info = await stat(join(SHERPA_LINUX_SHARED_SOURCE_DIR, lib))
      if (!info.isFile()) {
        throw new Error(lib + " is not a file")
      }
    } catch (err) {
      throw new Error(
        "Missing " + lib + " at " + SHERPA_LINUX_SHARED_SOURCE_DIR + ". Build with --features native-tts-shared so sherpa-onnx downloads its Linux shared library archive before bundling. " + err,
      )
    }
  }
}
