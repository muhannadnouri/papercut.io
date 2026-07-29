import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { pathToFileURL } from 'node:url'
import path from 'node:path'

const DEFAULT_FIXTURE = 'scripts/fixtures/translation/es-en-quality.json'

function parseArgs(argv) {
  const options = {
    fixture: DEFAULT_FIXTURE,
    output: '',
    serverUrl: 'http://127.0.0.1:8080',
  }

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index]
    if (argument === '--dry-run') {
      options.dryRun = true
      continue
    }
    if (argument === '--help') {
      options.help = true
      continue
    }
    if (!argument.startsWith('--') || index + 1 >= argv.length) {
      throw new Error(`Invalid argument: ${argument}`)
    }
    const key = argument.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase())
    if (!['fixture', 'model', 'output', 'serverUrl'].includes(key)) {
      throw new Error(`Unknown option: ${argument}`)
    }
    options[key] = argv[index + 1]
    index += 1
  }

  return options
}

function words(text) {
  return text.toLocaleLowerCase().match(/[\p{L}\p{N}]+/gu) ?? []
}

export function tokenBigramDice(left, right) {
  const leftWords = words(left)
  const rightWords = words(right)
  if (leftWords.length < 2 || rightWords.length < 2) {
    return leftWords.join(' ') === rightWords.join(' ') ? 1 : 0
  }

  const counts = new Map()
  for (let index = 0; index < leftWords.length - 1; index += 1) {
    const bigram = `${leftWords[index]}\0${leftWords[index + 1]}`
    counts.set(bigram, (counts.get(bigram) ?? 0) + 1)
  }

  let matches = 0
  for (let index = 0; index < rightWords.length - 1; index += 1) {
    const bigram = `${rightWords[index]}\0${rightWords[index + 1]}`
    const remaining = counts.get(bigram) ?? 0
    if (remaining > 0) {
      matches += 1
      counts.set(bigram, remaining - 1)
    }
  }

  return (2 * matches) / (leftWords.length + rightWords.length - 2)
}

export function buildTranslationRequest(fixture, segment, model) {
  return {
    model,
    messages: [{
      role: 'user',
      content: [{
        type: 'text',
        source_lang_code: fixture.sourceLanguage,
        target_lang_code: fixture.targetLanguage,
        text: segment.source,
      }],
    }],
    temperature: 0,
    max_tokens: 1024,
  }
}

function validateFixture(fixture) {
  if (!fixture.sourceLanguage || !fixture.targetLanguage || !Array.isArray(fixture.segments)) {
    throw new Error('Fixture requires sourceLanguage, targetLanguage, and segments')
  }
  if (fixture.segments.length === 0) {
    throw new Error('Fixture must contain at least one segment')
  }
  for (const segment of fixture.segments) {
    if (!segment.id || !segment.source || !segment.reference || !segment.opus) {
      throw new Error('Every fixture segment requires id, source, reference, and opus')
    }
  }
}

async function resolveModel(serverUrl, requestedModel) {
  if (requestedModel) return requestedModel

  const response = await fetch(`${serverUrl}/v1/models`)
  if (!response.ok) {
    throw new Error(`Could not list models from ${serverUrl}: HTTP ${response.status}`)
  }
  const body = await response.json()
  const model = body.data?.[0]?.id
  if (!model) throw new Error(`No model is loaded at ${serverUrl}`)
  return model
}

function translatedText(body) {
  const content = body.choices?.[0]?.message?.content
  if (typeof content === 'string') return content.trim()
  if (Array.isArray(content)) {
    return content
      .filter((part) => part.type === 'text' && typeof part.text === 'string')
      .map((part) => part.text)
      .join('')
      .trim()
  }
  throw new Error('Translation response did not include assistant text')
}

async function translate(serverUrl, request) {
  const startedAt = performance.now()
  const response = await fetch(`${serverUrl}/v1/chat/completions`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(request),
    signal: AbortSignal.timeout(300_000),
  })
  const body = await response.json()
  if (!response.ok) {
    throw new Error(`TranslateGemma request failed: ${body.error?.message ?? `HTTP ${response.status}`}`)
  }
  return {
    text: translatedText(body),
    elapsedMs: Math.round(performance.now() - startedAt),
    usage: body.usage ?? null,
  }
}

function average(values) {
  return values.reduce((sum, value) => sum + value, 0) / values.length
}

function printHelp() {
  console.log(`Usage:
  npm run translation:compare -- [options]

Options:
  --server-url URL   llama-server URL (default: http://127.0.0.1:8080)
  --model ID         loaded model id; otherwise discovered from /v1/models
  --fixture PATH     aligned JSON fixture (default: ${DEFAULT_FIXTURE})
  --output PATH      report path (default: .cache/translation-quality/<timestamp>.json)
  --dry-run          validate the fixture and print the first request without inference
  --help             show this help`)
}

async function main() {
  const options = parseArgs(process.argv.slice(2))
  if (options.help) {
    printHelp()
    return
  }

  const fixture = JSON.parse(await readFile(options.fixture, 'utf8'))
  validateFixture(fixture)

  if (options.dryRun) {
    console.log(JSON.stringify(buildTranslationRequest(fixture, fixture.segments[0], options.model ?? '<loaded-model>'), null, 2))
    return
  }

  const serverUrl = options.serverUrl.replace(/\/+$/, '')
  const model = await resolveModel(serverUrl, options.model)
  const segments = []

  for (const segment of fixture.segments) {
    console.log(`Translating ${segment.id}...`)
    const candidate = await translate(serverUrl, buildTranslationRequest(fixture, segment, model))
    segments.push({
      ...segment,
      candidate: candidate.text,
      elapsedMs: candidate.elapsedMs,
      usage: candidate.usage,
      opusReferenceDice: tokenBigramDice(segment.opus, segment.reference),
      candidateReferenceDice: tokenBigramDice(candidate.text, segment.reference),
    })
  }

  const report = {
    fixture: fixture.name,
    sourceLanguage: fixture.sourceLanguage,
    targetLanguage: fixture.targetLanguage,
    model,
    generatedAt: new Date().toISOString(),
    summary: {
      segments: segments.length,
      elapsedMs: segments.reduce((sum, segment) => sum + segment.elapsedMs, 0),
      opusReferenceDice: average(segments.map((segment) => segment.opusReferenceDice)),
      candidateReferenceDice: average(segments.map((segment) => segment.candidateReferenceDice)),
    },
    segments,
  }
  const output = options.output || path.join(
    '.cache',
    'translation-quality',
    `translategemma-${new Date().toISOString().replace(/[:.]/g, '-')}.json`,
  )
  await mkdir(path.dirname(output), { recursive: true })
  await writeFile(output, `${JSON.stringify(report, null, 2)}\n`)

  console.table(segments.map((segment) => ({
    segment: segment.id,
    milliseconds: segment.elapsedMs,
    OPUS: segment.opusReferenceDice.toFixed(3),
    TranslateGemma: segment.candidateReferenceDice.toFixed(3),
  })))
  console.log(`Report: ${output}`)
  console.log('Token overlap is a regression diagnostic, not a substitute for human translation review.')
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : ''
if (import.meta.url === invokedPath) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : error)
    process.exitCode = 1
  })
}
