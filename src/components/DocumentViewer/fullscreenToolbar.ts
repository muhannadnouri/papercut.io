const TAP_MOVEMENT_LIMIT_PX = 10

export interface PointerPosition {
  x: number
  y: number
}

/** Distinguish a deliberate toolbar-toggle tap from scrolling or text selection. */
export function isFullscreenToolbarTap(
  start: PointerPosition,
  end: PointerPosition,
): boolean {
  return Math.abs(end.x - start.x) <= TAP_MOVEMENT_LIMIT_PX &&
    Math.abs(end.y - start.y) <= TAP_MOVEMENT_LIMIT_PX
}
