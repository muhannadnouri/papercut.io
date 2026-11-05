import { execFileSync } from "node:child_process"
import { createHash } from "node:crypto"
import {
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  writeFile,
} from "node:fs/promises"
import { tmpdir } from "node:os"
import {
  dirname,
  join,
  relative,
} from "node:path"
import { pathToFileURL } from "node:url"
import { gunzipSync } from "node:zlib"
import {
  afterEach,
  beforeEach,
  expect,
  it,
  vi,
} from "vitest"
import { ROOT } from "../../lib/paths.js"
import { buildSearchIndex } from "../build-search-index.js"

const api = vi.hoisted(() => ({
  createIndex: vi.fn(),
  addHTMLFile: vi.fn(),
  getFiles: vi.fn(),
  close: vi.fn(),
}))
vi.mock("pagefind", () => ({
  createIndex: api.createIndex,
  close: api.close,
}))
let site

async function html(name, content = "<html lang='en'><title>Book</title><body>Reading books</body></html>", root = site) {
  const path = join(root, "documents", name)
  await mkdir(dirname(path), { recursive: true })
  await writeFile(path, content)
}

beforeEach(async () => {
  site = await mkdtemp(join(tmpdir(), "papercut-index-test-"))
  vi.resetAllMocks()
  api.createIndex.mockResolvedValue({
    index: api,
    errors: [],
  })
  api.addHTMLFile.mockResolvedValue({ errors: [] })
  api.getFiles.mockResolvedValue({
    errors: [],
    files: [{
      path: "pagefind-entry.json",
      content: Buffer.from("{}"),
    }],
  })
})
afterEach(async () => await rm(site, {
  recursive: true,
  force: true,
}))

it("sorts relative HTML paths and awaits each file before indexing the next", async () => {
  await html("z.html")
  await html("folder/a space.html")
  await html("a.html")
  await html("skip.txt")
  let active = 0
  let peak = 0
  api.addHTMLFile.mockImplementation(async () => {
    peak = Math.max(peak, ++active)
    await Promise.resolve()
    active--
    return { errors: [] }
  })
  await buildSearchIndex(site)
  expect(api.addHTMLFile.mock.calls.map(([file]) => file.sourcePath))
    .toEqual(["documents/a.html", "documents/folder/a space.html", "documents/z.html"])
  expect(api.addHTMLFile.mock.calls[0][0].content).toContain("Reading books")
  expect(peak).toBe(1)
  expect(await readFile(join(site, "pagefind/pagefind-entry.json"), "utf8")).toBe("{}")
  expect(api.close).toHaveBeenCalledOnce()
})

it.each(["createIndex", "addHTMLFile", "getFiles"])("fails on %s errors and closes the service", async method => {
  await html("a.html")
  api[method].mockResolvedValue({ errors: ["indexing failed"] })
  await expect(buildSearchIndex(site)).rejects.toThrow("indexing failed")
  if (method !== "getFiles") {
    expect(api.getFiles).not.toHaveBeenCalled()
  }
  expect(api.close).toHaveBeenCalledOnce()
})

it("closes the service when indexing rejects", async () => {
  await html("a.html")
  api.addHTMLFile.mockRejectedValue(new Error("backend stopped"))
  await expect(buildSearchIndex(site)).rejects.toThrow("backend stopped")
  expect(api.getFiles).not.toHaveBeenCalled()
  expect(api.close).toHaveBeenCalledOnce()
})

it("rejects an empty bundled-document input", async () => {
  await mkdir(join(site, "documents"))
  await expect(buildSearchIndex(site)).rejects.toThrow("No bundled HTML")
  expect(api.createIndex).not.toHaveBeenCalled()
  expect(api.close).toHaveBeenCalledOnce()
})

it.each([
  {
    path: "pagefind-entry.json",
    content: Buffer.alloc(0),
  },
  {
    path: "../outside",
    content: Buffer.from("escape"),
  },
])("rejects an invalid bundle and keeps the previous output ($path)", async invalid => {
  await html("a.html")
  await buildSearchIndex(site)
  const valid = {
    path: "pagefind-entry.json",
    content: Buffer.from('{"new":true}'),
  }
  api.getFiles.mockResolvedValue({
    errors: [],
    files: invalid.path === valid.path ? [invalid] : [valid, invalid],
  })
  await expect(buildSearchIndex(site)).rejects.toThrow(/Empty Pagefind output|Invalid Pagefind output path/)
  expect(await readFile(join(site, "pagefind/pagefind-entry.json"), "utf8")).toBe("{}")
  expect((await readdir(site)).sort()).toEqual(["documents", "pagefind"])
  expect(api.close).toHaveBeenCalledTimes(2)
})

it("produces identical real indexes across fresh roots/file orders and preserves CLI URLs/content", async () => {
  const files = ["z.html", "nested/index.html", "nested/café space.html"]
  const manifests = []
  let firstRoot
  for (let run = 0; run < 4; run++) {
    const root = join(site, String(run))
    for (const name of run % 2 ? [...files].reverse() : files) {
      await html(name, `<html lang="en"><title>${name}</title><body>Search reading ${name}</body></html>`, root)
    }
    const script = pathToFileURL(join(ROOT, "scripts/search/build-search-index.js")).href
    execFileSync(process.execPath, ["--input-type=module", "-e",
      `import { buildSearchIndex } from ${JSON.stringify(script)}; await buildSearchIndex(${JSON.stringify(root)})`], { cwd: ROOT })
    const output = join(root, "pagefind")
    expect(JSON.parse(await readFile(join(output, "pagefind-entry.json"), "utf8")).languages.en).toBeDefined()
    const entries = await readdir(output, {
      recursive: true,
      withFileTypes: true,
    })
    const manifest = {}
    for (const entry of entries.filter(entry => entry.isFile())) {
      const path = join(entry.parentPath, entry.name)
      manifest[relative(output, path)] = createHash("sha256").update(await readFile(path)).digest("hex")
    }
    manifests.push(manifest)
    firstRoot ??= root
  }
  for (const manifest of manifests.slice(1)) {
    expect(manifest).toEqual(manifests[0])
  }
  async function fragments() {
    const dir = join(firstRoot, "pagefind/fragment")
    const result = {}
    for (const name of await readdir(dir)) {
      const bytes = gunzipSync(await readFile(join(dir, name)))
      const fragment = JSON.parse(bytes.subarray(bytes.indexOf(123)).toString())
      delete fragment.page_number
      result[fragment.url] = fragment
    }
    return result
  }
  const indexed = await fragments()
  await rm(join(firstRoot, "pagefind"), { recursive: true })
  execFileSync(process.execPath, [join(ROOT, "node_modules/pagefind/lib/runner/bin.cjs"),
    "--site", firstRoot, "--glob", "documents/**/*.html"], { cwd: ROOT })
  expect(await fragments()).toEqual(indexed)
  expect(Object.keys(indexed).sort()).toEqual([
    "/documents/nested/", "/documents/nested/café space.html", "/documents/z.html",
  ])
}, 30000)
