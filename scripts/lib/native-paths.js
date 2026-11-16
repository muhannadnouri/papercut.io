import { resolve } from "node:path"

import {
  ROOT,
  SRC_TAURI_DIR,
} from "./paths.js"

// Match desktopBuildEnv's ROOT-relative target normalization before Tauri changes cwd.
export function nativeCacheDir(cache, archive, env = process.env) {
  return resolve(ROOT, env.CARGO_TARGET_DIR || resolve(SRC_TAURI_DIR, "target"), cache, archive, "lib")
}
