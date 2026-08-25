import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const outputDir = path.dirname(fileURLToPath(import.meta.url))
const sourceDir = path.join(outputDir, 'source')
const template = await readFile(path.join(sourceDir, 'index.template.html'), 'utf8')
const localeDir = path.join(sourceDir, 'locales')
const localeFiles = (await readdir(localeDir)).filter((file) => file.endsWith('.json')).sort()

for (const localeFile of localeFiles) {
  const locale = JSON.parse(await readFile(path.join(localeDir, localeFile), 'utf8'))
  const used = new Set()
  const output = template.replace(/\{\{([a-z0-9.-]+)\}\}/g, (_, key) => {
    if (!Object.hasOwn(locale.values, key)) {
      throw new Error(`${localeFile}: missing value for ${key}`)
    }
    used.add(key)
    return locale.values[key]
  })
  const unused = Object.keys(locale.values).filter((key) => !used.has(key))
  if (unused.length) {
    throw new Error(`${localeFile}: unused values: ${unused.join(', ')}`)
  }
  if (!/^(?:[A-Za-z-]+\/)?index\.html$/.test(locale.output)) {
    throw new Error(`${localeFile}: invalid output path ${locale.output}`)
  }

  const destination = path.join(outputDir, locale.output)
  await mkdir(path.dirname(destination), { recursive: true })
  await writeFile(destination, output)
  console.log(`[site] wrote ${path.relative(outputDir, destination)}`)
}
