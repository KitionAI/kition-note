import { describe, expect, it } from 'vitest'
import { peerMarks, placesOf, sharedPresence } from './Presence'

const at = (offset: number) => ({ object: 'note', field: 'body', offset })

describe('sharedPresence', () => {
  it('shares the caret and the selection end as replica positions, and nothing before the engine answered', () => {
    expect(sharedPresence(null)).toBeNull()
    expect(sharedPresence({ selection: ['note'], caret: at(9), anchor: at(3) })).toEqual({ selection: ['note'], caret: at(9), viewport: { anchor: at(3) } })
    expect(sharedPresence({ selection: [], caret: null, anchor: null })).toEqual({ selection: [] })
    expect(sharedPresence({ selection: [1, 'note'], caret: { object: 'note', field: 'body', offset: -1 } })).toEqual({ selection: ['note'] })
  })
})

describe('peerMarks', () => {
  const peers = [
    { principal: 'p1', name: 'Ada', color: '#e11d48', selection: [], caret: at(4), viewport: { anchor: at(1) } },
    { principal: 'p2', name: 'Bo', color: '#2563eb', selection: [], caret: at(400) },
    { principal: 'p3', name: 'Cy', color: '#16a34a', selection: [] },
  ]
  it('asks the engine for each peer in order, with the selection end when there is one', () => {
    expect(placesOf(peers)).toEqual([{ caret: at(4), anchor: at(1) }, { caret: at(400) }, { caret: null }])
  })
  it('keeps the peers the engine placed and drops malformed answers', () => {
    const marks = peerMarks(peers, [
      { caret: { x: 10, y: 20, height: 18 }, rects: [{ x: 1, y: 20, width: 9, height: 18 }, { x: 'bad' }] },
      { caret: null, rects: [] },
      'junk',
    ])
    expect(marks).toEqual([{ principal: 'p1', name: 'Ada', color: '#e11d48', caret: { x: 10, y: 20, height: 18 }, rects: [{ x: 1, y: 20, width: 9, height: 18 }] }])
  })
})
