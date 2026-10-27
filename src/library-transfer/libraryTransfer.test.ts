import {
  expect,
  it,
} from 'vitest'
import { formatPairingCode } from './libraryTransfer'

it('groups pairing codes as they are typed or pasted', () => {
  expect(formatPairingCode('abcd')).toBe('ABCD')
  expect(formatPairingCode('abcde')).toBe('ABCD-E')
  expect(formatPairingCode('2345abcdghjk')).toBe('2345-ABCD-GHJK')
  expect(formatPairingCode(' 2345 abcd–ghjk ')).toBe('2345-ABCD-GHJK')
  // Deleting the symbol after a dash also drops the dash, so backspace never sticks.
  expect(formatPairingCode('ABCD-')).toBe('ABCD')
  expect(formatPairingCode('2345-ABCD-GHJKX')).toBe('2345-ABCD-GHJK')
  expect(formatPairingCode('')).toBe('')
})
