import assert from "node:assert/strict"
import {
  readFile,
  readdir,
} from "node:fs/promises"
import {
  join,
  relative,
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
    const result = await index.writeFiles({ outputPath: join(site, "pagefind") })
    assert(!result.errors.length, result.errors.join("\n"))
    console.log(`[pagefind] indexed ${paths.length} bundled HTML documents in path order`)
  } finally {
    await pagefind.close()
  }
}

if (isMain(import.meta.url)) {
  await buildSearchIndex()
}
