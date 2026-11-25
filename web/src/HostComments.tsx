/**
 * The host's comment threads in the note (Kit UI protocol 2.6 section 7k, capability
 * `comment-anchors`): a host that keeps threads itself shows them (`comments/show`) and the note
 * marks each open one, its anchored text tinted and a marker beside the column, kept on the text
 * as it changes (`AnchorTracker`); a click on a marker tells the host (`comments/activated`), which
 * opens the thread. A new thread on the selection is the host's (`comment.create`), offered only
 * when the host serves it, which it does only to a person with comment rights (UI-V4).
 */
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { MessageSquareIcon as MessageSquare } from '@kition/tessera/icons/react'
import { toast, type HostConnection, type KitCanvasView, type KitEngine } from '@kition/sdk/editor'
import type { KitShownThread } from '@kition/sdk/protocol'
import { AnchorTracker, NOTE_BODY, NOTE_ROOT_ID, anchorForSelection, type TextRange } from './commentAnchors'
import { message, type NoteRun } from './useNote'
import type { Strings } from './App'

type Rect = { x: number; y: number; width: number; height: number }

/** One thread as drawn: its tinted rectangles (document pixels). */
export type ThreadMark = { id: string; count: number; rects: Rect[] }

/** The host keeps the threads and the note anchors them. */
export const hostKeepsComments = (host: HostConnection) => host.commentAnchors

/** The person may start a thread here: the host keeps threads and serves `comment.create` (only with comment rights). */
export const canCreateComment = (host: HostConnection) => host.commentAnchors && host.hasService('comment.create')

const isRect = (value: unknown): value is Rect => {
  if (typeof value !== 'object' || value === null) return false
  const rect = value as Record<string, unknown>
  return [rect.x, rect.y, rect.width, rect.height].every((n) => typeof n === 'number' && Number.isFinite(n))
}

/**
 * Keeps the host's threads on the note's text and places them after every change and every answer
 * of the text tool (`moved`: a fold moves text too); answers `create` (a new thread on the
 * selection) and `activate` (open a thread in the host). `run` is the note's command run, after the
 * keys still waiting for the engine.
 */
