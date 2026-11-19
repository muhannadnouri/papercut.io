import {
  expect,
  it,
} from 'vitest'
import type { OutlineEntry } from '../../viewers/types'
import { initialOutlineExpansionDepth } from './outlineExpansion'

function leaves(count: number): OutlineEntry[] {
  return Array.from({ length: count }, (_, i) => ({
    label: 'Section',
    target: String(i),
    children: [],
  }))
}

it('opens every level at 20 entries and closes branches at 21, counting publisher groups', () => {
  const chapter = {
    label: 'Chapter',
    target: 'chapter',
    children: leaves(17),
  }
  const group = {
    label: 'Part',
    target: null,
    children: [chapter],
  }
  const entries = [group, ...leaves(1)]
  expect(initialOutlineExpansionDepth(entries)).toBe(Infinity)
  chapter.children.push(...leaves(1))
  expect(initialOutlineExpansionDepth(entries)).toBe(0)
})

it.each([null, 'title'])('opens exactly one level under a lone root with destination %s at 21', target => {
  const chapter = {
    label: 'Contents',
    target: 'chapter',
    children: leaves(18),
  }
  const entries = [{
    label: 'Contents',
    target,
    children: [chapter],
  }]
  expect(initialOutlineExpansionDepth(entries)).toBe(Infinity)
  chapter.children.push(...leaves(1))
  expect(initialOutlineExpansionDepth(entries)).toBe(1)
})
