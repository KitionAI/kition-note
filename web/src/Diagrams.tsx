/**
 * Blocks drawn as Obsidian's live preview draws them, away from the caret: a ```mermaid block as
 * its diagram, laid out by the same Mermaid (11.13.0) with Obsidian's settings, at its natural
 * size from the column's left edge (a block that does not parse shows Mermaid's error as code);
 * an HTML block as its HTML (`<details>` folds), sanitized, and without anything it would load
 * from the network (the kit loads nothing from it, UI-K1). The engine asks for the blocks it has
 * no size for (`diagram.view`), keeps each one's room once it is measured (`diagram.set`, again
 * when an HTML block changes size), and reports where the drawn ones are; they are drawn over the
 * canvas there. A click on one leaves the caret where it is, as in Obsidian; its edit button puts
 * the caret at the start of its source.
 */
import DOMPurify from 'dompurify'
import { useEffect, useRef, useState } from 'react'
import { createKitStrings, type KitCanvasView } from '@kition/sdk/editor'
import { CodeXmlIcon } from '@kition/tessera/icons/react'
import type { NoteRun } from './useNote'
import { useWheelToCanvas } from './wheel'
import type { ColorScheme } from './colorScheme'

type Strings = ReturnType<typeof createKitStrings>
type Mermaid = (typeof import('mermaid'))['default']

type Kind = 'mermaid' | 'html'
export type DiagramPlace = { kind: Kind; source: string; top: number; left: number; width: number; height: number; code: number }
export type DiagramsView = { wanted: { kind: Kind; source: string }[]; column: number; diagrams: DiagramPlace[] }
/** A block drawn: Mermaid's SVG or sanitized HTML, or the error Mermaid gave. */
type Drawn = { svg: string; error?: undefined } | { svg?: undefined; error: string }

/**
 * The diagrams' faces, as Obsidian passes them: the CSS variable (its value, Obsidian's text faces,
 * is set in app.css). Mermaid measures text with the name as given, so the same name lays a
 * diagram out the same.
 */
const FONT = 'var(--note-ui-font)'

let loading: Promise<Mermaid> | null = null
/** The scheme Mermaid was last set up for. */
let themed: ColorScheme | null = null

/** Mermaid's settings, as Obsidian's, with its theme for the host's scheme (Obsidian's dark theme draws with Mermaid's `dark`). */
function settings(scheme: ColorScheme) {
  return {
    startOnLoad: false,
    securityLevel: 'strict' as const,
    theme: scheme === 'dark' ? ('dark' as const) : ('default' as const),
    themeVariables: { fontFamily: FONT },
    flowchart: { useMaxWidth: false },
    sequence: { useMaxWidth: false },
    // Obsidian's gantt axis: dates, a tick on Mondays.
    gantt: { useMaxWidth: true, axisFormatter: [['%Y-%m-%d', (date: Date) => date.getDay() === 1]] } as Record<string, unknown>,
    journey: { useMaxWidth: true },
    class: { useMaxWidth: true },
    gitGraph: { useMaxWidth: false },
    state: { useMaxWidth: true },
    er: { useMaxWidth: false },
    pie: { useMaxWidth: true },
  }
}

/** Mermaid, loaded the first time a note has a diagram, set up for `scheme`. */
async function mermaid(scheme: ColorScheme): Promise<Mermaid> {
  loading ??= import('mermaid').then(({ default: loaded }) => loaded)
  const loaded = await loading
  if (themed !== scheme) {
    loaded.initialize(settings(scheme))
    themed = scheme
  }
  return loaded
}

let drawing: Promise<unknown> = Promise.resolve()
let counter = 0

const REMOTE = /^\s*(?!data:)[a-z][a-z\d+.-]*:|^\s*\/\//i
// An instance of its own: its hooks (no network, links that do not navigate) are for HTML blocks
// only, not for the HTML a paste converts.
let purifier: ReturnType<typeof DOMPurify> | null = null

/**
 * An HTML block's markup made safe: no scripts, handlers, frames, forms, or media, and nothing
 * that loads from the network (a remote picture, a style's `url()`); links do not navigate.
 */
