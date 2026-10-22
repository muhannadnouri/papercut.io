import { execFileSync, spawn } from 'node:child_process'
import { dirname, join } from 'node:path'
import { pathToFileURL } from 'node:url'
import type { Readable, Writable } from 'node:stream'
import { StringDecoder } from 'node:string_decoder'
import { mkdirSync, writeFileSync } from 'node:fs'

export const browser = [process.env.CHROME_BIN, 'chromium', 'chromium-browser', 'google-chrome', 'google-chrome-stable']
  .find(candidate => {
    if (!candidate) return false
    try { execFileSync(candidate, ['--version'], { stdio: 'ignore', timeout: 5000 }); return true } catch { return false }
  })
if (process.env.CI && !browser) throw new Error('Viewport capture tests require Chrome/Chromium; set CHROME_BIN.')

export async function runBrowserFixture(directory: string, frame: string, width = 620, screenshotPath?: string) {
  // A DevTools pipe waits for real asynchronous layout; dump-dom virtual time can stop between RAFs.
  const child = spawn(browser!, ['--headless', '--no-sandbox', '--disable-gpu', '--disable-background-networking',
    '--allow-file-access-from-files', '--disable-dev-shm-usage', '--no-first-run', '--remote-debugging-pipe',
    `--user-data-dir=${join(directory, 'profile')}`, 'about:blank'],
  { stdio: ['ignore', 'ignore', 'pipe', 'pipe', 'pipe'] })
  const input = child.stdio[3] as Writable, output = child.stdio[4] as Readable
  const closed = new Promise<void>((resolve) => child.once('close', () => resolve()))
  const commands = new Map<number, { resolve: (value: Record<string, unknown>) => void; reject: (error: Error) => void }>()
  let nextId = 0, buffer = '', errors = ''
  const decoder = new StringDecoder('utf8')
  child.stderr?.on('data', (data) => { errors = (errors + data).slice(-2000) })
  const fail = (error: Error) => {
    for (const command of commands.values()) command.reject(error)
    commands.clear()
  }
  child.on('error', fail)
  input.on('error', fail)
  output.on('error', fail)
  child.on('exit', () => fail(new Error(`Browser exited: ${errors}`)))
  output.on('data', (data) => {
    buffer += decoder.write(data)
    let end: number
    while ((end = buffer.indexOf('\0')) >= 0) {
      const message = JSON.parse(buffer.slice(0, end))
      buffer = buffer.slice(end + 1)
      const command = commands.get(message.id)
      if (!command) continue
      commands.delete(message.id)
      if (message.error) command.reject(new Error(JSON.stringify(message.error)))
      else command.resolve(message.result)
    }
  })
  const send = (method: string, params: Record<string, unknown> = {}, sessionId?: string) =>
    new Promise<Record<string, unknown>>((resolve, reject) => {
      const id = ++nextId
      commands.set(id, { resolve, reject })
      input.write(JSON.stringify({ id, method, params, sessionId }) + '\0')
    })
  const timer = setTimeout(() => { fail(new Error(`Browser fixture timed out: ${errors}`)); child.kill() }, 15_000)
  try {
    const target = await send('Target.createTarget', { url: 'about:blank' })
    const session = await send('Target.attachToTarget', { targetId: target.targetId, flatten: true })
    const sessionId = String(session.sessionId)
    await send('Page.enable', {}, sessionId)
    await send('Emulation.setDeviceMetricsOverride', { width: width, height: 600, deviceScaleFactor: 1, mobile: false }, sessionId)
    await send('Page.addScriptToEvaluateOnNewDocument', { source: `window.captureResult=new Promise(resolve=>{addEventListener('message',({data})=>{if(data?.type==='capture-result')resolve(JSON.stringify(data.result))});addEventListener('error',event=>resolve(JSON.stringify({error:event.message})));addEventListener('unhandledrejection',event=>resolve(JSON.stringify({error:String(event.reason)})))});` }, sessionId)
    await send('Page.navigate', { url: pathToFileURL(frame).href }, sessionId)
    const evaluated = await send('Runtime.evaluate', { expression: 'window.captureResult', awaitPromise: true, returnByValue: true }, sessionId)
    const value = evaluated.result as { value?: string }
    if (!value.value) throw new Error(`No capture result: ${JSON.stringify(evaluated)}`)
    const result = JSON.parse(value.value)
    if (result.error) throw new Error(result.error)
    if (screenshotPath) {
      const screenshot = await send('Page.captureScreenshot', { format: 'png' }, sessionId)
      mkdirSync(dirname(screenshotPath), { recursive: true })
      writeFileSync(screenshotPath, Buffer.from(String(screenshot.data), 'base64'))
    }
    return result
  } finally { clearTimeout(timer); child.kill(); await closed }
}
