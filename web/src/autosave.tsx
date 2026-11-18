/**
 * Notes save themselves, as Kition's native Markdown editor did: a moment after the last change,
 * and when the page goes away. Not a read-only note, not while the host disabled Save (UI-C2), and
 * not while a change on disk waits for the person (they choose to merge it or keep theirs).
 * Kit UI protocol section 3: a save is the kit's own `host/write` of its document.
 */
import { useEffect, useRef } from 'react'
import { isFileChangedError, toast, useKitEditor, useUi, createKitStrings } from '@kition/kit-editor'
import { message } from './useNote'

/** How long the note stays unsaved after a change before it saves itself. */
export const AUTOSAVE_MS = 1000

/** Tests that need unsaved edits set this key to `off` in the page's storage, before the note opens. */
export const AUTOSAVE_SWITCH = 'kit.note.autosave'

function autosaveOn() {
  try {
    return window.localStorage.getItem(AUTOSAVE_SWITCH) !== 'off'
  } catch {
    return true
  }
}
const switchboard = { enabled: autosaveOn() }

/** Saves the open note on its own; renders nothing. `waiting` is a change on disk the person has not decided on. */
export function NoteAutosave({ waiting, t }: { waiting: boolean; t: ReturnType<typeof createKitStrings> }) {
  const { session, host } = useKitEditor()
  const ui = useUi()
  // The host's choice, not the person's: a person who hid the Save button still has their notes saved.
  const save = ui.state('action:save')
  const allowed = !host.readOnly && !save.denied && !save.hostDisabled
  const { dirty, saving } = session
  const revision = session.change.revision
  const latest = useRef({ session, allowed, waiting })
  latest.current = { session, allowed, waiting }

  useEffect(() => {
    if (!dirty || saving || !allowed || waiting || !switchboard.enabled) return undefined
    const timer = window.setTimeout(() => {
      const now = latest.current
      if (!switchboard.enabled || !now.allowed || now.waiting || !now.session.dirty) return
      // A file changed on disk is refused (kit-editor's save): the person is asked instead.
      void now.session.save().catch((error: unknown) => {
        if (!isFileChangedError(error)) toast('error', t('autosaveFailed'), message(error))
      })
    }, AUTOSAVE_MS)
    return () => window.clearTimeout(timer)
  }, [allowed, dirty, revision, saving, t, waiting])

  // The page goes away (its tab closed, Kition quit): save what is unsaved, as far as there is time.
  useEffect(() => {
    const flush = () => {
      const now = latest.current
      if (switchboard.enabled && now.allowed && !now.waiting && now.session.dirty && !now.session.saving) void now.session.save().catch(() => undefined)
    }
    window.addEventListener('pagehide', flush)
    return () => window.removeEventListener('pagehide', flush)
  }, [])

  return null
}
