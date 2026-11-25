import { spawn } from 'node:child_process'
import { once } from 'node:events'
import {
  expect,
  it,
  vi,
} from 'vitest'
import {
  stopDriver,
  withCleanup,
} from '../smoke-cleanup.mjs'

it('retains the primary exception when cleanup also fails', async () => {
  const primary = Error('passage not visible')
  const secondary = Error('profile locked')
  const log = vi.spyOn(console, 'error').mockImplementation(() => {})
  try {
    await expect(withCleanup('profile', () => Promise.reject(primary), () => Promise.reject(secondary)))
      .rejects.toBe(primary)
    expect(log).toHaveBeenCalledWith('profile', '(secondary cleanup failure)', secondary)
  } finally {
    log.mockRestore()
  }
})

it('fails a passing case when cleanup fails', async () => {
  const failure = Error('driver did not stop')
  await expect(withCleanup('driver', () => true, () => {
    throw failure
  })).rejects.toBe(failure)
})

it.skipIf(process.platform !== 'linux')('stops a child that outlives its driver group leader', async () => {
  // The inherited stdout pipe closes only after the surviving sleep process releases it.
  const driver = spawn('sh', ['-c', 'sleep 15 &'], { detached: true })
  const closed = once(driver, 'close', { signal: AbortSignal.timeout(3000) })
  try {
    await once(driver, 'exit')
    expect(driver.exitCode).toBe(0)
    stopDriver(driver)
    await closed
  } finally {
    driver.stdout.destroy()
    driver.stderr.destroy()
    try {
      process.kill(-driver.pid, 'SIGKILL')
    } catch (error) {
      if (error.code !== 'ESRCH') {
        throw error
      }
    }
  }
})

it.skipIf(process.platform !== 'linux')('accepts an already-gone group', async () => {
  const driver = spawn('true', [], { detached: true })
  await once(driver, 'close')
  expect(() => stopDriver(driver)).not.toThrow()
})

it.skipIf(process.platform !== 'linux')('propagates group signal errors other than ESRCH', () => {
  const denied = Object.assign(Error('permission denied'), { code: 'EPERM' })
  const kill = vi.spyOn(process, 'kill').mockImplementation(() => {
    throw denied
  })
  try {
    expect(() => stopDriver({
      pid: 123,
      exitCode: 0,
    })).toThrow(denied)
  } finally {
    kill.mockRestore()
  }
})