export function useHostComments(host: HostConnection, doc: KitEngine | null, run: NoteRun, revision: number, moved: unknown, t: Strings) {
  const on = hostKeepsComments(host)
  const tracker = useMemo(() => new AnchorTracker(), [doc])
  /** Bumped whenever the tracker's threads or ranges changed. */
  const [tracked, setTracked] = useState(0)
  const bump = useCallback(() => setTracked((value) => value + 1), [])

  useEffect(() => {
    if (!on || !doc) return undefined
    const stop = host.onComments((threads: KitShownThread[]) => {
      tracker.show(threads)
      bump()
    })
    host.locateComments((anchors) => tracker.locate(anchors))
    return stop
  }, [bump, doc, host, on, tracker])

  // The note's text after every change: each thread's range moves with it. Read straight from the
  // engine, so reading is no change.
  const reading = useRef(Promise.resolve())
  useEffect(() => {
    if (!on || !doc) return
    reading.current = reading.current.then(() =>
      doc
        .execute('text.read', {})
        .then((answer) => {
          tracker.text((JSON.parse(answer) as { text: string }).text)
          bump()
        })
        .catch(() => undefined),
    )
  }, [bump, doc, on, revision, tracker])

  const [marks, setMarks] = useState<ThreadMark[]>([])
  useEffect(() => {
    if (!on || !doc) return undefined
    const marked = tracker.marked()
    if (!marked.length) {
      setMarks([])
      return undefined
    }
    let stale = false
    const at = (offset: number) => ({ object: NOTE_ROOT_ID, field: NOTE_BODY, offset })
    void doc
      .invoke<string>('presenceLocate', JSON.stringify(marked.map(({ range }) => ({ caret: at(range.end), anchor: at(range.start) }))))
      .then((answer) => {
        if (stale) return
        const places = JSON.parse(answer) as unknown
        const placed = Array.isArray(places) ? places : []
        setMarks(
          marked.flatMap(({ thread }, index) => {
            const where = placed[index] as { rects?: unknown } | undefined
            const rects = (Array.isArray(where?.rects) ? where.rects : []).filter(isRect)
            return rects.length ? [{ id: thread.id, count: thread.count, rects }] : []
          }),
        )
      })
      .catch(() => undefined)
    return () => {
      stale = true
    }
  }, [doc, moved, on, tracked, tracker])

  /**
   * A new thread in the host on the engine's selection; from the person's gesture only (UI-V1).
   * The selection is read after the keys still waiting for the engine (the ordered `run`, not the
   * engine directly): Comment clicked, or Mod+Alt+M pressed, right after fast Shift+Arrow keys
   * comments on the whole selection they made, not on the one the engine had reached.
   */
  const create = useCallback(async () => {
    if (!doc || !canCreateComment(host)) return
    let selection: { from: number; to: number }
    let text: string
    try {
      selection = await run<{ from: number; to: number }>('select.get', {})
      text = (await run<{ text: string }>('text.read', {})).text
    } catch (error) {
      toast('error', t('commandFailed'), message(error))
      return
    }
    // The ranges move to this text first, so the new one starts from the same text as the rest.
    tracker.text(text)
    const anchor = anchorForSelection(text, selection.from, selection.to)
    if (!anchor) {
      toast('info', t('commentSelectFirst'))
      return
    }
    try {
      const { thread } = await host.service('comment.create', { anchor })
      if (thread) {
        tracker.remember(thread, anchor.range as TextRange)
        bump()
      }
    } catch (error) {
      if ((error as { data?: { reason?: string } })?.data?.reason === 'cancelled') return
      toast('error', t('commentFailed'), message(error))
    }
  }, [bump, doc, host, run, t, tracker])

  const activate = useCallback((id: string) => host.activateComment(id), [host])

  return { on, marks, create, activate }
}

/**
 * The marked threads over the canvas: the anchored text tinted (the canvas keeps its clicks), and a
 * marker per thread in the margin right of the column (`right`, document pixels; beside the text's
 * end without it) that opens the thread in the host.
 */
export function HostCommentMarks({ marks, view, right, onActivate, t }: { marks: readonly ThreadMark[]; view: KitCanvasView | null; right: number | null; onActivate: (id: string) => void; t: Strings }) {
  if (!view || !marks.length) return null
  const at = (x: number, y: number) => ({ left: (x - view.x) * view.zoom, top: (y - view.y) * view.zoom })
  // Markers on one line sit side by side.
  const taken = new Map<number, number>()
  return (
    <div className="note-threads" data-testid="note-threads">
      {marks.map((mark) => {
        const first = mark.rects[0]
        const line = Math.round(first.y)
        const slot = taken.get(line) ?? 0
        taken.set(line, slot + 1)
        const x = (right ?? Math.max(...mark.rects.filter((rect) => Math.round(rect.y) === line).map((rect) => rect.x + rect.width))) + 8 + slot * 26
        return (
          <div key={mark.id} className="note-thread" data-thread={mark.id}>
            {mark.rects.map((rect, index) => (
              <div key={index} className="note-thread-range" data-testid="note-thread-range" style={{ ...at(rect.x, rect.y), width: rect.width * view.zoom, height: rect.height * view.zoom }} />
            ))}
            <button
              type="button"
              className="note-thread-marker"
              data-testid="note-thread-marker"
              data-thread={mark.id}
              title={t('commentOpen')}
              aria-label={t('commentOpen')}
              style={{ ...at(x, first.y), height: Math.max(18, first.height * view.zoom) }}
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => onActivate(mark.id)}
            >
              <MessageSquare size={13} />
              {mark.count > 1 ? <span>{mark.count}</span> : null}
            </button>
          </div>
        )
      })}
    </div>
  )
}
