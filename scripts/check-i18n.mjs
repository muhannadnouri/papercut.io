import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const localeUrls = {
  en: new URL('../src/i18n/locales/en.json', import.meta.url),
  ar: new URL('../src/i18n/locales/ar.json', import.meta.url),
}

const resources = Object.fromEntries(
  Object.entries(localeUrls).map(([locale, url]) => [
    locale,
    JSON.parse(readFileSync(fileURLToPath(url), 'utf8')),
  ]),
)

const referenceKeys = flattenResource(resources.en)
for (const [locale, resource] of Object.entries(resources)) {
  const keys = flattenResource(resource)
  const missing = [...referenceKeys.keys()].filter((key) => !keys.has(key))
  const extra = [...keys.keys()].filter((key) => !referenceKeys.has(key))
  const empty = [...keys].filter(([, value]) => !value.trim()).map(([key]) => key)
  if (missing.length || extra.length || empty.length) {
    throw new Error([
      `${locale} translation resource is invalid.`,
      missing.length ? `Missing: ${missing.join(', ')}` : '',
      extra.length ? `Extra: ${extra.join(', ')}` : '',
      empty.length ? `Empty: ${empty.join(', ')}` : '',
    ].filter(Boolean).join('\n'))
  }
}

console.log(`i18n resources valid: ${Object.keys(resources).join(', ')}`)

// Flatten nested resources so locale files can stay readable while key parity
// remains a small dependency-free build check.
function flattenResource(value, prefix = '', output = new Map()) {
  for (const [key, child] of Object.entries(value)) {
    const path = prefix ? `${prefix}.${key}` : key
    if (typeof child === 'string') {
      output.set(path, child)
    } else if (child && typeof child === 'object' && !Array.isArray(child)) {
      flattenResource(child, path, output)
    } else {
      throw new Error(`Translation value must be a string or object: ${path}`)
    }
  }
  return output
}
