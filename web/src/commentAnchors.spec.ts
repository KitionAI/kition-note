import { describe, expect, it } from 'vitest'
import { parseCommentAnchor } from '@kition/kit-ui-protocol'
import { AnchorTracker, anchorForSelection, characters, mapRange, relocate } from './commentAnchors'

const NOTE = '# Plan\n\nGoals for the quarter.\n\nClosing words.\n'
const at = (text: string, part: string) => characters(text.slice(0, text.indexOf(part))).length
const rangeOf = (text: string, part: string) => ({ start: at(text, part), end: at(text, part) + characters(part).length })
const read = (text: string, range: { start: number; end: number } | null) => (range ? characters(text).slice(range.start, range.end).join('') : null)
/** `text` with `part` replaced by `by` (its first occurrence), and the range moved through it. */
const edit = (text: string, range: { start: number; end: number }, part: string, by: string) => {
  const after = text.replace(part, by)
  return { after, range: mapRange(range, characters(text), characters(after)) }
}

describe('anchorForSelection', () => {
  it('anchors a selection to the note root with its range in Unicode characters and its quote', () => {
    const text = '😀 Goals for the quarter.'
    const anchor = anchorForSelection(text, 2, 7)
    expect(anchor).toEqual({ ids: ['note'], range: { start: 2, end: 7 }, quote: 'Goals' })
    // Both sides accept it (UI-V3).
    expect(parseCommentAnchor(anchor)).toEqual(anchor)
  })

  it('orders a backward selection, and makes no anchor without one', () => {
    expect(anchorForSelection(NOTE, 12, 8)?.range).toEqual({ start: 8, end: 12 })
    expect(anchorForSelection(NOTE, 5, 5)).toBeNull()
    expect(anchorForSelection(NOTE, 0, 10_000)).toBeNull()
  })

  it('keeps the quote within 500 characters and free of characters hosts refuse', () => {
    const long = 'x'.repeat(800)
    expect(anchorForSelection(long, 0, 800)?.quote).toHaveLength(500)
    const tricky = 'a\u202eb\u0007c\nd\te'
    const anchor = anchorForSelection(tricky, 0, characters(tricky).length)
    expect(anchor?.quote).toBe('a b c\nd\te')
    expect(parseCommentAnchor(anchor)).not.toBeNull()
  })
})

describe('mapRange', () => {
  const goals = rangeOf(NOTE, 'Goals for the quarter')

  it('moves a range with text typed before it, and keeps it where text is typed after it', () => {
    const before = edit(NOTE, goals, '# Plan', '# The plan for 2027')
    expect(read(before.after, before.range)).toBe('Goals for the quarter')
    const after = edit(NOTE, goals, 'Closing', 'Final closing')
    expect(after.range).toEqual(goals)
  })

  it('leaves text typed right at either edge outside, and takes in text typed inside', () => {
    const atStart = edit(NOTE, goals, 'Goals', 'New Goals')
    expect(read(atStart.after, atStart.range)).toBe('Goals for the quarter')
    const atEnd = edit(NOTE, goals, 'quarter.', 'quarter!!.')
    expect(read(atEnd.after, atEnd.range)).toBe('Goals for the quarter')
    const inside = edit(NOTE, goals, 'for the', 'for the third')
    expect(read(inside.after, inside.range)).toBe('Goals for the third quarter')
  })

  it('shrinks a range whose edge was deleted, and drops one whose text is all gone', () => {
    const partly = edit(NOTE, goals, '\n\nGoals for', '\n\n')
    expect(read(partly.after, partly.range)).toBe(' the quarter')
    expect(edit(NOTE, goals, 'Goals for the quarter.', '').range).toBeNull()
  })

  it('takes in text that replaces an edge', () => {
    const replaced = edit(NOTE, goals, 'Plan\n\nGoals', 'Plan\n\nAims')
    expect(read(replaced.after, replaced.range)).toBe('Aims for the quarter')
  })

  it('counts Unicode characters, not UTF-16 units', () => {
    const emoji = edit(NOTE, goals, '# Plan', '# 😀😀 Plan')
    expect(read(emoji.after, emoji.range)).toBe('Goals for the quarter')
    expect(emoji.range).toEqual({ start: goals.start + 3, end: goals.end + 3 })
  })

  it('follows a range through a series of edits', () => {
    let text = NOTE
    let range: { start: number; end: number } | null = goals
    for (const [part, by] of [
      ['# Plan', '# Plan A'],
      ['quarter', 'whole quarter'],
      ['Closing', 'Some closing'],
      ['\n\nGoals', '\n\nA line.\n\nGoals'],
    ] as const) {
      const next = edit(text, range!, part, by)
      text = next.after
      range = next.range
    }
    expect(read(text, range)).toBe('Goals for the whole quarter')
  })
})

