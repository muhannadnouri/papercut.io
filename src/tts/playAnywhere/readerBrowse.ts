/** Observe input intent, never bare scroll events: audio reveals also scroll. */
export function bindReaderBrowse(shell: HTMLElement, browse: () => void): () => void {
  const doc = shell.ownerDocument; const view = doc.defaultView
  if (!view) {
    return () => {}
  }
  const inReader = (target: EventTarget | null) => target instanceof Element && Boolean(target.closest('.document-view')) && shell.contains(target)
  const interactive = (target: EventTarget | null) => target instanceof Element && Boolean(target.closest('button, input, textarea, select, [contenteditable="true"], [role="slider"]'))
  const wheel = (event: WheelEvent) => {
    if (inReader(event.target) && !event.ctrlKey && (event.deltaX || event.deltaY)) {
      browse()
    }
  }
  let pointer: { id: number;
    x: number;
    y: number } | null = null
  const down = (event: PointerEvent) => {
    const target = event.target
    if (interactive(target)) {
      return
    }
    if (inReader(target)) {
      pointer = {
        id: event.pointerId,
        x: event.clientX,
        y: event.clientY,
      }
      if (target instanceof HTMLElement && target.scrollHeight > target.clientHeight) {
        const rect = target.getBoundingClientRect()
        if (event.clientX >= rect.right - Math.max(16, target.offsetWidth - target.clientWidth) || event.clientX <= rect.left + 16) {
          browse()
        }
      }
    }
    // The HTML reader uses the window scrollbar, outside the article surface.
    if ((target === doc.documentElement || target === doc.body) &&
      (event.clientX >= doc.documentElement.clientWidth - 16 || event.clientX <= 16)) {
      browse()
    }
  }
  const move = (event: PointerEvent) => {
    if (pointer?.id !== event.pointerId) {
      return
    }
    if (Math.hypot(event.clientX - pointer.x, event.clientY - pointer.y) < 5) {
      return
    }
    browse(); pointer = null
  }
  const up = () => {
    pointer = null
  }
  const click = (event: MouseEvent) => {
    if (inReader(event.target) && event.target instanceof Element && event.target.closest('a[href]')) {
      browse()
    }
  }
  const key = (event: KeyboardEvent) => {
    if (interactive(event.target) || event.altKey || event.metaKey || event.ctrlKey) {
      return
    }
    const target = event.target
    if (!inReader(target) && target !== doc.body && target !== doc.documentElement) {
      return
    }
    if (['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'PageUp', 'PageDown', 'Home', 'End', ' '].includes(event.key)) {
      browse()
    }
  }
  doc.addEventListener('wheel', wheel, {
    capture: true,
    passive: true,
  })
  doc.addEventListener('pointerdown', down, true)
  doc.addEventListener('pointermove', move, {
    capture: true,
    passive: true,
  })
  doc.addEventListener('pointerup', up, true)
  doc.addEventListener('pointercancel', up, true)
  doc.addEventListener('keydown', key, true)
  doc.addEventListener('click', click, true)
  return () => {
    doc.removeEventListener('wheel', wheel, true); doc.removeEventListener('pointerdown', down, true)
    doc.removeEventListener('pointermove', move, true); doc.removeEventListener('pointerup', up, true)
    doc.removeEventListener('pointercancel', up, true); doc.removeEventListener('keydown', key, true); doc.removeEventListener('click', click, true)
  }
}
