// Comment threads a host keeps itself (Kit UI protocol 2.6 section 7k, capability `comment-anchors`),
// through tests/comments-host.html: a selection's Comment sends `comment.create` with its anchor,
// the host's threads are marked on their text and stay on it as the note changes, a marker's click
// opens the thread in the host (`comments/activated`), and a viewer is offered no Comment.
// node tests/comments.mjs [chromium|webkit]
// Prints PASS or FAIL per step; exits 1 on any FAIL. Needs the engine built into public/engine.
// Run it with no HTTP proxy set.
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'
import { canvasReady, caretAfter, focusCanvas, kitFrame, selectText } from './canvasHelpers.mjs'

const which = process.argv[2] ?? 'chromium'
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const context = await browser.newContext({ viewport: { width: 1280, height: 900 } })
await context.addInitScript(() => localStorage.setItem('kit.note.autosave', 'off'))
const page = await context.newPage()
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

const NOTE = ['# Plan', '', 'Goals for the quarter.', '', 'Closing words.', ''].join('\n')
const MOD = process.platform === 'darwin' ? 'Meta' : 'Control'
const kit = () => kitFrame(page)
const run = (command, params = {}) => page.evaluate(async ([command, params]) => (await window.commentsHost.connection().ai.run([{ command, params }], false)).completed[0].result, [command, params])
const engineText = async () => (await run('text.read')).text
const select = (text) => selectText(page, run, engineText, text)
const hostState = () => page.evaluate(() => ({ created: window.commentsHost.created, activated: window.commentsHost.activated, threads: window.commentsHost.threads }))
const markers = () => kit().getByTestId('note-thread-marker')
/** The ranges' tinted rectangles, in the frame's pixels. */
const rangeBoxes = async () => Promise.all((await kit().getByTestId('note-thread-range').all()).map((range) => range.boundingBox()))
/** Where the engine draws Unicode character `index`, in the frame's pixels. */
async function charBox(index) {
  const at = await run('selection.where', { at: index })
  const [x, y, , , zoom] = (await kit().getByTestId('note-editor').getAttribute('data-view')).split(',').map(Number)
  const box = await kit().locator('[data-testid="note-canvas"] canvas').first().boundingBox()
  return { x: box.x + (at.x - x) * zoom, y: box.y + (at.y - y) * zoom }
}
const charsBefore = (text, part) => [...text.slice(0, text.indexOf(part))].length
/** Selects `part` with the mouse, as a person who may not edit the note selects (keys do not move a read-only caret). */
async function dragSelect(part) {
  const text = await engineText()
  const from = await charBox(charsBefore(text, part))
  const to = await charBox(charsBefore(text, part) + [...part].length)
  await page.mouse.move(from.x + 1, from.y + 10)
  await page.mouse.down()
  await page.mouse.move(to.x - 1, to.y + 10, { steps: 8 })
  await page.mouse.up()
}
async function open(role = 'editor', text = NOTE) {
  await page.goto(`${origin}/tests/comments-host.html`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent === 'ready', null, { timeout: 90000 })
  await page.evaluate(async ([text, role]) => window.commentsHost.open('Plan.md', text, role), [text, role])
  const ready = await until(async () => (await canvasReady(page)) && (await engineText()) === text, 300)
  if (ready) await focusCanvas(page)
  return ready
}

