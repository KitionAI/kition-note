/**
 * Presence (Kit UI protocol section 7e) in a note: this person's caret and the other end of their
 * selection are replica text positions (`{ object: "note", field: "body", offset }`, offsets in
 * Unicode characters, the same on every replica), which the engine's `presenceCaret` answers; the
 * selection's other end travels in the viewport (`anchor`), as the doc kit sends it. Each other
 * person's caret and selection are placed in this view by the engine (`presenceLocate`, in
 * document pixels) and drawn over the canvas in their color with their name (UI-P2), never into
 * the note.
 */
import { useEffect, useMemo, useState } from 'react'
import { peerColor, usePresence, type HostConnection, type KitCanvasView, type KitEngine, type KitPeerPresence, type KitPresence, type KitPresenceCaret } from '@kition/kit-editor'

/** `presenceCaret`: where this person is (null fields before they click or type). */
export type SharedCaret = { selection?: unknown; caret?: unknown; anchor?: unknown }

/** One peer placed by `presenceLocate`, in document pixels. */
export type PlacedPeer = { caret: { x: number; y: number; height: number } | null; rects: Array<{ x: number; y: number; width: number; height: number }> }

/** A peer as drawn here. */
export type PeerMark = { principal: string; name: string; color: string } & PlacedPeer

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value)

/** A well-formed text position, or null. */
export function textPosition(value: unknown): KitPresenceCaret | null {
  if (!isRecord(value)) return null
  const { object, field, offset } = value
  return typeof object === 'string' && typeof field === 'string' && typeof offset === 'number' && Number.isInteger(offset) && offset >= 0 ? { object, field, offset } : null
}

/** What this session shares, from the engine's answer (null before it answered). */
export function sharedPresence(shared: SharedCaret | null): KitPresence | null {
  if (!shared) return null
  const caret = textPosition(shared.caret)
  const anchor = textPosition(shared.anchor)
  const selection = Array.isArray(shared.selection) ? shared.selection.filter((id): id is string => typeof id === 'string') : []
  return { selection, ...(caret ? { caret } : {}), ...(anchor ? { viewport: { anchor } } : {}) }
}

/** What the engine places for each peer, in the peers' order. */
export function placesOf(peers: readonly KitPeerPresence[]) {
  return peers.map((peer) => {
    const anchor = textPosition(peer.viewport?.anchor)
    return { caret: peer.caret ?? null, ...(anchor ? { anchor } : {}) }
  })
}

/** The peers with what the engine placed for them (`placed` answers `placesOf(peers)`); a peer placed nowhere is left out. */
export function peerMarks(peers: readonly KitPeerPresence[], placed: readonly unknown[]): PeerMark[] {
  const marks: PeerMark[] = []
  peers.forEach((peer, index) => {
    const where = placed[index]
    if (!isRecord(where)) return
    const caret = isRecord(where.caret) && [where.caret.x, where.caret.y, where.caret.height].every((n) => typeof n === 'number' && Number.isFinite(n)) ? (where.caret as PlacedPeer['caret']) : null
    const rects = (Array.isArray(where.rects) ? where.rects : []).filter((rect): rect is PlacedPeer['rects'][number] => isRecord(rect) && [rect.x, rect.y, rect.width, rect.height].every((n) => typeof n === 'number' && Number.isFinite(n)))
    if (caret || rects.length) marks.push({ principal: peer.principal, name: peer.name, color: peer.color, caret, rects })
  })
  return marks
}

/**
 * Shares this person's presence after every change and every answer of the text tool (`moved`
 * changes with them), and places the others' after every change and every new presence of theirs.
 */
export function useNotePresence(host: HostConnection, doc: KitEngine | null, revision: number, moved: unknown): PeerMark[] {
  const [shared, setShared] = useState<SharedCaret | null>(null)
  useEffect(() => {
    if (!doc) return undefined
    let stale = false
    void doc
      .invoke<string>('presenceCaret')
      .then((answer) => {
        if (!stale) setShared(JSON.parse(answer) as SharedCaret)
      })
      .catch(() => undefined)
    return () => {
      stale = true
    }
  }, [doc, revision, moved])
  const local = useMemo(() => sharedPresence(shared), [shared])
  const { available, peers } = usePresence(host, local)
  const [placed, setPlaced] = useState<{ peers: readonly KitPeerPresence[]; places: unknown[] }>({ peers: [], places: [] })
  useEffect(() => {
    if (!available || !doc || !peers.length) return undefined
    let stale = false
    void doc
      .invoke<string>('presenceLocate', JSON.stringify(placesOf(peers)))
      .then((answer) => {
        const places = JSON.parse(answer) as unknown
        if (!stale) setPlaced({ peers, places: Array.isArray(places) ? places : [] })
      })
      .catch(() => undefined)
    return () => {
      stale = true
    }
  }, [available, doc, peers, revision, moved])
  return useMemo(() => (placed.peers === peers ? peerMarks(peers, placed.places) : []), [peers, placed])
}

/** The other people's selections (a tint) and carets (a bar with their name), over the canvas. */
export function PeerPresence({ marks, view }: { marks: readonly PeerMark[]; view: KitCanvasView | null }) {
  if (!view || !marks.length) return null
  const at = (x: number, y: number) => ({ left: (x - view.x) * view.zoom, top: (y - view.y) * view.zoom })
  return (
    <div className="note-peers" data-testid="note-peers">
      {marks.map((mark) => {
        const color = peerColor(mark.color)
        return (
          <div key={mark.principal} className="note-peer" data-testid="note-peer" data-principal={mark.principal}>
            {mark.rects.map((rect, index) => (
              <div key={index} className="note-peer-selection" data-testid="note-peer-selection" style={{ ...at(rect.x, rect.y), width: rect.width * view.zoom, height: rect.height * view.zoom, background: color }} />
            ))}
            {mark.caret ? (
              <div className="note-peer-caret" data-testid="note-peer-caret" style={{ ...at(mark.caret.x, mark.caret.y), height: mark.caret.height * view.zoom, background: color }}>
                <span className="note-peer-name" style={{ background: color }}>
                  {mark.name}
                </span>
              </div>
            ) : null}
          </div>
        )
      })}
    </div>
  )
}
