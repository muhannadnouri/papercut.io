// Internal worker launched by smoke-desktop inside the disposable desktop.
import assert from 'node:assert/strict'
import { execFileSync, spawn } from 'node:child_process'
import { closeSync, openSync, writeFileSync } from 'node:fs'
import { createServer } from 'node:net'
import { join } from 'node:path'
import { setTimeout as delay } from 'node:timers/promises'
import { remote } from 'webdriverio'

const [application, report] = process.argv.slice(2)
const title = 'Northern Reading Log'
const target = 'Amberquartz'
const paragraphs = Array.from({ length: 80 }, (_, i) => 'Reading entry ' + (i + 1) + '. The library opens early.')
paragraphs[55] = target + ' was found along the riverbank.'
const story = [title, ...paragraphs].join('\n\n')
let step = 'start driver'
function progress(label) {
  step = label
  console.log('Desktop smoke:', label)
}

/** Use the tested executable or its isolated X window to identify the app. The driver and AppImage launcher have separate process lifetimes. */
function appPid() {
  if (process.platform === 'win32') {
    const output = execFileSync('powershell', ['-NoProfile', '-Command',
      '(Get-Process | Where-Object { $_.Path -eq $env:PAPERCUT_SMOKE_APP }).Id'],
    { encoding: 'utf8', env: { ...process.env, PAPERCUT_SMOKE_APP: application } })
    return Number(output.trim())
  }
  const window = execFileSync('xdotool', ['search', '--onlyvisible', '--name', '^Papercut$'], { encoding: 'utf8' }).trim()
  return Number(execFileSync('xdotool', ['getwindowpid', window], { encoding: 'utf8' }))
}

async function click(browser, selector) {
  const element = await browser.$(selector)
  await element.waitForClickable()
  await element.click()
}
const tab = name => '//button[@role="tab" and .//span[normalize-space(.)="' + name + '"]]'

/** Measure the word in place. A scrolling WebDriver element lookup here could move the reader and conceal a navigation regression. */
async function passagePosition(browser) {
  return browser.execute((title, target) => {
    const body = document.querySelector('.document-viewer-shown .document-html-content')
    if (!body || !body.textContent.includes(title)) return null
    const walker = document.createTreeWalker(body, NodeFilter.SHOW_TEXT)
    while (walker.nextNode()) {
      const node = walker.currentNode
      const start = node.textContent.indexOf(target)
      if (start === -1) continue
      const range = document.createRange()
      range.setStart(node, start)
      range.setEnd(node, start + target.length)
      const rect = range.getBoundingClientRect()
      // Hit testing also excludes text hidden by a sticky header or clipped by the reader.
      const hit = document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2)
      if (rect.width <= 0) return null
      return { visible: hit !== null && node.parentElement.contains(hit), top: rect.top, height: innerHeight }
    }
    return null
  }, title, target)
}

async function visiblePassage(browser) {
  await browser.waitUntil(async () => (await passagePosition(browser))?.visible === true,
    { timeoutMsg: 'Expected the target word in the visible reader passage' })
}

/** Establish that the target starts off screen before exercising search. Both sessions use the same fixture and profile. */
async function readAndBookmark(browser) {
  progress('paste text through Library')
  await click(browser, tab('Library'))
  await click(browser, '.document-import-btn')
  await click(browser, '[role="menuitem"][aria-label^="Paste Text"]')
  await browser.$('.paste-text-dialog input').setValue(title)
  const textarea = await browser.$('.paste-text-dialog textarea')
  await textarea.click()
  // WebKit drops literal newlines in WebDriver text input. Enter produces real paragraph breaks.
  await browser.elementSendKeys(textarea.elementId, story.replaceAll('\n', '\uE007'))
  assert.equal(await textarea.getValue(), story, 'Expected the complete multi-paragraph fixture')
  await click(browser, '.paste-text-dialog button[type="submit"]')
  await browser.waitUntil(async () => {
    const position = await passagePosition(browser)
    return position && position.top > 2 * position.height
  }, { timeoutMsg: 'Fixture target should begin multiple screens below the opening' })
  await click(browser, '.doc-header .back-button')
  progress('search the imported document')
  await click(browser, tab('Search'))
  await click(browser, '//label[.//input[@name="search-mode"] and contains(., "All Words")]')
  await browser.$('.search-input').setValue(target)
  await browser.keys('Enter')
  await click(browser, '//button[.//bdi[text()="' + title + '"] and contains(@class,"result-card-primary")]')
  progress('check search destination')
  await visiblePassage(browser)
  progress('save bookmark')
  await click(browser, 'button[aria-label="Save Bookmark"]')
  await browser.$('button[aria-label="Bookmark Actions"]').waitForDisplayed()
}