function sanitize(html: string) {
  if (!purifier) {
    purifier = DOMPurify(window)
    purifier.addHook('afterSanitizeAttributes', (node) => {
      for (const name of ['src', 'srcset', 'poster', 'background']) {
        const value = node.getAttribute(name)
        if (value !== null && (name === 'srcset' || REMOTE.test(value) || !value.startsWith('data:'))) node.removeAttribute(name)
      }
      const style = node.getAttribute('style')
      if (style && /url\s*\(|@import|expression\s*\(/i.test(style)) node.removeAttribute('style')
      if (node.tagName === 'A') {
        node.setAttribute('data-href', node.getAttribute('href') ?? '')
        node.removeAttribute('href')
      }
    })
  }
  return purifier.sanitize(html, {
    FORBID_TAGS: ['style', 'iframe', 'frame', 'object', 'embed', 'form', 'input', 'button', 'textarea', 'select', 'link', 'meta', 'base', 'video', 'audio', 'source', 'track', 'picture', 'svg', 'math'],
    FORBID_ATTR: ['id', 'name', 'formaction', 'action'],
  })
}

/** `source` drawn: HTML sanitized, or Mermaid's diagram, one at a time (Mermaid's rendering is not reentrant). */
function draw(source: string, kind: Kind, column: number, scheme: ColorScheme): Promise<Drawn> {
  if (kind === 'html') return Promise.resolve({ svg: sanitize(source) })
  const next = drawing.then(async (): Promise<Drawn> => {
    const id = `note-mermaid-${++counter}`
    // Drawn in a box as wide as the column, as Obsidian's are: a diagram that fills its width
    // (a gantt chart) is laid out for it, not for the page.
    const box = document.createElement('div')
    box.style.cssText = `position:absolute;left:-100000px;top:0;visibility:hidden;width:${column}px`
    document.body.append(box)
    try {
      const { svg } = await (await mermaid(scheme)).render(id, source, box)
      return { svg }
    } catch (error) {
      return { error: `Error parsing Mermaid diagram!\n\n${error instanceof Error ? error.message : String(error)}` }
    } finally {
      // A failed render leaves its scratch element in the page.
      document.getElementById(`d${id}`)?.remove()
      document.getElementById(id)?.remove()
      box.remove()
    }
  })
  drawing = next.catch(() => undefined)
  return next
}

/** The block's markup, as it is drawn over the canvas and measured. */
function markup(drawn: Drawn, kind: Kind) {
  if (kind === 'html' && drawn.svg !== undefined) return `<div class="note-diagram-html">${drawn.svg}</div>`
  if (drawn.svg !== undefined) return `<div class="note-diagram-svg">${drawn.svg}</div>`
  // Obsidian's error is code in a pre (whose 24 px line sets the rows apart).
  const pre = document.createElement('pre')
  pre.className = 'note-diagram-error'
  const code = document.createElement('code')
  code.textContent = drawn.error
  pre.append(code)
  return pre.outerHTML
}

/** The height `html` takes in a column `width` wide. */
function measure(html: string, width: number) {
  const probe = document.createElement('div')
  probe.className = 'note-diagram'
  probe.style.cssText = `position:absolute;left:-100000px;top:0;visibility:hidden;width:${width}px`
  probe.innerHTML = html
  document.body.append(probe)
  const height = probe.getBoundingClientRect().height
  probe.remove()
  return height
}

type Known = { drawn: Drawn; html: string; column: number; scheme: ColorScheme }

/**
 * A block's markup over the canvas. An HTML block that changes size (a `<details>` opened) tells
 * `onResize`, and the engine keeps its new room; its links do not navigate the editor's page.
 */
function Drawing({ html, height, onResize }: { html: string; height: number; onResize?: (height: number) => void }) {
  const box = useRef<HTMLDivElement>(null)
  const reported = useRef(height)
  reported.current = height
  useEffect(() => {
    const element = box.current
    if (!element || !onResize || typeof ResizeObserver === 'undefined') return undefined
    const observer = new ResizeObserver(() => {
      // Layout pixels (the zoom scales the block's box, not its layout).
      const now = Math.ceil(element.offsetHeight)
      if (Math.abs(now - reported.current) >= 1) {
        reported.current = now
        onResize(now)
      }
    })
    observer.observe(element)
    return () => observer.disconnect()
  }, [onResize])
  return (
    <div
      ref={box}
      className="note-diagram"
      onClick={(event) => {
        if (event.target instanceof Element && event.target.closest('a')) event.preventDefault()
      }}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  )
}

export function Diagrams({
  view: shown,
  diagrams,
  sizes,
  run,
  readOnly,
  scheme,
  t,
}: {
  view: KitCanvasView | null
  diagrams: DiagramsView | null
  sizes: (source: string, width: number, height: number) => Promise<unknown>
  run: NoteRun
  readOnly: boolean
  /** The host's color scheme: a diagram is drawn again in the other's colors. */
  scheme: ColorScheme
  t: Strings
}) {
  const known = useRef(new Map<string, Known>())
  const pending = useRef(new Map<string, Promise<Drawn>>())
  const [, setDrawn] = useState(0)
  // Draws the diagrams asked for, and measures again those drawn for another column width.
  useEffect(() => {
    if (!diagrams) return undefined
    let current = true
    const { column } = diagrams
    const stale = diagrams.diagrams.filter((diagram) => {
      const entry = known.current.get(diagram.source)
      return entry?.column !== column || (diagram.kind === 'mermaid' && entry?.scheme !== scheme)
    })
    const asked = new Map<string, Kind>()
    for (const { kind, source } of [...diagrams.wanted, ...stale]) asked.set(source, kind)
    void (async () => {
      for (const [source, kind] of asked) {
        // A diagram drawn for another column width or scheme is drawn again (Mermaid lays it out
        // for its box: a gantt chart fills it; its colors are its theme's); an HTML block is only
        // measured again.
        const before = known.current.get(source)
        if (before && kind === 'mermaid' && (before.column !== column || before.scheme !== scheme)) known.current.delete(source)
        let entry = known.current.get(source)
        if (!entry) {
          const key = `${scheme}:${source}`
          let drawing = pending.current.get(key)
          if (!drawing) {
            drawing = draw(source, kind, column, scheme)
            pending.current.set(key, drawing)
          }
          const drawn = await drawing
          pending.current.delete(key)
          entry = known.current.get(source) ?? { drawn, html: markup(drawn, kind), column: 0, scheme }
        }
        if (!current) return
        entry.column = column
        known.current.set(source, entry)
        // Diagrams no longer in the note are forgotten.
        if (known.current.size > 256) known.current.delete(known.current.keys().next().value as string)
        await sizes(source, column, measure(entry.html, column)).catch(() => undefined)
      }
      if (current && asked.size) setDrawn((count) => count + 1)
    })()
    return () => {
      current = false
    }
  }, [diagrams, sizes, scheme])
  const layer = useWheelToCanvas()
  return (
    <div ref={layer} className="note-diagrams">
      {shown && diagrams ? diagrams.diagrams.map((diagram) => {
        const entry = known.current.get(diagram.source)
        if (!entry) return null
        const top = (diagram.top - shown.y) * shown.zoom
        if (top + diagram.height * shown.zoom < 0 || top > shown.height) return null
        return (
          <div
            key={`${diagram.code}-${diagram.source.length}`}
            className="note-diagram-block"
            data-testid="note-diagram"
            data-kind={diagram.kind}
            data-error={entry.drawn.error ? 'true' : undefined}
            style={{ transform: `translate(${(diagram.left - shown.x) * shown.zoom}px, ${top}px) scale(${shown.zoom})`, width: diagram.width, height: diagram.height }}
            onMouseDown={(event) => {
              // The caret stays where it is, and the editor keeps the keyboard.
              event.preventDefault()
              event.stopPropagation()
            }}
          >
            <Drawing
              html={entry.html}
              height={diagram.height}
              onResize={diagram.kind === 'html' ? (height) => void sizes(diagram.source, diagrams.column, height).catch(() => undefined) : undefined}
            />
            {readOnly ? null : (
              <button
                type="button"
                className="note-diagram-edit"
                data-testid="note-diagram-edit"
                aria-label={t('diagram.edit')}
                title={t('diagram.edit')}
                onMouseDown={(event) => {
                  event.preventDefault()
                  event.stopPropagation()
                }}
                onClick={() => {
                  void run('select.set', { anchor: diagram.code })
                  // The keyboard goes to the note's source (an embedded editor takes it only on a click like this).
                  ;(document.querySelector('[data-testid="kit-text-sink"]') as HTMLElement | null)?.focus({ preventScroll: true })
                }}
              >
                <CodeXmlIcon size={16} aria-hidden="true" />
              </button>
            )}
          </div>
        )
      }) : null}
    </div>
  )
}
