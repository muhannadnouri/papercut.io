import { execFileSync } from 'node:child_process'

// Report teardown errors separately when the case already failed. A cleanup-only failure still fails the case.
export async function withCleanup(label, run, cleanup) {
  let failed = false
  try {
    return await run()
  } catch (error) {
    failed = true
    throw error
  } finally {
    try {
      await cleanup()
    } catch (error) {
      if (!failed) {
        throw error
      }
      console.error(label, '(secondary cleanup failure)', error)
    }
  }
}

/** Linux starts the driver as a group leader. Its native WebDriver can survive the leader's exit. */
export function stopDriver(driver) {
  if (!driver.pid) {
    return
  }
  if (process.platform === 'win32') {
    if (driver.exitCode === null) {
      execFileSync('taskkill', ['/PID', String(driver.pid), '/T', '/F'], { stdio: 'ignore' })
    }
    return
  }
  try {
    process.kill(-driver.pid, 'SIGTERM')
  } catch (error) {
    if (error.code !== 'ESRCH') {
      throw error
    }
  }
}
