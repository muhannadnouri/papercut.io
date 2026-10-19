import { currentDesktopPlatform, desktopBuildEnv } from "./lib/desktop/platform.js"
import { ensureLinuxSharedResourceDir } from "./lib/desktop/linux.js"
import { run } from "./lib/process.js"
import { tauriCommand } from "./lib/tauri.js"

if (currentDesktopPlatform() === "linux") ensureLinuxSharedResourceDir()

const { command, args } = tauriCommand(["dev", "--features", "native-tts-shared"])
await run(command, args, {
  env: desktopBuildEnv(currentDesktopPlatform(), process.env),
})