try {
  step('opens the note in a host that keeps comment threads', await open())
  step('the kit lists comment-anchors', (await page.evaluate(() => window.commentsHost.connection().capabilities)).includes('comment-anchors'))
  step('the kit shows no comments panel of its own', (await kit().getByTestId('note-comments-toggle').count()) === 0)

  // create: a selection offers Comment, which sends comment.create with the selection's anchor.
  await select('Goals for the quarter')
  const add = kit().getByTestId('note-comment-add')
  step('a selection offers Comment', await until(async () => (await add.count()) === 1))
  await add.click()
  step('Comment sends comment.create', await until(async () => (await hostState()).created.length === 1))
  const [created] = (await hostState()).created
  const goals = charsBefore(NOTE, 'Goals')
  step(
    'the anchor is the selection on the note text, with its quote',
    JSON.stringify(created.anchor) === JSON.stringify({ ids: ['note'], range: { start: goals, end: goals + 21 }, quote: 'Goals for the quarter' }),
    JSON.stringify(created),
  )

  // marker shown: the host's thread is marked on its text.
  step('the thread is marked', await until(async () => (await markers().count()) === 1))
  step('the marker names the host thread', (await markers().first().getAttribute('data-thread')) === 'thread-1')
  {
    const [first] = await rangeBoxes()
    const start = await charBox(goals)
    step('the tint covers the anchored text', first !== undefined && Math.abs(first.x - start.x) < 3 && Math.abs(first.y - start.y) < 6, JSON.stringify({ first, start }))
  }

  // The anchor survives edits: text typed above moves the mark down with its text.
  const before = (await rangeBoxes())[0]
  await caretAfter(page, run, engineText, '# Plan')
  await page.keyboard.press('Enter')
  await page.keyboard.type('A new line above.')
  await page.keyboard.press('Enter')
  const typed = await engineText()
  step('the note changed', typed.includes('A new line above.'))
  const moved = async () => {
    const [box] = await rangeBoxes()
    const start = await charBox(charsBefore(await engineText(), 'Goals'))
    return box !== undefined && box.y > before.y + 10 && Math.abs(box.x - start.x) < 3 && Math.abs(box.y - start.y) < 6
  }
  step('the mark moves with its text', await until(moved), JSON.stringify(await rangeBoxes()))
  // Text typed inside the anchored text joins it.
  await caretAfter(page, run, engineText, 'Goals for')
  await page.keyboard.type(' all of')
  step('the note took the typing inside the anchor', (await engineText()).includes('Goals for all of the quarter'))
  const widened = async () => {
    const text = await engineText()
    const boxes = await rangeBoxes()
    const end = await charBox(charsBefore(text, 'quarter') + 7)
    const last = boxes.at(-1)
    return last !== undefined && Math.abs(last.x + last.width - end.x) < 3
  }
  step('the mark widens over the text typed inside it', await until(widened))
  step(
    'the host finds the stored anchor in the changed note (comments/locate)',
    JSON.stringify(await page.evaluate(async (anchor) => window.commentsHost.locate([anchor, { ...anchor, quote: 'not in the note' }]), created.anchor)) === '[true,false]',
  )

  // click opens thread: a marker's click tells the host which thread.
  await markers().first().click()
  step('a marker click sends comments/activated', await until(async () => (await hostState()).activated.join() === 'thread-1'), JSON.stringify((await hostState()).activated))

  // The shortcut and the context menu start a thread too.
  await select('Closing words')
  step('the keyboard still selects after a marker click', await until(async () => (await run('selection.get')).text === 'Closing words'), JSON.stringify(await run('selection.get')))
  await page.keyboard.press(`${MOD}+Alt+KeyM`)
  step('Mod+Alt+M sends comment.create', await until(async () => (await hostState()).created.length === 2))
  step('on the selection', (await hostState()).created[1]?.anchor.quote === 'Closing words', JSON.stringify((await hostState()).created[1]))
  step('the second thread is marked', await until(async () => (await markers().count()) === 2))
  await select('Plan')
  const at = await charBox(charsBefore(await engineText(), 'Plan') + 1)
  await page.mouse.click(at.x + 2, at.y + 8, { button: 'right' })
  const item = kit().getByTestId('note-menu-comment-add')
  step('the context menu offers Comment', await until(async () => (await item.count()) === 1))
  await item.click()
  step('the context menu sends comment.create', await until(async () => (await hostState()).created.length === 3))

  // A resolved thread has no mark; the host's list is the whole list each time.
  await page.evaluate(() => {
    window.commentsHost.threads[0].resolved = true
    window.commentsHost.show()
  })
  step('a resolved thread is not marked', await until(async () => (await markers().count()) === 2 && (await kit().locator('[data-testid="note-thread-marker"][data-thread="thread-1"]').count()) === 0))

  // Reopened on a changed file, a stored anchor is found again by its quote.
  const stored = (await hostState()).threads.find((thread) => thread.id === 'thread-2')
  await page.evaluate(() => {
    window.commentsHost.threads = []
  })
  step('opens the note changed elsewhere', await open('editor', NOTE.replace('# Plan', '# Plan\n\nWritten while it was closed.')))
  await page.evaluate((thread) => {
    window.commentsHost.threads = [thread]
    window.commentsHost.show()
  }, stored)
  step('a stored anchor is marked where its text is now', await until(async () => {
    const [box] = await rangeBoxes()
    const start = await charBox(charsBefore(await engineText(), 'Closing words'))
    return box !== undefined && Math.abs(box.x - start.x) < 3 && Math.abs(box.y - start.y) < 6
  }), JSON.stringify({ stored, boxes: await rangeBoxes() }))

  // A commenter (read-only, may comment) starts threads; a viewer is offered none.
  step('opens the note as a commenter', await open('commenter'))
  await dragSelect('Goals for the quarter')
  step('a commenter selects with the mouse, and sees the selection', await until(async () => (await run('selection.get')).text === 'Goals for the quarter' && (await kit().getByTestId('note-readonly-selection').count()) === 1))
  step('a commenter is offered Comment', await until(async () => (await kit().getByTestId('note-comment-add').count()) === 1), JSON.stringify(await run('selection.get')))
  await kit().getByTestId('note-comment-add').click()
  step('a commenter’s Comment sends comment.create', await until(async () => (await hostState()).created.length === 1))
  step('opens the note as a viewer', await open('viewer'))
  await page.evaluate((thread) => {
    window.commentsHost.threads = [thread]
    window.commentsHost.show()
  }, { ...stored, anchor: { ids: ['note'], range: { start: charsBefore(NOTE, 'Closing'), end: charsBefore(NOTE, 'Closing') + 13 }, quote: 'Closing words' } })
  step('a viewer sees the threads marked', await until(async () => (await markers().count()) === 1))
  await dragSelect('Goals for the quarter')
  step('a viewer selects text', await until(async () => (await run('selection.get')).text === 'Goals for the quarter'), JSON.stringify(await run('selection.get')))
  await page.waitForTimeout(400)
  step('a viewer is offered no Comment on a selection', (await kit().getByTestId('note-comment-add').count()) === 0)
  const sent = (await hostState()).created.length
  await page.keyboard.press(`${MOD}+Alt+KeyM`)
  await page.waitForTimeout(300)
  step('a viewer’s shortcut sends no comment.create', (await hostState()).created.length === sent)
  {
    const point = await charBox(charsBefore(NOTE, 'Goals') + 2)
    await page.mouse.click(point.x, point.y + 8, { button: 'right' })
    await until(async () => (await kit().getByTestId('note-context-menu').count()) === 1, 20)
    step('a viewer’s context menu has no Comment', (await kit().getByTestId('note-menu-comment-add').count()) === 0)
    await page.keyboard.press('Escape')
  }
  await markers().first().click()
  step('a viewer opens a thread from its marker', await until(async () => (await hostState()).activated.length === 1))

  step('no page errors', errors.length === 0, errors.join('; '))
} catch (error) {
  step('runs without throwing', false, error instanceof Error ? error.stack : String(error))
} finally {
  await browser.close()
  await server.close()
}
process.exit(failures ? 1 : 0)
