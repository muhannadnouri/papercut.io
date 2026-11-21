import assert from "node:assert/strict"
import {
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rename,
  rm,
  writeFile,
} from "node:fs/promises"
import {
  dirname,
  isAbsolute,
  join,
  relative,
  resolve,
  sep,
} from "node:path"
import * as pagefind from "pagefind"
import { ROOT } from "../lib/paths.js"
import { isMain } from "../lib/process.js"

export async function buildSearchIndex(site = join(ROOT, "dist")) {
  try {
    const documents = join(site, "documents")
    const entries = await readdir(documents, {
      recursive: true,
      withFileTypes: true,
    })
    const paths = entries.filter(entry => entry.isFile() && entry.name.endsWith(".html"))
      .map(entry => relative(site, join(entry.parentPath, entry.name)).split(sep).join("/"))
      .sort()
    assert(paths.length, "No bundled HTML documents found for Pagefind")
    const { index, errors } = await pagefind.createIndex()
    assert(!errors.length, errors.join("\n"))
    assert(index, "Pagefind did not create an index")
    // Page numbers follow ingestion order; filesystem order differs across checkouts.
    for (const sourcePath of paths) {
      const result = await index.addHTMLFile({
        sourcePath,
        content: await readFile(join(site, sourcePath), "utf8"),
      })
      assert(!result.errors.length, result.errors.join("\n"))
    }
    // Pagefind 1.4 can acknowledge writeFiles before its service flushes the bytes (#1271).
    const result = await index.getFiles()
    assert(!result.errors.length, result.errors.join("\n"))
    assert(result.files.some(file => file.path === "pagefind-entry.json"), "Pagefind did not return its manifest")

    const output = join(site, "pagefind")
    const staging = await mkdtemp(join(site, ".pagefind-"))
    try {
      const bundle = join(staging, "pagefind")
      for (const file of result.files) {
        const target = resolve(bundle, file.path)
        const rel = relative(bundle, target)
        assert(rel && !isAbsolute(file.path) && !isAbsolute(rel) && rel !== ".." && !rel.startsWith(".." + sep),
          "Invalid Pagefind output path: " + file.path)
        assert(file.content.length, "Empty Pagefind output: " + file.path)
        await mkdir(dirname(target), { recursive: true })
        await writeFile(target, file.content, { flag: "wx" })
      }
      await rm(output, {
        recursive: true,
        force: true,
      })
      await rename(bundle, output)
    } finally {
      await rm(staging, {
        recursive: true,
        force: true,
      })
    }
    console.log(`[pagefind] indexed ${paths.length} bundled HTML documents in path order`)
  } finally {
    await pagefind.close()
  }
}

if (isMain(import.meta.url)) {
  await buildSearchIndex()
}
