/**
 * Tag autocomplete: typing `#` and a tag's first letters after a space (or at a line's start)
 * offers the note's tags (inline `#tags` and its front matter's `tags`) that start with them.
 * Arrow keys move, Enter or Tab completes, Escape closes. Completing replaces the typed letters
 * with the tag and a space.
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import { createKitStrings, toast } from '@kition/sdk/editor'
import { message, type NoteRun } from './useNote'
import type { Caret } from './SlashMenu'

type Strings = ReturnType<typeof createKitStrings>

const TAG = /(?:^|\s)#([\p{L}\p{N}_/-]+)$/u
const SHOWN = 8

export function TagMenu({ caret, place, run, tags, t, readOnly }: { caret: Caret | null; place: { left: number; top: number; height: number } | null; run: NoteRun; tags: () => Promise<string[]>; t: Strings; readOnly: boolean }) {
  const [selected, setSelected] = useState(0)
  const [dismissed, setDismissed] = useState<number | null>(null)
  const [known, setKnown] = useState<string[]>([])
  const query = caret && !readOnly ? TAG.exec(caret.before)?.[1] : undefined
  const typing = query !== undefined
  useEffect(() => {
    if (!typing) return
    let live = true
    void tags()
      .then((names) => live && setKnown(names))
      .catch(() => live && setKnown([]))
    return () => {
      live = false
    }
  }, [tags, typing])
  const wanted = (query ?? '').toLowerCase()
  const items = [...new Set(known)].filter((name) => name.toLowerCase().startsWith(wanted) && name !== query).slice(0, SHOWN)
  const open = typing && caret !== null && dismissed !== caret.focus
  const latest = useRef({ items, selected, caret, query })
  latest.current = { items, selected, caret, query }

  useEffect(() => setSelected(0), [query])

  const choose = useCallback(
    async (index: number) => {
      const name = latest.current.items[index]
      if (!name) return
      try {
        // The caret now, not as last drawn: a key typed just before Enter may not have redrawn yet.
        const at = await run<Caret>('selection.get', {})
        const typed = TAG.exec(at.before)?.[1]
        if (typed === undefined) return
        await run('selection.set', { anchor: at.focus - [...typed].length, focus: at.focus })
        await run('selection.replace', { text: `${name} ` })
      } catch (error) {
        toast('error', t('commandFailed'), message(error))
      }
    },
    [run, t],
  )

  useEffect(() => {
    if (!open || !items.length) return undefined
    const keys = (event: KeyboardEvent) => {
      const count = latest.current.items.length
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        setSelected((value) => (value + (event.key === 'ArrowDown' ? 1 : count - 1)) % count)
      } else if (event.key === 'Enter' || event.key === 'Tab') {
        void choose(latest.current.selected)
      } else if (event.key === 'Escape') {
        setDismissed(latest.current.caret?.focus ?? null)
      } else {
        return
      }
      event.preventDefault()
      event.stopPropagation()
    }
    window.addEventListener('keydown', keys, true)
    return () => window.removeEventListener('keydown', keys, true)
  }, [choose, items.length, open])

  if (!open || !place || !items.length) return null
  return (
    <div className="note-slash" role="listbox" aria-label={t('tags.suggestions')} data-testid="note-tags" style={{ left: place.left, top: place.top + place.height + 4 }}>
      {items.map((name, index) => (
        <button
          key={name}
          type="button"
          role="option"
          aria-selected={index === selected}
          className={`note-slash-item${index === selected ? ' is-selected' : ''}`}
          data-testid="note-tag-item"
          onMouseEnter={() => setSelected(index)}
          onMouseDown={(event) => {
            event.preventDefault()
            void choose(index)
          }}
        >
          #{name}
        </button>
      ))}
    </div>
  )
}
