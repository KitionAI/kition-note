/**
 * Escape in Focus (KitPointer 4.4): it cancels first, and leaves Focus only when there is nothing
 * to cancel. The note's text tool takes every editing key, Escape included (it collapses the
 * selection), so kit-editor sees every Escape as taken; here, before the tool, an Escape with no
 * text selected and nothing else open (a field, a menu, a dialog, the note's own pop-ups, which
 * take Escape on the window first) leaves Focus instead (regressions/issue-focus-escape-text-tool).
 */
import { useEffect, useRef } from 'react'
import { useFrameDisplay } from '@kition/kit-editor'

/** Something open that Escape closes before it leaves Focus. */
const OPEN = '[role="menu"], [role="dialog"], .kui-dialog-backdrop, .kui-drop-panel'

function isField(target: EventTarget | null) {
  if (target instanceof HTMLTextAreaElement) return !target.classList.contains('kui-text-sink')
  return target instanceof HTMLInputElement || target instanceof HTMLSelectElement || (target instanceof HTMLElement && target.isContentEditable)
}

/** Leaves Focus on an Escape that has nothing to cancel; `selected` says whether text is selected now. */
export function useFocusEscape(selected: () => boolean) {
  const display = useFrameDisplay()
  const focus = display?.focus ?? false
  const exit = display?.exitFocus
  const latest = useRef(selected)
  latest.current = selected
  useEffect(() => {
    if (!focus || !exit) return undefined
    const keys = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || event.defaultPrevented || event.isComposing) return
      if (event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) return
      if (isField(event.target) || document.querySelector(OPEN) || latest.current()) return
      event.preventDefault()
      event.stopPropagation()
      exit()
    }
    // The document's capture phase: after the note's pop-ups (the window's) and before the tool.
    document.addEventListener('keydown', keys, true)
    return () => document.removeEventListener('keydown', keys, true)
  }, [exit, focus])
}

/** `useFocusEscape` for the editor: drawn inside the frame, whose display it reads. */
export function FocusEscape({ selected }: { selected: () => boolean }) {
  useFocusEscape(selected)
  return null
}
