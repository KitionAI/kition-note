/**
 * The slash menu: `/` at a line's start or after a space opens the note's block commands at the
 * caret, filtered by what is typed after it. Arrow keys move, Enter or Tab runs, Escape closes
 * (the keys are the menu's while it is open). Choosing deletes the `/query` and runs the engine
 * command on the line.
 */
import { useCallback, useEffect, useRef, useState } from 'react'
import { createKitStrings, toast, type KitCanvasView } from '@kition/sdk/editor'
import { message, type NoteRun } from './useNote'

type Strings = ReturnType<typeof createKitStrings>

const ITEMS: Array<{ key: string; command: string; words: string }> = [
  { key: 'slash.heading1', command: 'format.heading1', words: 'h1 heading title' },
  { key: 'slash.heading2', command: 'format.heading2', words: 'h2 heading' },
  { key: 'slash.heading3', command: 'format.heading3', words: 'h3 heading' },
  { key: 'slash.bullet', command: 'list.bullet', words: 'bullet list ul' },
  { key: 'slash.numbered', command: 'list.numbered', words: 'numbered list ol' },
  { key: 'slash.task', command: 'list.task', words: 'task todo checkbox' },
  { key: 'slash.quote', command: 'format.quote', words: 'quote blockquote' },
  { key: 'slash.code', command: 'codeBlock.insert', words: 'code block fence' },
  { key: 'slash.table', command: 'table.insert', words: 'table grid' },
  { key: 'slash.callout', command: 'callout.insert', words: 'callout note tip' },
  { key: 'slash.math', command: 'mathBlock.insert', words: 'math formula latex' },
  { key: 'slash.divider', command: 'rule.insert', words: 'divider rule hr line' },
  { key: 'slash.image', command: 'image.insert', words: 'image picture' },
  { key: 'slash.link', command: 'format.link', words: 'link url' },
  { key: 'slash.wikilink', command: 'wikilink.insert', words: 'wikilink note link' },
  { key: 'slash.date', command: 'date.insert', words: 'date today' },
]

/** What the engine's `select.get` answers that the menu reads. */
export type Caret = { focus: number; before: string }

const SLASH = /(?:^|\s)\/([\p{L}\p{N}]*)$/u

export function SlashMenu({ caret, place, run, t, readOnly }: { caret: Caret | null; place: { left: number; top: number; height: number } | null; run: NoteRun; t: Strings; readOnly: boolean }) {
  const [selected, setSelected] = useState(0)
  const [dismissed, setDismissed] = useState<number | null>(null)
  const query = caret && !readOnly ? SLASH.exec(caret.before)?.[1] : undefined
  const open = query !== undefined && caret !== null && dismissed !== caret.focus
  const wanted = (query ?? '').toLowerCase()
  const items = ITEMS.filter((item) => !wanted || t(item.key).toLowerCase().includes(wanted) || item.words.includes(wanted))
  const latest = useRef({ items, selected, caret, query })
  latest.current = { items, selected, caret, query }

  useEffect(() => setSelected(0), [query])

  const choose = useCallback(
    async (index: number) => {
      const item = latest.current.items[index]
      if (!item) return
      try {
        // The caret now, not as last drawn: a key typed just before Enter may not have redrawn yet.
        const at = await run<Caret>('select.get', {})
        const typed = SLASH.exec(at.before)?.[1]
        if (typed === undefined) return
        await run('text.delete', { from: at.focus - [...typed].length - 1, to: at.focus })
        await run(item.command, {})
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
    <div className="note-slash" role="listbox" data-testid="note-slash" style={{ left: place.left, top: place.top + place.height + 4 }}>
      {items.map((item, index) => (
        <button
          key={item.command}
          type="button"
          role="option"
          aria-selected={index === selected}
          className={`note-slash-item${index === selected ? ' is-selected' : ''}`}
          data-testid="note-slash-item"
          data-command={item.command}
          onMouseEnter={() => setSelected(index)}
          onMouseDown={(event) => {
            event.preventDefault()
            void choose(index)
          }}
        >
          {t(item.key)}
        </button>
      ))}
    </div>
  )
}

/** Where the caret is on the canvas, in CSS pixels. */
export function caretPlace(caret: { x: number; y: number; height: number } | null, view: KitCanvasView | null) {
  return caret && view ? { left: (caret.x - view.x) * view.zoom, top: (caret.y - view.y) * view.zoom, height: caret.height * view.zoom } : null
}
