/**
 * Selecting with the mouse in a read-only note (a viewer's, a commenter's): kit-editor refuses the
 * engine's text tool every gesture there (its pointer may edit), so a press and drag over the note
 * set the selection through the engine's `selection.set`, a viewing command, as the text tool would
 * have: the press puts the caret at the point, the drag moves the selection's other end. A
 * commenter selects a passage to comment on this way; a viewer to copy it.
 */
import { useCallback, useRef, type PointerEvent as ReactPointerEvent } from 'react'
import type { KitCanvasView } from '@kition/kit-editor'
import type { NoteRun } from './useNote'

type Point = { x: number; y: number }

export function useReadOnlySelect(on: boolean, view: () => KitCanvasView | null, run: NoteRun, shown: () => void) {
  const drag = useRef<{ anchor: number | null; busy: boolean; next: Point | null } | null>(null)

  /** The character at `point` (document pixels): the caret goes there and its place is read back. */
  const charAt = useCallback(
    async (point: Point) => {
      await run('selection.set', point)
      return (await run<{ focus: number }>('selection.get', {})).focus
    },
    [run],
  )

  /** Moves the selection's other end to the latest point, one engine round at a time. */
  const pump = useCallback(async () => {
    const state = drag.current
    if (!state || state.busy || !state.next) return
    const point = state.next
    state.next = null
    state.busy = true
    try {
      const focus = await charAt(point)
      if (state.anchor === null) state.anchor = focus
      else await run('selection.set', { anchor: state.anchor, focus })
      shown()
    } catch {
      // A refused or failed read leaves the selection as it was.
    } finally {
      state.busy = false
      if (state.next) void pump()
    }
  }, [charAt, run, shown])

  const pointAt = (event: ReactPointerEvent<HTMLElement>): Point | null => {
    const shownView = view()
    const surface = event.currentTarget.querySelector('[data-testid="kit-canvas-surface"]') ?? event.currentTarget
    if (!shownView) return null
    const box = surface.getBoundingClientRect()
    return { x: shownView.x + (event.clientX - box.left) / shownView.zoom, y: shownView.y + (event.clientY - box.top) / shownView.zoom }
  }

  const onPointerDown = (event: ReactPointerEvent<HTMLElement>) => {
    // Only a primary press on the note itself (not a control drawn over it).
    if (!on || event.button !== 0 || !(event.target instanceof HTMLCanvasElement)) return
    const point = pointAt(event)
    if (!point) return
    drag.current = { anchor: null, busy: false, next: point }
    void pump()
  }
  const onPointerMove = (event: ReactPointerEvent<HTMLElement>) => {
    const state = drag.current
    if (!on || !state || (event.buttons & 1) === 0) return
    const point = pointAt(event)
    if (!point) return
    state.next = point
    void pump()
  }
  const onPointerUp = (event: ReactPointerEvent<HTMLElement>) => {
    const state = drag.current
    if (!state) return
    const point = pointAt(event)
    if (point && state.anchor !== null) state.next = point
    void pump().then(() => {
      if (drag.current === state && !state.busy && !state.next) drag.current = null
    })
  }
  return on ? { onPointerDown, onPointerMove, onPointerUp } : {}
}