/** Open from Library so only bookmark restoration can account for the passage being visible after restart. */
async function reopen(browser) {
  progress('reopen from Library')
  await click(browser, tab('Library'))
  await click(browser, '//button[@aria-pressed and contains(., "Documents (")]')
  await browser.$('.document-filter-input').setValue(title)
  await click(browser, '//button[.//bdi[text()="' + title + '"]]')
  progress('check restored bookmark')
  await visiblePassage(browser)
  await click(browser, '//div[@role="status" and contains(., "Jumped to bookmark.")]//button[normalize-space(.)="Top"]')
  await browser.waitUntil(async () => {
    const position = await passagePosition(browser)
    return position && position.top > 2 * position.height
  }, { timeoutMsg: 'Top should return to the beginning of the document' })
}

async function availablePort() {
  const socket = createServer()
  await new Promise((resolve, reject) => socket.once('error', reject).listen(0, '127.0.0.1', resolve))
  const { port } = socket.address()
  await new Promise(resolve => socket.close(resolve))
  return port
}
const port = await availablePort()
let nativePort = await availablePort()
while (nativePort === port) nativePort = await availablePort()
const log = openSync(join(report, 'driver.log'), 'w')
const driver = spawn('tauri-driver', ['--port', String(port), '--native-port', String(nativePort)], {
  stdio: ['ignore', log, log], detached: process.platform !== 'win32',
})
closeSync(log)
let launchError
driver.on('error', error => { launchError = error })
const started = Date.now()
try {
  for (;;) {
    if (launchError) throw launchError
    if (driver.exitCode !== null || Date.now() - started > 15000) throw Error('Driver startup failed, see driver.log')
    try {
      if ((await fetch('http://127.0.0.1:' + port + '/status', { signal: AbortSignal.timeout(1000) })).ok) break
    } catch { /* The native driver is still starting. */ }
    await delay(150)
  }
  const launch = { application }
  if (process.platform === 'win32') {
    // EdgeDriver otherwise allocates a fresh browser profile for each session.
    launch.webviewOptions = { userDataFolder: process.env.WEBVIEW2_USER_DATA_FOLDER }
  }
  let previousPid
  for (const journey of [readAndBookmark, reopen]) {
    progress('launch app')
    const browser = await remote({ hostname: '127.0.0.1', port, path: '/', logLevel: 'warn',
      connectionRetryCount: 0, connectionRetryTimeout: 45000, waitforTimeout: 15000,
      capabilities: { 'tauri:options': launch } })
    let pid
    try {
      await browser.setWindowSize(1050, 740)
      await browser.$(tab('Library')).waitForDisplayed()
      pid = appPid()
      assert.ok(Number.isSafeInteger(pid) && pid > 1 && pid !== previousPid, 'Expected a new Papercut process')
      console.log('App PID', pid)
      await journey(browser)
      if (process.platform === 'win32') {
        progress('close the Windows app normally')
        // Let WebView2 finish its storage writes before releasing the driver session.
        execFileSync('powershell', ['-NoProfile', '-Command',
          '$app = Get-Process -Id ' + pid + '; if (!$app.CloseMainWindow() -or !$app.WaitForExit(10000)) { exit 1 }'])
      }
    } catch (error) {
      await Promise.allSettled([
        browser.saveScreenshot(join(report, 'failure.png')),
        browser.execute(() => document.body.innerText).then(text => writeFileSync(join(report, 'visible.txt'), text)),
      ])
      throw error
    } finally {
      try {
        await browser.deleteSession()
      } catch (error) {
        if (process.platform !== 'win32' || !/invalid session id|session deleted/i.test(error.message)) throw error
      }
    }
    progress('confirm process exit')
    for (let attempt = 0; ; attempt++) {
      try { process.kill(pid, 0) } catch (error) { if (error.code === 'ESRCH') break; throw error }
      if (attempt === 80) throw Error('Papercut process still running: ' + pid)
      await delay(125)
    }
    previousPid = pid
  }
  console.log('Verified bookmark after process restart. Journey ms', Date.now() - started)
} catch (error) {
  writeFileSync(join(report, 'failure.txt'), step + '\n' + error.stack)
  throw error
} finally {
  if (driver.pid && driver.exitCode === null) {
    if (process.platform === 'win32') execFileSync('taskkill', ['/PID', String(driver.pid), '/T', '/F'], { stdio: 'ignore' })
    else process.kill(-driver.pid, 'SIGTERM')
  }
}
