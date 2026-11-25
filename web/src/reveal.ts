/**
 * Shows a place of the note the host names (`document.view` when it opens the note, the
 * notification `document/reveal` while it is open; Kit UI protocol section 7i, OP3): a search hit,
 * a diagnostic, a link with a `#heading` or `#^block` part. A note understands three members of a
 * view:
 *
 * - `path`: `body:<line>` or `body:<line>:<column>` (both from 1, the column in Unicode characters),
 *   the shape of the note's diagnostic targets (`note_check`, design 4.1);
 * - `heading`: a heading path from the top, as the engine's `section.read` takes it, or else a
 *   heading's text or anchor (a link's `#heading` part names one heading, not its path);
 * - `blockId`: an Obsidian block id, without the caret.
 *
 * The line it names is selected (at the column, the caret goes there) through the engine's
 * `select.set`, and the editor scrolls the selection into view. A view the note does not hold is
 * ignored.
 */
import type { KitView } from '@kition/sdk/editor'
import type { NoteRun } from './useNote'

export type Place = { line: number; column?: number }

const BODY = /^body:(\d+)(?::(\d+))?$/

/** The line (and column) `view` names, asking the engine for headings and block ids; `null` when the note holds none. */
async function placeOf(view: KitView, run: NoteRun): Promise<Place | null> {
  const body = view.path ? BODY.exec(view.path) : null
  if (body) return { line: Number(body[1]), ...(body[2] ? { column: Number(body[2]) } : {}) }
  const read = async (command: string, params: Record<string, unknown>) => {
    try {
      const answer = await run<{ line?: unknown }>(command, params)
      return typeof answer.line === 'number' ? { line: answer.line } : null
    } catch {
      return null
    }
  }
  if (view.heading) return (await read('section.read', { heading: view.heading })) ?? headingNamed(view.heading.at(-1) ?? '', run)
  if (view.blockId) return read('block.read', { blockId: view.blockId })
  return null
}

/** A heading's anchor as Markdown renderers write it (Kition's `findMarkdownHeadingOffset`). */
function headingSlug(text: string) {
  return text.toLowerCase().replace(/[^\p{L}\p{M}\p{N}_\s-]/gu, '').replace(/\s/g, '-')
}

/**
 * The line of the heading `section` names, not as a path from the top: a link's `#heading` part
 * (`[[Plan#Risks]]`, `[the risks](#risks)`), matched by its anchor (repeated anchors numbered
 * `-1`, `-2`, as renderers do) or its text, ignoring case.
 */
export async function headingNamed(section: string, run: NoteRun): Promise<Place | null> {
  const wanted = section.replace(/^#/, '').trim().toLowerCase()
  if (!wanted) return null
  let headings: Array<{ path: string[]; line: number }> = []
  try {
    headings = (await run<{ headings: Array<{ path: string[]; line: number }> }>('outline.read', { limit: 1000 })).headings
  } catch {
    return null
  }
  const used = new Set<string>()
  const named = headings.map(({ path, line }) => {
    const text = path.at(-1) ?? ''
    const base = headingSlug(text)
    let slug = base
    for (let suffix = 1; used.has(slug); suffix += 1) slug = `${base}-${suffix}`
    used.add(slug)
    return { text: text.toLowerCase(), slug, line }
  })
  const found = named.find((heading) => heading.slug === wanted) ?? named.find((heading) => heading.text === wanted)
  return found ? { line: found.line } : null
}

/** Selects the place `view` names; answers whether the note holds it. */
export async function revealView(view: KitView, run: NoteRun): Promise<boolean> {
  return showPlace(await placeOf(view, run), run)
}

/** Selects `place` (a line, and a column in it); answers whether the note holds it. */
export async function showPlace(place: Place | null, run: NoteRun): Promise<boolean> {
  if (!place || !Number.isInteger(place.line) || place.line < 1) return false
  try {
    await run('select.set', { line: place.line, ...(place.column !== undefined ? { column: place.column } : {}) })
    return true
  } catch {
    return false
  }
}
