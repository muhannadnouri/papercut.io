import assert from "node:assert/strict"
import {
  cp,
  copyFile,
  mkdir,
  readFile,
  rename,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises"
import {
  basename,
  join,
} from "node:path"
import { gzipSync } from "node:zlib"
import { ROOT } from "../lib/paths.js"
import { ANDROID_RUST_TOOLCHAIN } from "../lib/android/constants.js"
import { run } from "../lib/process.js"
import {
  emscriptenEnv,
  output,
  receipt,
  source,
  SOURCE_CACHE,
} from "./source-build.js"

const version = "1.4.0"
const revision = "8c3de15a6b5ab603985d98062e748463daf42d65"
const lock = JSON.parse(await readFile(join(ROOT, "package-lock.json"), "utf8"))
assert.equal(lock.packages["node_modules/pagefind"].version, version, "Review the Pagefind source pin after an npm version change")

const env = {
  ...process.env,
  GIT_VERSION: version,
  CARGO_TARGET_DIR: join(SOURCE_CACHE, "pagefind-target"),
}
delete env.PAGEFIND_DEV
const wasmOpt = join(emscriptenEnv().EMSDK, "upstream/bin/wasm-opt")
const bindgen = join(SOURCE_CACHE, "wasm-tools/bin/wasm-bindgen")
assert.equal(output(bindgen, ["--version"]).trim(), "wasm-bindgen 0.2.100")
const checkout = await source("pagefind", "https://github.com/Pagefind/pagefind.git", revision)
const work = join(SOURCE_CACHE, "pagefind-work")
await rm(work, {
  recursive: true,
  force: true,
})
await cp(checkout, work, {
  recursive: true,
  filter: path => basename(path) !== ".git",
})
await run(process.execPath, [".backstage/version.cjs"], {
  cwd: work,
  env,
})
await run("patch", ["-p1", "--batch", "--forward", "--fuzz=0", "-i", join(ROOT, "scripts/fdroid/pagefind-locks.patch")], { cwd: work })

const browser = join(work, "pagefind_web")
const vendor = join(work, "pagefind/vendor")
await mkdir(join(vendor, "wasm"), { recursive: true })
const manifest = await readFile(join(browser, "Cargo.toml"), "utf8")
// Keep the upstream standalone WASM crate out of any Cargo workspace above the checkout.
await writeFile(join(browser, "Cargo.toml"), manifest + "\n[workspace]\n")
const languages = [...manifest.matchAll(/^([a-z]{2}) = \["pagefind_stem\/\w+"\]$/gm)].map(match => match[1])
assert.equal(languages.length, 30, "Review the pinned Pagefind language features")
// The upstream CommonJS script inherits Papercut's ESM package type under the build cache.
await rename(join(browser, "build.js"), join(browser, "build.cjs"))
const glue = join(vendor, `pagefind_web.${version}.js`)
const cargo = ["+" + ANDROID_RUST_TOOLCHAIN, "build", "--release", "--locked", "--jobs", "2"]
for (const language of ["unknown", ...languages]) {
  const features = language === "unknown" ? [] : ["--features", language]
  await run("cargo", [...cargo, "--target", "wasm32-unknown-unknown", ...features], {
    cwd: browser,
    env,
  })
  await run(bindgen, ["--target", "no-modules", "--out-dir", "pkg",
    join(env.CARGO_TARGET_DIR, "wasm32-unknown-unknown/release/pagefind_web.wasm")], { cwd: browser })
  await run(wasmOpt, ["--all-features", "-Oz", "pkg/pagefind_web_bg.wasm", "-o", "pkg/optimized.wasm"], { cwd: browser })
  const bytes = await readFile(join(browser, "pkg/optimized.wasm"))
  assert(WebAssembly.validate(bytes), "Invalid Pagefind WASM for " + language)
  await writeFile(join(vendor, "wasm", `pagefind_web_bg.${language}.${version}.wasm.gz`),
    gzipSync(Buffer.concat([Buffer.from("pagefind_dcd"), bytes]), { level: 9 }))
  if (language === "unknown") {
    await run(process.execPath, ["build.cjs"], { cwd: browser })
    await copyFile(join(browser, "pkg/pagefind_web.js"), glue)
  }
}
await copyFile(glue, join(browser, "pkg/pagefind_web.js"))

for (const directory of ["pagefind_web_js", "pagefind_ui/default", "pagefind_ui/modular", "pagefind_playground"]) {
  const cwd = join(work, directory)
  await run("npm", ["ci", "--ignore-scripts", "--omit=optional", "--no-fund", "--no-audit"], { cwd })
  // Reuse the same esbuild client and binary as the app, including F-Droid's source-built binary.
  const esbuild = join(cwd, "node_modules/esbuild")
  await rm(esbuild, { recursive: true })
  await symlink(join(ROOT, "node_modules/esbuild"), esbuild, "dir")
  await run(process.execPath, [directory === "pagefind_playground" ? "_build.js" : "build.js"], {
    cwd,
    env,
  })
}

await run("cargo", [...cargo, "--features", "extended"], {
  cwd: work,
  env,
})
const binary = join(env.CARGO_TARGET_DIR, "release/pagefind")
assert.equal(output(binary, ["--version"]).trim(), `pagefind ${version}`)
const destination = join(SOURCE_CACHE, "source-pagefind")
await mkdir(destination, { recursive: true })
await copyFile(binary, join(destination, "pagefind"))
await receipt(destination, { pagefind: revision }, ["pagefind"], {
  rust: ANDROID_RUST_TOOLCHAIN,
  wasmBindgen: "0.2.100",
  esbuild: lock.packages["node_modules/esbuild"].version,
})
