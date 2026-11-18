/**
 * The wheel over an element drawn over the canvas (a diagram, the properties block) scrolls the
 * note as it does over the canvas: kit-editor listens on its canvas only, so the event is handed on.
 * A sideways wheel over something that scrolls sideways itself (a wide diagram) stays there.
 */
import { useCallback, useRef } from 'react'

function wheel(this: HTMLElement, event: WheelEvent) {
  if (!event.ctrlKey && !event.metaKey && Math.abs(event.deltaX) > Math.abs(event.deltaY)) {
    const target = event.target instanceof Element ? event.target.closest<HTMLElement>('.note-diagram') : null
    if (target && target.scrollWidth > target.clientWidth) return
  }
  const surface = this.parentElement?.querySelector<HTMLElement>('[data-testid="kit-canvas-surface"]')
  if (!surface) return
  event.preventDefault()
  surface.dispatchEvent(new WheelEvent('wheel', event))
}

/** A ref for the element whose wheel scrolls the note. */
export function useWheelToCanvas() {
  const attached = useRef<HTMLElement | null>(null)
  return useCallback((element: HTMLElement | null) => {
    attached.current?.removeEventListener('wheel', wheel)
    attached.current = element
    element?.addEventListener('wheel', wheel, { passive: false })
  }, [])
}
