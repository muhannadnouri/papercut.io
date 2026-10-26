/** Snapshot before controls take focus; never infer selection direction from anchor/focus. */
export function readerSelection(root: HTMLElement): { range: Range;
  isCurrent: () => boolean } | null {
  const doc = root.ownerDocument; const selection = doc.getSelection()
  if (!selection || selection.rangeCount !== 1 || selection.isCollapsed) {
    return null
  }
  const range = selection.getRangeAt(0).cloneRange()
  if (!root.contains(range.startContainer) || !root.contains(range.endContainer)) {
    return null
  }
  const { startContainer, startOffset, endContainer, endOffset } = range
  return {
    range,
    isCurrent: () => {
      if (!root.isConnected || !root.contains(startContainer) || !root.contains(endContainer)) {
        return false
      }
      const current = doc.getSelection()
      if (current && !current.isCollapsed && current.rangeCount === 1) {
        const next = current.getRangeAt(0)
        return next.startContainer === startContainer && next.startOffset === startOffset &&
        next.endContainer === endContainer && next.endOffset === endOffset
      }
      // Tab/button focus can collapse selection; the reader intent epoch owns explicit invalidation.
      return !current || current.isCollapsed
    },
  }
}

/** Walk forward from the start boundary, stopping at the selection's end. */
export function selectedTextStart(range: Range): { node: Text;
  offset: number } | null {
  const doc = range.startContainer.ownerDocument!
  const walker = doc.createTreeWalker(range.commonAncestorContainer, NodeFilter.SHOW_TEXT)
  let current: Node | null = range.startContainer
  let offset = range.startOffset
  if (current.nodeType !== Node.TEXT_NODE) {
    if (offset) {
      current = current.childNodes[offset - 1]
      while (current.lastChild) {
        current = current.lastChild
      }
    }
    walker.currentNode = current
    current = walker.nextNode(); offset = 0
  } else {
    walker.currentNode = current
  }
  while (current) {
    const node = current as Text
    for (let at = offset; at < node.length; at++) {
      if (!selectionContainsPoint(range, node, at)) {
        return null
      }
      if (!/\s/u.test(node.data[at])) {
        return {
          node,
          offset: at,
        }
      }
    }
    current = walker.nextNode(); offset = 0
  }
  return null
}

export function selectionContainsPoint(range: Range, node: Text, offset: number): boolean {
  return range.comparePoint(node, offset) === 0 && !(range.endContainer === node && offset >= range.endOffset)
}

/** Tab snapshots before focus loss; new reader input invalidates any retained menu target. */
export function bindSelectionIntent(shell: HTMLElement, actions: { snapshot: () => void;
  invalidate: () => void }): () => void {
  const doc = shell.ownerDocument
  const pointer = (event: PointerEvent) => {
    if (!(event.target instanceof Element) || !event.target.closest('.audio-controls, .audio-playback-dock, .reader-passage-action')) {
      actions.invalidate()
    }
  }
  const key = (event: KeyboardEvent) => {
    if (event.target instanceof Element && event.target.closest('.audio-controls, .audio-playback-dock, .reader-passage-action')) {
      return
    }
    if (event.key === 'Tab') {
      actions.snapshot()
    } else {
      actions.invalidate()
    }
  }
  doc.addEventListener('pointerdown', pointer, true); doc.addEventListener('keydown', key, true)
  return () => {
    doc.removeEventListener('pointerdown', pointer, true); doc.removeEventListener('keydown', key, true)
  }
}
