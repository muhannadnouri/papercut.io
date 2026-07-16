import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const localeUrls = {
  en: new URL('../src/i18n/locales/en.json', import.meta.url),
  ar: new URL('../src/i18n/locales/ar.json', import.meta.url),
}
const PLURAL_SUFFIX = /_(zero|one|two|few|many|other)$/

const resources = Object.fromEntries(
  Object.entries(localeUrls).map(([locale, url]) => [
    locale,
    JSON.parse(readFileSync(fileURLToPath(url), 'utf8')),
  ]),
)

const referenceKeys = flattenResource(resources.en)
const referenceSchema = buildSchema(referenceKeys)
for (const [locale, resource] of Object.entries(resources)) {
  const keys = flattenResource(resource)
  const schema = buildSchema(keys)
  const missing = [...referenceSchema.keys()].filter((key) => !schema.has(key))
  const extra = [...schema.keys()].filter((key) => !referenceSchema.has(key))
  const empty = [...keys].filter(([, value]) => !value.trim()).map(([key]) => key)
  const invalid = validatePluralForms(locale, referenceSchema, schema)
  if (missing.length || extra.length || empty.length || invalid.length) {
    throw new Error([
      `${locale} translation resource is invalid.`,
      missing.length ? `Missing: ${missing.join(', ')}` : '',
      extra.length ? `Extra: ${extra.join(', ')}` : '',
      empty.length ? `Empty: ${empty.join(', ')}` : '',
      invalid.length ? `Plural forms: ${invalid.join(', ')}` : '',
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

// Compare plural message families by base key because languages require
// different CLDR categories (English has two; Arabic has six).
function buildSchema(keys) {
  const schema = new Map()
  for (const key of keys.keys()) {
    const match = key.match(PLURAL_SUFFIX)
    const base = match ? key.slice(0, -match[0].length) : key
    const entry = schema.get(base) ?? { plain: false, forms: new Set() }
    if (match) entry.forms.add(match[1])
    else entry.plain = true
    schema.set(base, entry)
  }
  return schema
}

function validatePluralForms(locale, reference, schema) {
  const issues = []
  const requiredForms = new Set(new Intl.PluralRules(locale).resolvedOptions().pluralCategories)
  for (const [key, referenceEntry] of reference) {
    const entry = schema.get(key)
    if (!entry) continue
    const isPlural = referenceEntry.forms.size > 0
    if (!isPlural && !entry.plain) {
      issues.push(`${key} must be a plain message`)
      continue
    }
    if (!isPlural) continue
    const missing = [...requiredForms].filter((form) => !entry.forms.has(form))
    const extra = [...entry.forms].filter((form) => !requiredForms.has(form))
    if (entry.plain || missing.length || extra.length) {
      issues.push(`${key} requires ${[...requiredForms].join('/')} (${[
        entry.plain ? 'unexpected plain key' : '',
        missing.length ? `missing ${missing.join('/')}` : '',
        extra.length ? `extra ${extra.join('/')}` : '',
      ].filter(Boolean).join('; ')})`)
    }
  }
  return issues
}
