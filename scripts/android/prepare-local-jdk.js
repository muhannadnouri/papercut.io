import { ensureLocalJdk } from "../lib/android/build-env.js"

// Public setup command for sandboxed environments without a system JDK 17.
const force = process.argv.includes("--force")
const javaHome = await ensureLocalJdk({ force })
console.log("[local-jdk] ready " + javaHome)
