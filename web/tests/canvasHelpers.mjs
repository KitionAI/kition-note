// Helpers for the flow tests on the engine's canvas: the engine holds and draws the note, so a
// test reads its text through the host's AI bridge, moves the person's caret as the host shows a
// place (`document/reveal`, a line and a column) and extends it with the keyboard, then types with
// the keyboard into kit-editor's text field. (An agent's `select.set` through `ai/run` runs in
// an editor state of the agent's own, Kit UI protocol C3b: it never moves the person's selection.)

/** The note's frame in `page`. */
export const kitFrame = (page) => page.frames().find((frame) => frame !== page.mainFrame())

/**
 * The modifier the editor's shortcuts take in `page`, `Meta` or `Control`: kit-editor and the note
 * read Cmd or Ctrl from `navigator.platform`, while Playwright's `ControlOrMeta` follows the
 * operating system running the test, and the two disagree in WebKit on Linux (it reports a Mac).
 * The browser's own field editing (select all in an input) follows the operating system:
 * `ControlOrMeta` there.
 */
export const primaryKey = async (page) => ((await page.evaluate(() => /Mac|iPhone|iPad/i.test(navigator.platform))) ? 'Meta' : 'Control')

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
 * Shows `path` (`body:<line>` or `body:<line>:<column>`) as the host does (`document/reveal`): the
 * note selects it as the person's. The host is the development host or a test's own (`aiHost`,
 * `commentsHost`, or `workspaceHost` with the editor `slot`), each with the connection to the kit.
 */
export const reveal = (page, path, slot = 'a') =>
  page.evaluate(
    ([path, slot]) => (window.workspaceHost ? window.workspaceHost.connection(slot) : (window.devHost ?? window.aiHost ?? window.commentsHost).connection()).reveal({ path }),
    [path, slot],
  )

/**
 * Puts the person's selection from Unicode character `anchor` to `focus` of the note: the host
 * reveals the anchor's line and column (`body:<line>:<column>`, which the note selects as the
 * person's caret), then Shift+Arrow keys extend it to `focus`, as a person would. The canvas must
 * have the keyboard for a selection. `run(command, params)` runs an engine command through the
 * host's AI bridge, whose reads answer the person's selection; `slot` is the workspace host's editor.
 */
export async function selectChars(page, run, anchor, focus = anchor, slot = 'a') {
  const chars = [...(await run('text.read')).text]
  const before = chars.slice(0, anchor)
  const lineStart = before.lastIndexOf('\n') + 1
  const line = before.filter((char) => char === '\n').length + 1
  const path = `body:${line}:${anchor - lineStart + 1}`
  const holds = async (wanted) => {
    for (let i = 0; i < 100; i += 1) {
      const now = await run('select.get')
      if (now.anchor === wanted.anchor && now.focus === wanted.focus) return true
      await page.waitForTimeout(20)
    }
    return false
  }
  await reveal(page, path, slot)
  if (!(await holds({ anchor, focus: anchor }))) throw new Error(`the host's reveal of ${path} did not put the caret at ${anchor}: ${JSON.stringify(await run('select.get'))}`)
  const key = focus > anchor ? 'Shift+ArrowRight' : 'Shift+ArrowLeft'
  for (let i = 0; i < Math.abs(focus - anchor); i += 1) await page.keyboard.press(key)
  if (!(await holds({ anchor, focus }))) throw new Error(`Shift+Arrow did not select ${anchor} to ${focus}: ${JSON.stringify(await run('select.get'))}`)
}

/**
 * Puts the caret right after `after` (its first occurrence), as the host shows a place.
 * `run(command, params)` runs an engine command; `read()` answers the engine's text.
 */
export async function caretAfter(page, run, read, after) {
  const text = await read()
  const at = text.indexOf(after)
  if (at < 0) throw new Error(`no ${JSON.stringify(after)} in the note`)
  await selectChars(page, run, scalars(text, at + after.length))
  // The editor asks the tool for the caret again after a key; an arrow pair leaves it in place.
  await page.keyboard.press('ArrowLeft')
  await page.keyboard.press('ArrowRight')
}

export async function selectText(page, run, read, wanted) {
  await caretAfter(page, run, read, wanted)
  for (let i = 0; i < [...wanted].length; i += 1) await page.keyboard.press('Shift+ArrowLeft')
}

/**
 * Clicks the canvas at the place of Unicode character `index`: where the engine's caret is with
 * the caret there (its line's syntax shown), asked in one agent run, whose selection is its own.
 */
export async function clickAtChar(page, run, index) {
  const at = await page.evaluate(async (index) => {
    const steps = [{ command: 'select.set', params: { anchor: index } }, { command: 'select.get', params: {} }]
    const { completed } = await window.devHost.connection().ai.run(steps, false)
    return completed[1].result.caret
  }, index)
  const frame = kitFrame(page)
  const [x, y, , , zoom] = (await frame.getByTestId('note-editor').getAttribute('data-view')).split(',').map(Number)
  const box = await frame.locator('[data-testid="note-canvas"] canvas').first().boundingBox()
  await page.mouse.click(box.x + (at.x + 2 - x) * zoom, box.y + (at.y + at.height / 2 - y) * zoom)
}
