import { join } from "node:path"
import { ensureLocalJdk } from "./jdk.js"
import {
  pathSeparator,
  run,
} from "../process.js"
import { ROOT } from "../paths.js"
import { tauriCommand } from "../tauri.js"

export { ensureLocalJdk }

// Force Android builds onto a known JDK while still allowing extra native env.
export async function androidBuildEnv(extra = {}) {
  const javaHome = await ensureLocalJdk()
  return {
    ...process.env,
    JAVA_HOME: javaHome,
    PATH: join(javaHome, "bin") + pathSeparator() + process.env.PATH,
    ...extra,
  }
}

// One Android entry path keeps normal and native-TTS APK builds consistent.
export async function runTauriAndroidBuild(args, extraEnv = {}) {
  const env = await androidBuildEnv(extraEnv)
  const tauri = tauriCommand(args)
  console.log("[android-build] JAVA_HOME=" + env.JAVA_HOME)
  await run(tauri.command, tauri.args, {
    cwd: ROOT,
    env,
    label: "tauri " + args.join(" "),
  })
}
