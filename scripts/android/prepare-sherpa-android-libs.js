import { isMain } from "../lib/process.js"
import {
  androidSherpaLibDir,
  ensureAndroidSherpaLibs,
} from "../lib/android/sherpa.js"

export { androidSherpaLibDir, ensureAndroidSherpaLibs }

// Public setup command; importing this module must not download or copy files.
if (isMain(import.meta.url)) {
  const force = process.argv.includes("--force")
  await ensureAndroidSherpaLibs({
    force,
    abi: process.env.PAPERCUT_ANDROID_ABI ?? "arm64-v8a",
  })
}
