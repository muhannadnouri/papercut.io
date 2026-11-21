import { copyLinuxSherpaLibs } from "../lib/linux/sherpa.js"

// Tauri beforeBundle hook: copy shared libs into resources after Cargo fetches them.
await copyLinuxSherpaLibs()
