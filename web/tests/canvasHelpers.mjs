// Helpers for the flow tests on the engine's canvas: the engine holds and draws the note, so a
// test reads its text through the host's AI bridge and moves the caret through the engine's
// `selection.set`, then types with the keyboard into kit-editor's text field.

/** The note's frame in `page`. */
export const kitFrame = (page) => page.frames().find((frame) => frame !== page.mainFrame())

/** Whether the canvas is up in the note's frame. */
export const canvasReady = async (page) => (await kitFrame(page).locator('[data-testid="note-canvas"] canvas').count()) > 0

/** Clicks into the canvas, so the engine's text tool has the keyboard. */
export async function focusCanvas(page) {
  const box = await kitFrame(page).locator('[data-testid="note-canvas"] canvas').first().boundingBox()
  // Low in the canvas: a note's top can hold its properties block, whose fields take a click.
  await page.mouse.click(box.x + box.width / 2, box.y + box.height - 20)
}

/** Unicode characters before byte offset `at` of a JavaScript string index (both count code points here). */
const scalars = (text, index) => [...text.slice(0, index)].length

/**
 * Puts the caret right after `after` (its first occurrence), or selects `select` when given:
 * the caret goes to the selection's end and Shift+ArrowLeft extends it, as a person would.
 * `run(command, params)` runs an engine command; `read()` answers the engine's text.
 */
export async function caretAfter(page, run, read, after) {
  const text = await read()
  const at = text.indexOf(after)
  if (at < 0) throw new Error(`no ${JSON.stringify(after)} in the note`)
  await run('selection.set', { anchor: scalars(text, at + after.length) })
  // The editor asks the tool for the caret again after a key; an arrow pair leaves it in place.
  await page.keyboard.press('ArrowLeft')
  await page.keyboard.press('ArrowRight')
}

export async function selectText(page, run, read, wanted) {
  await caretAfter(page, run, read, wanted)
  for (let i = 0; i < [...wanted].length; i += 1) await page.keyboard.press('Shift+ArrowLeft')
}

/** Clicks the canvas at the place of Unicode character `index` (found through the engine's caret). */
export async function clickAtChar(page, run, index) {
  await run('selection.set', { anchor: index })
  const { caret: at } = await run('selection.get')
  const frame = kitFrame(page)
  const [x, y, , , zoom] = (await frame.getByTestId('note-editor').getAttribute('data-view')).split(',').map(Number)
  const box = await frame.locator('[data-testid="note-canvas"] canvas').first().boundingBox()
  await page.mouse.click(box.x + (at.x + 2 - x) * zoom, box.y + (at.y + at.height / 2 - y) * zoom)
}
