/**
 * The note scrolls as a page does in Obsidian, not as a drawing canvas: up and down only, never
 * above its top or past its end, with a thin scroll bar at the right. kit-editor's canvas pans and
 * zooms freely (a drawing's way), so the editor takes the wheel itself and keeps every view in
 * those bounds.
 */
import { useRef, type PointerEvent as ReactPointerEvent } from 'react'
import type { KitCanvasView } from '@kition/sdk/editor'

/** The view's top-left in bounds: centered across the document, its top between the document's top and its end. */
export function scrollBounds(view: KitCanvasView, doc: { width: number; height: number }, y = view.y) {
  const visibleWidth = view.width / view.zoom
  const visibleHeight = view.height / view.zoom
  const x = (doc.width - visibleWidth) / 2
  const bottom = Math.max(0, doc.height - visibleHeight)
  return { x, y: Math.min(Math.max(0, y), bottom) }
}

/** Pixels a wheel event asks to scroll (lines and pages to pixels). */
export function wheelPixels(event: WheelEvent, pageHeight: number) {
  if (event.deltaMode === 1) return event.deltaY * 24
  if (event.deltaMode === 2) return event.deltaY * pageHeight
  return event.deltaY
}

const MIN_THUMB = 28

/** The scroll bar: its thumb shows the part of the note in view; dragging it or clicking the track scrolls. */
export function NoteScrollbar({ view, docHeight, onScroll }: { view: KitCanvasView | null; docHeight: number; onScroll: (y: number) => void }) {
  const drag = useRef<{ startY: number; startTop: number } | null>(null)
  if (!view || docHeight <= 0) return null
  const visible = view.height / view.zoom
  if (visible >= docHeight - 1) return null
  const track = view.height
  const thumb = Math.max(MIN_THUMB, (visible / docHeight) * track)
  const room = track - thumb
  const top = (Math.min(Math.max(0, view.y), docHeight - visible) / (docHeight - visible)) * room
  const toY = (thumbTop: number) => (Math.min(Math.max(0, thumbTop), room) / room) * (docHeight - visible)
  const down = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.preventDefault()
    event.stopPropagation()
    const target = event.currentTarget
    const rect = target.parentElement?.getBoundingClientRect()
    const onThumb = event.target !== target
    if (!onThumb && rect) onScroll(toY(event.clientY - rect.top - thumb / 2))
    drag.current = { startY: event.clientY, startTop: onThumb || !rect ? top : event.clientY - rect.top - thumb / 2 }
    target.setPointerCapture(event.pointerId)
  }
  const move = (event: ReactPointerEvent<HTMLDivElement>) => {
    // The canvas under the bar never sees the pointer (no hover answers while it drags).
    event.stopPropagation()
    const started = drag.current
    if (started) onScroll(toY(started.startTop + event.clientY - started.startY))
  }
  const up = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.stopPropagation()
    drag.current = null
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId)
  }
  return (
    <div className="note-scrollbar" data-testid="note-scrollbar" onPointerDown={down} onPointerMove={move} onPointerUp={up} onPointerCancel={up}>
      <div className="note-scrollbar-thumb" style={{ top, height: thumb }} />
    </div>
  )
}
