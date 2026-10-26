// @vitest-environment jsdom
import {
  afterEach,
  expect,
  it,
  vi,
} from 'vitest'
import { bindReaderBrowse } from './readerBrowse'
afterEach(() => document.body.replaceChildren())
it('responds to reader intent, ignores programmatic scroll, toolbar input and zoom; cleans up', () => {
  const shell = document.createElement('div'); shell.innerHTML = '<header><button>Pause</button></header><main class="document-view"><p>Text</p><input /></main>'
  document.body.append(shell)
  const browse = vi.fn(); const dispose = bindReaderBrowse(shell, browse); const text = shell.querySelector('p')!
  text.dispatchEvent(new Event('scroll', { bubbles: true })); expect(browse).not.toHaveBeenCalled()
  text.dispatchEvent(new WheelEvent('wheel', {
    deltaY: 10,
    bubbles: true,
    ctrlKey: true,
  })); expect(browse).not.toHaveBeenCalled()
  text.dispatchEvent(new WheelEvent('wheel', {
    deltaY: 10,
    bubbles: true,
  })); expect(browse).toHaveBeenCalledTimes(1)
  document.body.dispatchEvent(new KeyboardEvent('keydown', {
    key: 'PageDown',
    bubbles: true,
  })); expect(browse).toHaveBeenCalledTimes(2)
  shell.querySelector('input')!.dispatchEvent(new KeyboardEvent('keydown', {
    key: 'ArrowDown',
    bubbles: true,
  })); expect(browse).toHaveBeenCalledTimes(2)
  shell.querySelector('button')!.dispatchEvent(new WheelEvent('wheel', {
    deltaY: 10,
    bubbles: true,
  })); expect(browse).toHaveBeenCalledTimes(2)
  const pointer = (type: string, x: number) => {
    const event = new Event(type, { bubbles: true }); Object.assign(event, {
      pointerId: 1,
      clientX: x,
      clientY: 50,
    }); text.dispatchEvent(event)
  }
  pointer('pointerdown', 60); pointer('pointermove', 62); expect(browse).toHaveBeenCalledTimes(2)
  pointer('pointermove', 90); expect(browse).toHaveBeenCalledTimes(3)
  // Momentum's later scroll events do not restore following or add new intents.
  text.dispatchEvent(new Event('scroll', { bubbles: true })); expect(browse).toHaveBeenCalledTimes(3)
  dispose(); text.dispatchEvent(new WheelEvent('wheel', {
    deltaY: 10,
    bubbles: true,
  })); expect(browse).toHaveBeenCalledTimes(3)
})
