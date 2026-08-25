import type { DragDropEvent } from '@tauri-apps/api/webview'

/** Reduce native enter/over/drop/leave events to the only two UI decisions.
 * Keeping blocked drops path-free prevents a stale event from starting work. */
export function documentDropAction(
  event: DragDropEvent,
  blocked: boolean,
): { active: boolean; paths?: string[] } {
  if (event.type === 'enter' || event.type === 'over') return { active: true }
  if (event.type === 'drop') {
    return blocked || event.paths.length === 0
      ? { active: false }
      : { active: false, paths: event.paths }
  }
  return { active: false }
}
