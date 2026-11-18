import { describe, expect, it } from 'vitest'
import { FirstView } from './firstView'

describe('FirstView', () => {
  it('runs a place shown on open only after the first view is placed', () => {
    const first = new FirstView()
    const order: string[] = []
    first.whenPlaced(() => order.push('reveal'))
    order.push('placed')
    first.placed()
    expect(order).toEqual(['placed', 'reveal'])
  })

  it('runs at once when the first view is already placed, and each waiting call once', () => {
    const first = new FirstView()
    let runs = 0
    first.whenPlaced(() => (runs += 1))
    first.placed()
    first.placed()
    expect(runs).toBe(1)
    first.whenPlaced(() => (runs += 1))
    expect(runs).toBe(2)
  })
})