describe('relocate', () => {
  const anchor = anchorForSelection(NOTE, at(NOTE, 'Closing'), at(NOTE, 'Closing') + 7)!

  it('keeps a range that still holds its quote', () => {
    expect(relocate(anchor, NOTE)).toEqual(anchor.range)
  })

  it('finds a moved quote again, nearest to where it was', () => {
    const moved = '# Plan\n\nNew intro.\n\nGoals for the quarter.\n\nClosing words.\n'
    expect(read(moved, relocate(anchor, moved))).toBe('Closing')
    const twice = 'Closing.\n' + moved
    expect(relocate(anchor, twice)?.start).toBe(at(twice, 'Closing words'))
  })

  it('finds a long anchor by its cut quote and keeps its length', () => {
    const long = `intro ${'y'.repeat(600)} outro`
    const made = anchorForSelection(long, 6, 606)!
    const moved = `a longer intro ${'y'.repeat(600)} outro`
    expect(relocate(made, moved)).toEqual({ start: 15, end: 615 })
  })

  it('answers null when the text is gone, for another object, or without a range', () => {
    expect(relocate(anchor, NOTE.replace('Closing', 'Ending'))).toBeNull()
    expect(relocate({ ...anchor, ids: ['table-1'] }, NOTE)).toBeNull()
    expect(relocate({ ids: ['note'] }, NOTE)).toBeNull()
  })
})

describe('AnchorTracker', () => {
  const thread = (id: string, text: string, part: string, resolved = false) => ({ id, anchor: anchorForSelection(text, at(text, part), at(text, part) + characters(part).length)!, resolved, count: 1 })

  it('marks the open threads the host shows, on their text as it changes', () => {
    const tracker = new AnchorTracker()
    tracker.show([thread('t1', NOTE, 'Goals'), thread('t2', NOTE, 'Closing', true)])
    // Threads shown before the text was read are found once it is.
    expect(tracker.marked()).toEqual([])
    tracker.text(NOTE)
    expect(tracker.marked().map(({ thread: { id }, range }) => [id, read(NOTE, range)])).toEqual([['t1', 'Goals']])
    const typed = NOTE.replace('Goals', 'Our Goals')
    tracker.text(typed)
    expect(read(typed, tracker.marked()[0].range)).toBe('Goals')
    // The host's list again (another thread added): the moved range is kept, not the stored one.
    tracker.show([thread('t1', NOTE, 'Goals'), thread('t3', NOTE, 'quarter')])
    expect(tracker.marked().map(({ thread: { id }, range }) => [id, read(typed, range)])).toEqual([
      ['t1', 'Goals'],
      ['t3', 'quarter'],
    ])
    // A thread the host no longer shows is no longer marked.
    tracker.show([thread('t3', NOTE, 'quarter')])
    expect(tracker.marked().map(({ thread: { id } }) => id)).toEqual(['t3'])
  })

  it('places a thread this person just made before the host shows it, and answers locate', () => {
    const tracker = new AnchorTracker()
    tracker.text(NOTE)
    const made = anchorForSelection(NOTE, at(NOTE, 'Closing'), at(NOTE, 'Closing') + 7)!
    tracker.remember('new', made.range!)
    const typed = NOTE.replace('# Plan', '# Plan, revised')
    tracker.text(typed)
    tracker.show([{ id: 'new', anchor: made, resolved: false, count: 1 }])
    expect(read(typed, tracker.marked()[0].range)).toBe('Closing')
    expect(tracker.locate([made, { ...made, quote: 'Not in the note' }])).toEqual([true, false])
    // A shown thread's anchor is found where its range moved, even once its quoted text was edited.
    const edited = typed.replace('Closing words', 'Final words')
    tracker.text(edited)
    expect(tracker.locate([made])).toEqual([true])
    expect(relocate(made, edited)).toBeNull()
  })

  it('drops the marker of a thread whose text was deleted', () => {
    const tracker = new AnchorTracker()
    tracker.text(NOTE)
    tracker.show([thread('t1', NOTE, 'Closing words.')])
    tracker.text(NOTE.replace('Closing words.\n', ''))
    expect(tracker.marked()).toEqual([])
  })
})
