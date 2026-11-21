import assert from "node:assert/strict"
import {
  mkdir,
  readFile,
} from "node:fs/promises"
import { join } from "node:path"
import { ROOT } from "../lib/paths.js"
import { run } from "../lib/process.js"
import {
  output,
  source,
  SOURCE_CACHE,
} from "./source-build.js"

const version = "0.28.2"
const lock = JSON.parse(await readFile(join(ROOT, "package-lock.json"), "utf8"))
assert.equal(lock.packages["node_modules/esbuild"].version, version, "Review the esbuild source pin after an npm version change")
const checkout = await source("esbuild", "https://github.com/evanw/esbuild.git", "609683d892977362a0f99026cb74b96263d728a9")
const tools = join(SOURCE_CACHE, "host-tools")
await mkdir(tools, { recursive: true })
const binary = join(tools, "esbuild")
await run("go", ["build", "-mod=readonly", "-trimpath", "-o", binary, "./cmd/esbuild"], {
  cwd: checkout,
  env: {
    ...process.env,
    CGO_ENABLED: "0",
    GOTOOLCHAIN: "local",
  },
})
assert.equal(output(binary, ["--version"]).trim(), version)
