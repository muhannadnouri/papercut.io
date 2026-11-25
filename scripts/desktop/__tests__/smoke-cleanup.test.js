import {
  expect,
  it,
  vi,
} from 'vitest'
import { withCleanup } from '../smoke-cleanup.mjs'

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
