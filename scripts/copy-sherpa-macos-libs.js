import { copyMacosSherpaLibs } from "./lib/macos/sherpa.js"

// Tauri beforeBundle hook: copy shared dylibs into resources after Cargo fetches them.
await copyMacosSherpaLibs()
