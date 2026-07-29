import { cpSync, existsSync, rmSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = dirname(dirname(fileURLToPath(import.meta.url)))
const pdfJsRoot = join(repoRoot, 'node_modules', 'pdfjs-dist')
const outputRoot = join(repoRoot, 'public', 'pdfjs')
const assetDirectories = ['standard_fonts', 'wasm']

for (const directory of assetDirectories) {
  const source = join(pdfJsRoot, directory)
  if (!existsSync(source)) {
    throw new Error(`Missing PDF.js runtime assets: ${source}. Run npm install first.`)
  }
}

rmSync(outputRoot, { recursive: true, force: true })
for (const directory of assetDirectories) {
  cpSync(join(pdfJsRoot, directory), join(outputRoot, directory), {
    recursive: true,
  })
}

console.log(`[pdfjs] prepared ${assetDirectories.join(' and ')} runtime assets`)
