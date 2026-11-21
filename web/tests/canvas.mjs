// The note editor on the engine's canvas, in a real browser through the development host:
// node tests/canvas.mjs [chromium|webkit]. Starts the Vite dev server itself, opens a note, and
// prints PASS or FAIL per step; exits 1 on any FAIL. Needs the engine and fonts built into
// public/ (pnpm run engine). Input methods cannot be driven headless: they are checked in the
// desktop app. SHOT=<dir> writes a screenshot of the editor there.
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'
import { primaryKey, selectChars } from './canvasHelpers.mjs'

const which = process.argv[2] ?? 'chromium'
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const context = await browser.newContext({ viewport: { width: 1280, height: 900 } })
await context.addInitScript(() => localStorage.setItem('kit.note.autosave', 'off'))
const page = await context.newPage()
const mod = await primaryKey(page)
const errors = []
let failures = 0
page.on('pageerror', (error) => errors.push(error.message))
const step = (name, ok, detail = '') => {
  if (!ok) failures += 1
  console.log(`${ok ? 'PASS' : 'FAIL'} ${name}${detail ? ` - ${detail}` : ''}`)
}
const until = async (check, tries = 100) => {
  for (let i = 0; i < tries; i += 1) {
    if (await check().catch(() => false)) return true
    await page.waitForTimeout(100)
  }
  return false
}

const NAME = 'Canvas.md'
const ORIGINAL = ['# Plan', '', 'Goals for the quarter.', '', '- first', '', 'Closing words.', ''].join('\n')
const run = (command, params = {}) => page.evaluate(async ([command, params]) => (await window.devHost.connection().ai.run([{ command, params }], false)).completed[0].result, [command, params])
const engineText = async () => (await run('text.read')).text
const kit = () => page.frames().find((frame) => frame !== page.mainFrame())
/** Puts the person's caret right after `text`, as the host shows a place (tests/canvasHelpers.mjs). */
async function caretAfter(text) {
  const all = await engineText()
  const at = all.indexOf(text) + text.length
  await selectChars(page, run, [...all.slice(0, at)].length)
}

