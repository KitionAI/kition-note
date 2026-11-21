/**
 * Comment anchors for a host that keeps the comment threads itself (Kit UI protocol 2.6 section 7k,
 * capability `comment-anchors`; managed co-editing): a new thread on the person's selection is
 * the host's (`comment.create`), and the note only marks the threads the host shows
 * (`comments/show`) and tells it which one the person clicked (`comments/activated`).
 *
 * An anchor names the note's root object (`ids: ["note"]`, the text field `body`), the selected
 * range in Unicode characters of that text (`KitPointer 1.0` PT7 counts text positions so), and the
 * quoted text. The host stores an anchor as it was made; the note keeps it on its text as the text
 * changes: while it is open each thread's range moves with every edit (`mapRange`), and an anchor
 * that arrives (a reopen, another person's thread) is found again by its quote, nearest to where it
 * was (`relocate`). An anchor whose text is gone has no marker, and `comments/locate` answers
 * `false` for it.
 */
import { ANCHOR_TEXT_LIMIT, type KitCommentAnchor, type KitShownThread } from '@kition/sdk/protocol'

/** The note's root object, whose `body` is the whole Markdown file (`model/schema.json`). */
export const NOTE_ROOT_ID = 'note'
export const NOTE_BODY = 'body'

export type TextRange = { start: number; end: number }

/** A text as Unicode characters, the unit every anchor and engine position counts. */
export const characters = (text: string) => Array.from(text)

// eslint-disable-next-line no-control-regex
const NOT_IN_QUOTES = /[\u0000-\u0008\u000b-\u001f\u007f-\u009f\u202a-\u202e\u2066-\u2069\u200e\u200f\u061c]/g

/**
 * A quote as an anchor carries it (UI-V3, UI-V6): at most 500 characters, control and
 * bidirectional formatting characters (which both sides refuse) as spaces, lines and tabs kept.
 */
export function quoteOf(chars: readonly string[], range: TextRange) {
  return chars.slice(range.start, Math.min(range.end, range.start + ANCHOR_TEXT_LIMIT)).join('').replace(NOT_IN_QUOTES, ' ')
}

/** The anchor of the selection `from`..`to` (Unicode characters) in `text`, or null for an empty or out-of-range one. */
export function anchorForSelection(text: string, from: number, to: number): KitCommentAnchor | null {
  const chars = characters(text)
  const start = Math.min(from, to)
  const end = Math.max(from, to)
  if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start < 0 || end > chars.length || start === end) return null
  return { ids: [NOTE_ROOT_ID], range: { start, end }, quote: quoteOf(chars, { start, end }) }
}

/**
 * `range` moved through the change from `before` to `after` (the text around the change is what the
 * two have in common at their start and end). Text typed at an edge stays outside the range; text
 * typed inside it, or over its edge, joins it. Answers null once all its text is deleted.
 */
export function mapRange(range: TextRange, before: readonly string[], after: readonly string[]): TextRange | null {
  if (before === after) return range
  const shortest = Math.min(before.length, after.length)
  let prefix = 0
  while (prefix < shortest && before[prefix] === after[prefix]) prefix += 1
  let suffix = 0
  while (suffix < shortest - prefix && before[before.length - 1 - suffix] === after[after.length - 1 - suffix]) suffix += 1
  const oldEnd = before.length - suffix
  const newEnd = after.length - suffix
  const delta = after.length - before.length
  if (prefix === oldEnd && prefix === newEnd) return range
  // An edge inside the replaced text takes in the replacement.
  const start = range.start < prefix ? range.start : range.start >= oldEnd ? range.start + delta : prefix
  const end = range.end <= prefix ? range.end : range.end >= oldEnd ? range.end + delta : newEnd
  if (end <= start) return null
  return { start, end }
}

/** Every place `needle` starts in `chars`. */
function occurrences(chars: readonly string[], needle: readonly string[]) {
  const found: number[] = []
  if (!needle.length || needle.length > chars.length) return found
  outer: for (let at = 0; at + needle.length <= chars.length; at += 1) {
    for (let index = 0; index < needle.length; index += 1) if (chars[at + index] !== needle[index]) continue outer
    found.push(at)
  }
  return found
}

