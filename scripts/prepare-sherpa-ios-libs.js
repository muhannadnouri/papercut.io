import { ensureIosSherpaLibs } from "./lib/ios/sherpa.js"

try {
  await ensureIosSherpaLibs({ force: process.argv.includes("--force") })
} catch (err) {
  console.error("[sherpa-ios-libs] " + (err instanceof Error ? err.message : String(err)))
  process.exit(1)
}