try {
  await page.goto(`${origin}/dev.html?locale=en-US`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 90000 })
  // Once open, the status line names the note; it may already say the revision a view change moved.
  await page.evaluate(async ([name, text]) => window.devHost.open(name, new TextEncoder().encode(text).buffer), [NAME, ORIGINAL])
  await page.waitForFunction((name) => document.getElementById('status')?.textContent?.startsWith(name), NAME, { timeout: 90000 })
  const ready = await until(async () => (await kit().locator('[data-testid="note-canvas"] canvas').count()) > 0, 300)
  step('opens the note on the engine canvas', ready)
  step('no CodeMirror editor is left', (await kit().locator('.cm-content').count()) === 0)
  step('the engine holds the text', (await engineText()) === ORIGINAL)

  // A click on the canvas focuses the text field the engine's text tool types through.
  const canvasBox = await kit().locator('[data-testid="note-canvas"] canvas').first().boundingBox()
  await page.mouse.click(canvasBox.x + canvasBox.width / 2, canvasBox.y + 120)
  await caretAfter('Goals for the quarter.')
  await page.keyboard.type(' Typed here.')
  step('typing reaches the engine', await until(async () => (await engineText()).includes('Goals for the quarter. Typed here.')), JSON.stringify(await engineText()))

  await caretAfter('- first')
  await page.keyboard.press('Enter')
  await page.keyboard.type('second')
  step('Enter continues the list', await until(async () => (await engineText()).includes('- first\n- second')), JSON.stringify(await engineText()))

  await page.keyboard.press('Backspace')
  await page.keyboard.press('Backspace')
  step('Backspace deletes before the caret', await until(async () => (await engineText()).includes('- seco\n')))

  // Shift+Arrow selects; Cmd/Ctrl+B makes it bold (the engine's command and shortcut).
  await caretAfter('Closing words')
  for (let i = 0; i < 5; i += 1) await page.keyboard.press('Shift+ArrowLeft')
  await page.keyboard.press(`${mod}+b`)
  step('the shortcut formats the selection', await until(async () => (await engineText()).includes('Closing **words**')), JSON.stringify(await engineText()))

  await page.keyboard.press(`${mod}+z`)
  step('Undo takes the formatting back', await until(async () => (await engineText()).includes('Closing words.')))

  // Paste arrives as typed text, line breaks kept.
  await caretAfter('# Plan')
  await kit().evaluate(() => {
    const sink = document.querySelector('[data-testid="kit-text-sink"]')
    const data = new DataTransfer()
    data.setData('text/plain', '\nPasted line')
    sink.value = '\nPasted line'
    sink.dispatchEvent(new InputEvent('input', { bubbles: true, data: '\nPasted line', inputType: 'insertFromPaste' }))
  })
  step('pasted text keeps its line break', await until(async () => (await engineText()).startsWith('# Plan\nPasted line\n')), JSON.stringify(await engineText()))

  // The slash menu: `/` on an empty line opens the block commands; typing filters, Enter runs.
  {
    await caretAfter('Closing words.')
    await page.keyboard.press('Enter')
    await page.keyboard.press('Enter')
    await page.keyboard.type('/tab')
    const menu = kit().getByTestId('note-slash')
    step('typing / opens the slash menu, filtered', await until(async () => (await menu.count()) === 1 && (await kit().getByTestId('note-slash-item').first().getAttribute('data-command')) === 'insert.table'))
    await page.keyboard.press('Enter')
    // The table Obsidian's Insert table makes: two empty columns (measured).
    step('Enter runs it: the /query goes and a table comes', await until(async () => (await engineText()).includes('|     |     |\n| --- | --- |') && !(await engineText()).includes('/tab')), JSON.stringify(await engineText()))
    step('the menu closes', await until(async () => (await menu.count()) === 0))
  }

  // Tag autocomplete: `#` and a tag's first letters offer the note's tags; Enter completes.
  {
    await caretAfter('Closing words.')
    await page.keyboard.press('Enter')
    await page.keyboard.type('#roadmap first, #ro')
    const menu = kit().getByTestId('note-tags')
    step('typing #ro offers the note\'s #roadmap', await until(async () => (await menu.count()) === 1 && (await kit().getByTestId('note-tag-item').first().textContent()) === '#roadmap'))
    await page.keyboard.press('Enter')
    step('Enter completes the tag', await until(async () => (await engineText()).includes('#roadmap first, #roadmap \n') || (await engineText()).endsWith('#roadmap first, #roadmap ')), JSON.stringify(await engineText()))
    step('the tag menu closes', await until(async () => (await menu.count()) === 0))
  }

  // The note scrolls like a page: the wheel never takes it above its top or sideways.
  {
    const canvas = kit().locator('[data-testid="note-canvas"] canvas').first()
    const box = await canvas.boundingBox()
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
    await page.mouse.wheel(300, -3000)
    await page.waitForTimeout(300)
    const [vx, vy] = (await kit().getByTestId('note-editor').getAttribute('data-view')).split(',').map(Number)
    step('the wheel stops at the note\'s top and does not pan sideways', vy === 0 && vx === 0, `${vx},${vy}`)
  }

  // Moving the caret to another line repaints the canvas: that line shows its syntax, the old one
  // hides it (the document did not change, the pixels did).
  {
    const canvas = kit().locator('[data-testid="note-canvas"] canvas').first()
    const pixels = () => canvas.evaluate((element) => element.toDataURL())
    await caretAfter('Closing words.')
    await page.keyboard.press('ArrowLeft')
    await page.keyboard.press('ArrowRight')
    await page.waitForTimeout(800)
    const before = await pixels()
    const line = async () => (await run('selection.get')).line
    const startLine = await line()
    await page.waitForTimeout(800)
    const settled = await pixels()
    await page.keyboard.press('ArrowLeft')
    await page.keyboard.press('ArrowRight')
    // Only keys until the pixels are read: a command through the session repaints by itself.
    for (let i = 1; i < startLine; i += 1) await page.keyboard.press('ArrowUp')
    await page.waitForTimeout(800)
    const after = await pixels()
    step('the caret reaches the heading by keys', (await line()) === 1, `${startLine} -> ${await line()}`)
    step('the canvas repaints the heading with its # mark', after !== settled && settled === before)
  }

  // The fold chevron beside a heading hides its section; a second click shows it again.
  {
    const heading = await run('selection.where', { at: 0 })
    const [vx, vy, , , zoom] = (await kit().getByTestId('note-editor').getAttribute('data-view')).split(',').map(Number)
    const box = await kit().locator('[data-testid="note-canvas"] canvas').first().boundingBox()
    const chevron = { x: box.x + (heading.x - 16 - vx) * zoom, y: box.y + (heading.y + heading.height / 2 - vy) * zoom }
    const body = [...(await engineText()).slice(0, (await engineText()).indexOf('Goals'))].length
    await page.mouse.click(chevron.x, chevron.y)
    // A byte the fold hides has its caret drawn at the folded heading's end (Obsidian's fold).
    const folded = async () => {
      const where = await run('selection.where', { at: body })
      return where.y === heading.y && where.x > heading.x
    }
    step('the chevron folds the heading\'s section', await until(folded))
    await page.mouse.click(chevron.x, chevron.y)
    step('a second click unfolds it', await until(async () => (await run('selection.where', { at: body })).y > heading.y))
  }

  // An input method (Chromium drives one over CDP): the text being composed shows at the caret,
  // the field sits at the caret for the candidate window, and only the committed text reaches the note.
  if (which === 'chromium') {
    await caretAfter('Goals for the quarter. Typed here.')
    const cdp = await page.context().newCDPSession(page)
    await cdp.send('Input.imeSetComposition', { text: 'zhong', selectionStart: 5, selectionEnd: 5 })
    const composing = await until(async () => (await kit().getByTestId('note-composition').innerText().catch(() => '')) === 'zhong', 30)
    step('the composition shows at the caret', composing)
    // The text field follows the caret a moment after the composition shows.
    const near = await until(() =>
      kit().evaluate(() => {
        const sink = document.querySelector('[data-testid="kit-text-sink"]').getBoundingClientRect()
        const shown = document.querySelector('[data-testid="note-composition"]').getBoundingClientRect()
        return Math.abs(sink.left - shown.left) < 4 && sink.top >= shown.top
      }),
      30,
    )
    step('the text field sits at the caret for the candidate window', near)
    step('nothing reaches the note while composing', !(await engineText()).includes('zhong'))
    await cdp.send('Input.insertText', { text: '中文' })
    step('the committed text reaches the note', await until(async () => (await engineText()).includes('Typed here.中文')), JSON.stringify(await engineText()))
    step('the composition goes once committed', await until(async () => (await kit().getByTestId('note-composition').count()) === 0, 30))
  }

  if (process.env.SHOT) await page.screenshot({ path: `${process.env.SHOT}/canvas-${which}.png` })
  step('no page errors', errors.length === 0, errors.join(' | '))
} catch (error) {
  step('runs without throwing', false, String(error?.stack ?? error))
} finally {
  await browser.close()
  await server.close()
}
process.exit(failures ? 1 : 0)