/** Whether `chars` hold `quote` (as `quoteOf` writes quotes) at `at`. */
function quotedAt(chars: readonly string[], quote: readonly string[], at: number) {
  if (at < 0 || at + quote.length > chars.length) return false
  return quoteOf(chars, { start: at, end: at + quote.length }) === quote.join('')
}

/**
 * Where `anchor` is in `text` now: its range when that still holds its quote, else the quote found
 * nearest to where the range was (a quote cut at 500 characters keeps the range's length), else
 * null. An anchor of another object, or one without a range, is not in the text.
 */
export function relocate(anchor: KitCommentAnchor, text: string | readonly string[]): TextRange | null {
  if (anchor.ids[0] !== NOTE_ROOT_ID || !anchor.range) return null
  const chars = typeof text === 'string' ? characters(text) : text
  const { start, end } = anchor.range
  const length = end - start
  if (length <= 0) return null
  if (anchor.quote === undefined) return end <= chars.length ? { start, end } : null
  const quote = characters(anchor.quote)
  if (!quote.length) return null
  if (quotedAt(chars, quote, start)) return { start, end: Math.min(chars.length, start + length) }
  // The text moved: the quote's place nearest to where it was.
  const cleaned = chars.map((char) => char.replace(NOT_IN_QUOTES, ' '))
  let best: number | null = null
  for (const at of occurrences(cleaned, quote)) if (best === null || Math.abs(at - start) < Math.abs(best - start)) best = at
  if (best === null) return null
  return { start: best, end: Math.min(chars.length, best + Math.max(length, quote.length)) }
}

/**
 * The ranges of the host's threads in the note as it changes: `show` takes the host's whole list
 * (a thread it already tracks keeps the range its edits moved it to; a new one is found by its
 * anchor), `text` takes the note after a change and moves every range with it, and `remember`
 * places a thread this person just made at their selection.
 */
export class AnchorTracker {
  private chars: string[] | null = null
  private ranges = new Map<string, TextRange | null>()
  private threads: KitShownThread[] = []

  /** The host's threads, the whole list each time. */
  show(threads: readonly KitShownThread[]) {
    this.threads = [...threads]
    const next = new Map<string, TextRange | null>()
    for (const thread of threads) next.set(thread.id, this.ranges.has(thread.id) ? (this.ranges.get(thread.id) ?? null) : this.chars ? relocate(thread.anchor, this.chars) : null)
    this.ranges = next
  }

  /** The note's text now: every range moves through the change since the last one. */
  text(text: string) {
    const chars = characters(text)
    const before = this.chars
    this.chars = chars
    for (const [id, range] of this.ranges) {
      if (before && range) this.ranges.set(id, mapRange(range, before, chars))
      // Threads shown before the text was read are found now.
      else if (!before) this.ranges.set(id, this.find(id))
    }
  }

  private find(id: string) {
    const thread = this.threads.find((candidate) => candidate.id === id)
    return thread && this.chars ? relocate(thread.anchor, this.chars) : null
  }

  /** A thread this person just made on `range` (the host shows it next). */
  remember(id: string, range: TextRange) {
    this.ranges.set(id, range)
  }

  /**
   * Whether each anchor is in the note now (`comments/locate`): a shown thread's anchor is where its
   * range moved to (its text may have been edited since), any other is found by its quote.
   */
  locate(anchors: readonly KitCommentAnchor[]) {
    return anchors.map((anchor) => {
      const key = JSON.stringify(anchor)
      const thread = this.threads.find((candidate) => JSON.stringify(candidate.anchor) === key)
      if (thread && this.ranges.has(thread.id)) return this.ranges.get(thread.id) != null
      return this.chars ? relocate(anchor, this.chars) !== null : false
    })
  }

  /** The open threads with their range now; resolved threads and those whose text is gone are left out. */
  marked(): Array<{ thread: KitShownThread; range: TextRange }> {
    return this.threads.flatMap((thread) => {
      const range = this.ranges.get(thread.id)
      return !thread.resolved && range ? [{ thread, range }] : []
    })
  }
}
