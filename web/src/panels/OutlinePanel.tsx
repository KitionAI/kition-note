/**
 * The note's outline (Kition's `DocumentOutlinePanel`): the engine's `outline.read`, one row
 * per heading, indented by level; a row puts the caret at its heading's end and shows it.
 */
import { useEffect, useState } from 'react'
import type { NoteRun, Session } from '../useNote'

type Heading = { path: string[]; level: number; line: number; words: number }

export function OutlinePanel({ session, run, onShown, title, empty }: { session: Session; run: NoteRun; onShown: () => void; title: string; empty: string }) {
  const [headings, setHeadings] = useState<Heading[]>([])
  const doc = session.doc
  const revision = session.change.revision
  useEffect(() => {
    if (!doc) return undefined
    let current = true
    void doc
      .execute('outline.read', { limit: 1000 })
      .then((answer) => {
        if (current) setHeadings((JSON.parse(answer) as { headings: Heading[] }).headings)
      })
      .catch(() => {
        if (current) setHeadings([])
      })
    return () => {
      current = false
    }
  }, [doc, revision])

  const reveal = async (heading: Heading) => {
    // A column past the line's end puts the caret at its end.
    await run('select.set', { line: Math.max(1, heading.line), column: Number.MAX_SAFE_INTEGER })
    onShown()
  }

  const top = headings.reduce((least, heading) => Math.min(least, heading.level), 6)
  return (
    <aside className="kui-panel note-kit-panel" data-testid="note-outline">
      <div className="kui-panel-header">{title}</div>
      <div className="kui-panel-body note-kit-outline">
        {headings.length ? (
          headings.map((heading, index) => (
            <button
              key={`${heading.line}:${index}`}
              type="button"
              className="note-kit-outline-row"
              style={{ paddingLeft: 8 + (heading.level - top) * 14 }}
              data-level={heading.level}
              data-testid="note-outline-row"
              title={heading.path.join(' › ')}
              onClick={() => void reveal(heading)}
            >
              {heading.path.at(-1) || '—'}
            </button>
          ))
        ) : (
          <div className="kui-muted note-kit-outline-empty">{empty}</div>
        )}
      </div>
    </aside>
  )
}
