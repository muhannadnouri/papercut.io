import { describe, expect, it } from 'vitest'
import { PhysicalPosition } from '@tauri-apps/api/dpi'
import { documentDropAction } from './documentDrop'

describe('documentDropAction', () => {
  it('shows feedback while hovering and forwards only unblocked drops', () => {
    const position = new PhysicalPosition(0, 0)
    const drop = {
      type: 'drop' as const,
      paths: ['/tmp/book.epub'],
      position,
    }

    expect(documentDropAction({ type: 'enter', paths: drop.paths, position }, false))
      .toEqual({ active: true })
    expect(documentDropAction({ type: 'leave' }, false)).toEqual({ active: false })
    expect(documentDropAction(drop, false)).toEqual({
      active: false,
      paths: ['/tmp/book.epub'],
    })
    expect(documentDropAction(drop, true)).toEqual({ active: false })
  })
})
