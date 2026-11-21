import { buildAndroidSourceLibs } from "../lib/android/source.js"

await buildAndroidSourceLibs({ abi: process.env.PAPERCUT_ANDROID_ABI ?? "arm64-v8a" })
