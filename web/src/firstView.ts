/**
 * The note's first view and the place the host opened it at (`document.view`, OP3). Placing the
 * first view scrolls to the top, and the canvas can be placed a frame or more after the note is
 * open; a place shown before that would be scrolled away again, its caret never moving to bring
 * it back. So the opening place waits until the first view is placed.
 */
export class FirstView {
  private done = false
  private waiting: Array<() => void> = []

  /** The first view is placed: runs what waited for it. */
  placed(): void {
    if (this.done) return
    this.done = true
    const waiting = this.waiting
    this.waiting = []
    for (const run of waiting) run()
  }

  /** Runs `run` once the first view is placed (at once if it is). */
  whenPlaced(run: () => void): void {
    if (this.done) run()
    else this.waiting.push(run)
  }
}
