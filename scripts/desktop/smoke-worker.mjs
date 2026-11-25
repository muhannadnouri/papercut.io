// Internal worker launched by smoke-desktop inside the disposable desktop.
import assert from 'node:assert/strict'
import {
  execFileSync,
  spawn,
} from 'node:child_process'
import {
  closeSync,
  openSync,
  writeFileSync,
} from 'node:fs'
import { createServer } from 'node:net'
import { pathToFileURL } from 'node:url'
import {
  basename,
  join,
} from 'node:path'
import { setTimeout as delay } from 'node:timers/promises'
import { remote } from 'webdriverio'

const [application, report, caseFile] = process.argv.slice(2)
const caseId = basename(report)
const { phases } = await import(pathToFileURL(caseFile).href)
let step = 'start driver'
function progress(label) {
  step = label
  console.log(caseId, label)
}

/** Use the tested executable or its isolated X window to identify the app. The driver and AppImage launcher have separate process lifetimes. */
function appPid() {
  if (process.platform === 'win32') {
    const output = execFileSync('powershell', ['-NoProfile', '-Command',
      '(Get-Process | Where-Object { $_.Path -eq $env:PAPERCUT_SMOKE_APP }).Id'],
    {
      encoding: 'utf8',
      env: {
        ...process.env,
        PAPERCUT_SMOKE_APP: application,
      },
    })
    return Number(output.trim())
  }
  const window = execFileSync('xdotool', ['search', '--onlyvisible', '--name', '^Papercut$'], { encoding: 'utf8' }).trim()
  return Number(execFileSync('xdotool', ['getwindowpid', window], { encoding: 'utf8' }))
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
while (nativePort === port) {
  nativePort = await availablePort()
}
const log = openSync(join(report, 'driver.log'), 'w')
const driver = spawn('tauri-driver', ['--port', String(port), '--native-port', String(nativePort)], {
  stdio: ['ignore', log, log],
  detached: process.platform !== 'win32',
})
closeSync(log)
let launchError
driver.on('error', error => {
  launchError = error
})
const started = Date.now()
try {
  for (;;) {
    if (launchError) {
      throw launchError
    }
    if (driver.exitCode !== null || Date.now() - started > 15000) {
      throw Error('Driver startup failed, see driver.log')
    }
    try {
      if ((await fetch('http://127.0.0.1:' + port + '/status', { signal: AbortSignal.timeout(1000) })).ok) {
        break
      }
    } catch { /* The native driver is still starting. */ }
    await delay(150)
  }
  const launch = { application }
  if (process.platform === 'win32') {
    // EdgeDriver otherwise allocates a fresh browser profile for each session.
    launch.webviewOptions = { userDataFolder: process.env.WEBVIEW2_USER_DATA_FOLDER }
  }
  let previousPid
  for (const phase of phases) {
    progress('launch app')
    const browser = await remote({
      hostname: '127.0.0.1',
      port,
      path: '/',
      logLevel: 'warn',
      connectionRetryCount: 0,
      connectionRetryTimeout: 45000,
      waitforTimeout: 15000,
      capabilities: { 'tauri:options': launch },
    })
    let pid
    try {
      await browser.setWindowSize(1050, 740)
      await browser.$('//button[@role="tab" and .//span[normalize-space(.)="Library"]]').waitForDisplayed()
      pid = appPid()
      assert.ok(Number.isSafeInteger(pid) && pid > 1 && pid !== previousPid, 'Expected a new Papercut process')
      console.log(caseId, 'App PID', pid)
      await phase(browser, progress)
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
        if (process.platform !== 'win32' || !/invalid session id|session deleted/i.test(error.message)) {
          throw error
        }
      }
    }
    progress('confirm process exit')
    for (let attempt = 0; ; attempt++) {
      try {
        process.kill(pid, 0)
      } catch (error) {
        if (error.code === 'ESRCH') {
          break
        }
        throw error
      }
      if (attempt === 80) {
        throw Error('Papercut process still running: ' + pid)
      }
      await delay(125)
    }
    previousPid = pid
  }
  console.log(caseId, 'passed. Journey ms', Date.now() - started)
} catch (error) {
  writeFileSync(join(report, 'failure.txt'), caseId + ' ' + step + '\n' + error.stack)
  throw error
} finally {
  if (driver.pid && driver.exitCode === null) {
    if (process.platform === 'win32') {
      execFileSync('taskkill', ['/PID', String(driver.pid), '/T', '/F'], { stdio: 'ignore' })
    } else {
      process.kill(-driver.pid, 'SIGTERM')
    }
  }
}
