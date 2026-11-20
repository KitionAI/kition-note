// The note editor's flow in a real browser, through the development host: node tests/flow.mjs [chromium|webkit]
// Starts the Vite dev server itself, opens a note, and prints PASS or FAIL per step; exits 1 on any FAIL.
// Needs the engine and fonts built into public/ (pnpm run engine). Run it with no HTTP proxy set.
// The engine holds and draws the note: steps read its text through the host's AI bridge and move
// the caret through its selection (tests/canvasHelpers.mjs). Input methods cannot be driven
// headless: they are checked in the desktop app.
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'
import { canvasReady, caretAfter as caretAfterIn, clickAtChar, focusCanvas, reveal, selectChars, selectText } from './canvasHelpers.mjs'

const which = process.argv[2] ?? 'chromium'
const sdkDir = process.env.KITION_SDK ?? new URL('../../../kition-sdk', import.meta.url).pathname
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const context = await browser.newContext({ viewport: { width: 1280, height: 900 } })
// These steps check unsaved edits: the note's autosave is off (src/autosave.tsx).
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

const NAME = 'Flow.md'
const ORIGINAL = ['---', 'title: Plan', 'tags:', '  - work', '---', '# Plan', '', 'Goals for the quarter.', '', '| Item | Owner |', '| ---- | ----- |', '| Ship | Ann   |', '| Test | Bo    |', '', 'Closing words.', '', '- [ ] Draft the plan', ''].join('\n')

const kit = () => page.frames().find((frame) => frame !== page.mainFrame())
/** Runs an engine command through the host's AI bridge and answers its result. */
const run = (command, params = {}) => page.evaluate(async ([command, params]) => (await window.devHost.connection().ai.run([{ command, params }], false)).completed[0].result, [command, params])
/** What the engine holds (`text.read`), as the agent sees it. */
const engineText = async () => (await run('text.read')).text
const caretAfter = (text) => caretAfterIn(page, run, engineText, text)
/** The caret (`selection.get`) and whether the editor shows it (`data-view` on the editor). */
async function caret() {
  const selection = await run('selection.get')
  const view = (await kit().getByTestId('note-editor').getAttribute('data-view'))?.split(',').map(Number)
  const [x, y, width, height] = view ?? [0, 0, 0, 0]
  const at = selection.caret
  const visible = Boolean(view) && at.y >= y - 1 && at.y + at.height <= y + height + 1 && at.x >= x - 1 && at.x <= x + width + 1
  return { line: selection.line, column: selection.column, text: selection.text, visible }
}
const clickAt = (index) => clickAtChar(page, run, index)
/** Saves through the host and answers the file's bytes as text. */
async function save() {
  const before = await page.evaluate((name) => window.devHost.files.get(name).version, NAME)
  await page.evaluate(() => window.devHost.connection().save())
  await until(async () => (await page.evaluate((name) => window.devHost.files.get(name).version, NAME)) > before)
  return page.evaluate((name) => new TextDecoder().decode(new Uint8Array(window.devHost.files.get(name).bytes)), NAME)
}
/** The lines of `after` that differ from `before` when both have the same number of lines; null otherwise. */
function changedLines(before, after) {
  const a = before.split('\n')
  const b = after.split('\n')
  if (a.length !== b.length) return null
  return a.map((line, index) => (line === b[index] ? null : [line, b[index]])).filter(Boolean)
}
const setFile = (name, text) =>
  page.evaluate(([name, text]) => {
    const file = window.devHost.files.get(name)
    window.devHost.files.set(name, { bytes: new TextEncoder().encode(text).buffer, version: file.version + 1 })
  }, [name, text])

try {
  await page.goto(`${origin}/dev.html?locale=en-US`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 90000 })
  await page.evaluate(async ([name, text]) => window.devHost.open(name, new TextEncoder().encode(text).buffer), [NAME, ORIGINAL])
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 90000 })
  const ready = await until(async () => (await canvasReady(page)) && (await engineText()) === ORIGINAL, 300)
  step('opens the note on the engine canvas', ready)
  // kit.json's `ui.display` and `ui.brand`: the Single line mode, in Kition's purple.
  step('opens in the Single line display mode', (await kit().locator('.kui-app').getAttribute('data-mode')) === 'line', String(await kit().locator('.kui-app').getAttribute('data-mode')))
  const brand = await kit().evaluate(() => ({ brand: document.documentElement.dataset.brand, primary: document.documentElement.style.getPropertyValue('--kit-primary').trim().toLowerCase() }))
  step('draws its primary color in the note brand', brand.brand === 'kit' && brand.primary === '#5645d4', JSON.stringify(brand))
  await focusCanvas(page)

  // Typing goes to the engine, which draws it.
  await caretAfter('Goals for the quarter.')
  await page.keyboard.type(' Typed.')
  const typed = ORIGINAL.replace('Goals for the quarter.', 'Goals for the quarter. Typed.')
  step('typing reaches the engine', await until(async () => (await engineText()) === typed), JSON.stringify((await engineText()).slice(0, 80)))
  step('saves the original bytes plus the typed text only', (await save()) === typed)

  // Typing while the agent edits elsewhere keeps both edits.
  await caretAfter('Closing words.')
  const agentAt = [...(await engineText())].join('').indexOf('# Plan\n') + '# Plan\n'.length
  const agent = run('text.insert', { at: agentAt, text: 'Agent line.\n' })
  await page.keyboard.type(' During.')
  await agent
  const both = await until(async () => {
    const text = await engineText()
    return text.includes('# Plan\nAgent line.\n') && text.includes('Closing words. During.')
  })
  step('typing while an agent edit runs keeps both edits', both, JSON.stringify(await engineText()))

  // An external change of the file merges with unsaved edits.
  await caretAfter('# Plan')
  await page.keyboard.type(' unsaved')
  await until(async () => (await engineText()).includes('# Plan unsaved'))
  const onDisk = await save()
  const external = onDisk.replace('Closing words. During.', 'Closing words. During.\n\nAdded on disk.')
  await caretAfter('Goals for the quarter. Typed.')
  await page.keyboard.type(' Local.')
  await until(async () => (await engineText()).includes('Typed. Local.'))
  await setFile(NAME, external)
  step('a change on disk under unsaved edits asks the person', await until(async () => (await kit().getByTestId('note-disk-changed').count()) === 1, 100))
  await kit().getByTestId('note-disk-merge').click()
  const keeps = await until(async () => {
    const text = await engineText()
    return text.includes('Added on disk.') && text.includes('Typed. Local.')
  })
  step('an external change merges and keeps unsaved edits', keeps, JSON.stringify(await engineText()))
  step('the question goes once merged', await until(async () => (await kit().getByTestId('note-disk-changed').count()) === 0))
  const mergedSave = await save()
  step('the merged note saves both sides', mergedSave.includes('Added on disk.') && mergedSave.includes('Typed. Local.'), JSON.stringify(mergedSave))

  // A table cell typed in place changes exactly one line of the saved file.
  await focusCanvas(page)
  await caretAfter('| Ship')
  for (let i = 0; i < 'Ship'.length; i += 1) await page.keyboard.press('Backspace')
  await page.keyboard.type('Launch')
  await until(async () => (await engineText()).includes('| Launch'))
  const cellDiff = changedLines(mergedSave, await save())
  step('a table cell edit changes exactly one line', cellDiff?.length === 1 && cellDiff[0][1].includes('Launch'), JSON.stringify(cellDiff))

  // A task box clicked on the canvas is the engine's task.toggle: one character of one line.
  const beforeTask = await save()
  const marker = [...(await engineText())].join('').indexOf('[ ] Draft')
  await clickAt([...(await engineText()).slice(0, marker + 1)].length)
  await until(async () => (await engineText()).includes('- [x] Draft the plan'))
  const taskDiff = changedLines(beforeTask, await save())
  step('a task box click toggles only its line', taskDiff?.length === 1 && taskDiff[0][1] === '- [x] Draft the plan', JSON.stringify(taskDiff))

  // Comments: a thread on the selection, then a reply, through the engine's comment commands.
  await focusCanvas(page)
  await selectText(page, run, engineText, 'Closing words')
  await kit().getByTestId('note-comments-toggle').click()
  await kit().getByTestId('note-comments-new').click()
  const composer = kit().locator('[data-testid="note-comment-panel"] textarea').first()
  await composer.fill('Is this the end?')
  await composer.press('ControlOrMeta+Enter')
  const threads = () => run('comment.threads')
  const added = await until(async () => JSON.stringify(await threads()).includes('Is this the end?'))
  step('adds a comment on the selection', added && JSON.stringify(await threads()).includes('"quote":"Closing words"'), JSON.stringify(await threads().catch((error) => String(error))))
  // The reply box comes with the thread, once the panel shows it.
  await until(async () => (await kit().locator('[data-testid="note-comment-panel"]').innerText()).includes('Is this the end?'))
  await until(async () => (await kit().locator('[data-testid="note-comment-panel"] textarea').count()) > 0)
  const reply = kit().locator('[data-testid="note-comment-panel"] textarea').last()
  await reply.fill('Yes.')
  await reply.press('ControlOrMeta+Enter')
  step('replies to the thread', await until(async () => JSON.stringify(await threads()).includes('Yes.')))
  step('comments stay out of the file', !(await save()).includes('Is this the end?'))
  await kit().getByTestId('note-comments-toggle').click()

  // The menus, one undo step per action, the panels, and Focus.
  {
    const menusPage = await context.newPage()
    menusPage.on('pageerror', (error) => errors.push(error.message))
    await menusPage.goto(`${origin}/dev.html?locale=en-US&modes=menus,line,classic,focus`)
    await menusPage.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 90000 })
    const NOTE = '# Plan\n\nGoals for the quarter.\n\n## Risks\n\n- [ ] Draft the plan\n'
    await menusPage.evaluate(async (text) => window.devHost.open('Menus.md', new TextEncoder().encode(text).buffer), NOTE)
    const frame = () => menusPage.frames().find((candidate) => candidate !== menusPage.mainFrame())
    const runHere = (command, params = {}) => menusPage.evaluate(async ([command, params]) => (await window.devHost.connection().ai.run([{ command, params }], false)).completed[0].result, [command, params])
    const text = async () => (await runHere('text.read')).text
    const waitText = async (wanted) => {
      for (let i = 0; i < 100; i += 1) {
        if ((await text().catch(() => '')) === wanted) return true
        await menusPage.waitForTimeout(100)
      }
      return false
    }
    await waitText(NOTE)
    for (let i = 0; i < 100 && !(await canvasReady(menusPage)); i += 1) await menusPage.waitForTimeout(100)
    const app = frame().locator('.kui-app')
    step('menus: the frame shows Menus', (await app.getAttribute('data-mode')) === 'menus', String(await app.getAttribute('data-mode')))
    await focusCanvas(menusPage)
    await selectText(menusPage, runHere, text, 'Goals')
    await frame().getByTestId('note-menu-Format').click()
    await frame().getByTestId('note-cmd-format.bold').click()
    const bold = NOTE.replace('Goals', '**Goals**')
    step('menus: Format > Bold wraps the selection', await waitText(bold), JSON.stringify(await text()))
    // A click on the task's box: the person's own step.
    const all = await text()
    await clickAtChar(menusPage, runHere, [...all.slice(0, all.indexOf('[ ]') + 1)].length)
    const ticked = bold.replace('- [ ]', '- [x]')
    step('menus: a click on the task box toggles it', await waitText(ticked), JSON.stringify(await text()))
    await focusCanvas(menusPage)
    await menusPage.keyboard.press('ControlOrMeta+z')
    await menusPage.keyboard.press('ControlOrMeta+z')
    step('two undos take back the task and the bold, one step each', await waitText(NOTE), JSON.stringify(await text()))
    await frame().getByTestId('note-redo').click()
    await frame().getByTestId('note-redo').click()
    step('the Redo button puts the bold back, then the task', await waitText(ticked), JSON.stringify(await text()))
    // View > Outline: the engine's headings; a row moves the caret to its heading.
    await frame().getByTestId('note-menu-View').click()
    await frame().getByTestId('note-control-outline').click()
    const rows = frame().getByTestId('note-outline-row')
    for (let i = 0; i < 50 && (await rows.count()) < 2; i += 1) await menusPage.waitForTimeout(100)
    step('the outline lists the headings', (await rows.allInnerTexts()).join('|') === 'Plan|Risks', (await rows.allInnerTexts()).join('|'))
    await rows.nth(1).click()
    await menusPage.waitForTimeout(200)
    const where = await runHere('selection.get')
    step('an outline row puts the caret on its heading', where.line === 5, JSON.stringify(where))
    await frame().getByTestId('note-comments-toggle').click()
    step('the comments panel opens beside the outline', (await frame().getByTestId('note-comments').count()) === 1)
    // Ids: every control drawn is one the editor lists (Kit UI protocol section 9).
    const { checkUiIds } = await import(new URL('packages/kit-dev-host/src/checkUiIds.mjs', `file://${sdkDir}/`).href)
    const problems = await checkUiIds(frame(), await menusPage.evaluate(() => window.devHost.connection().elements()))
    step('every control has a Kit UI id', problems.length === 0, JSON.stringify(problems).slice(0, 400))
    // Focus hides the menus and the panels; Escape brings them back.
    await app.focus()
    await menusPage.keyboard.press('ControlOrMeta+Backslash')
    await menusPage.waitForTimeout(200)
    step('Focus hides the menus and the panels', (await app.getAttribute('data-mode')) === 'focus' && (await frame().getByTestId('note-outline').count()) === 0 && (await frame().getByTestId('note-menu-Format').count()) === 0)
    await menusPage.keyboard.press('Escape')
    await menusPage.waitForTimeout(200)
    step('Escape leaves Focus with the panels open', (await app.getAttribute('data-mode')) === 'menus' && (await frame().getByTestId('note-outline').count()) === 1)
    // Classic: the same rows as a ribbon.
    await menusPage.goto(`${origin}/dev.html?locale=en-US&modes=classic,focus`)
    await menusPage.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 90000 })
    await frame().getByTestId('ribbon-tab-Insert').click()
    step('Classic draws the Insert tab as a ribbon', (await frame().getByTestId('note-cmd-insert.table').count()) === 1)
    await menusPage.close()
  }

  // A note without unsaved edits follows its file: the host's new version shows without a question,
  // merged into the open note, which keeps the caret and stays saved.
  {
    const clean = await save()
    const outside = clean.replace('Closing words. During.', 'Closing words. During. Changed outside.')
    // In the body (the caret never stays in the front matter).
    const goals = [...clean.slice(0, clean.indexOf('Goals') + 2)].length
    await selectChars(page, run, goals)
    await setFile(NAME, outside)
    const followed = await until(async () => (await engineText()) === outside, 100)
    step("a clean note takes its file's change from disk", followed && (await kit().getByTestId('note-disk-changed').count()) === 0, JSON.stringify(await engineText()))
    step('it is merged in place: the caret stays where it was', (await run('selection.get')).focus === goals, JSON.stringify(await run('selection.get')))
    const state = await page.evaluate(() => window.devHost.connection().state())
    step('and the note stays saved', state.dirty === false, JSON.stringify(state))
    // Keep mine: the note stays as it is and the next save writes over the file.
    await focusCanvas(page)
    await caretAfter('Closing words. During. Changed outside.')
    await page.keyboard.type(' Mine.')
    await until(async () => (await engineText()).includes('Changed outside. Mine.'))
    await setFile(NAME, outside.replace('Changed outside.', 'Changed outside again.'))
    await until(async () => (await kit().getByTestId('note-disk-changed').count()) === 1, 100)
    await kit().getByTestId('note-disk-keep').click()
    // The choice settles before the save the person makes next.
    await until(async () => (await kit().getByTestId('note-disk-changed').count()) === 0)
    await page.waitForTimeout(200)
    const kept = await save()
    step('Keep mine keeps the note and saves it over the file', kept.includes('Changed outside. Mine.') && !kept.includes('again') && (await kit().getByTestId('note-disk-changed').count()) === 0, JSON.stringify(kept))
  }

  // The host shows a place of the note (document.view on open, document/reveal while open; OP3):
  // a line (a search hit), a heading path, a block id; the editor scrolls it into view.
  {
    const lines = ['# Long', '']
    for (let i = 1; i <= 150; i += 1) lines.push(`Paragraph ${i}.`, '')
    lines.push('## Far heading', '', 'Under the far heading. ^far-block', '')
    const LONG = lines.join('\n')
    await page.evaluate(async (text) => window.devHost.open('Long.md', new TextEncoder().encode(text).buffer, { view: { path: 'body:201' } }), LONG)
    await until(async () => (await canvasReady(page)) && (await engineText().catch(() => '')) === LONG, 300)
    const opened = await until(async () => (await caret()).line === 201 && (await caret()).visible, 50)
    const first = await caret()
    step('opening at a line selects it and shows it', opened && first.text === 'Paragraph 100.', JSON.stringify(first))
    await page.evaluate(() => window.devHost.connection().reveal({ heading: ['Long', 'Far heading'] }))
    const heading = await until(async () => (await caret()).text === '## Far heading' && (await caret()).visible, 50)
    step('a heading path is revealed', heading, JSON.stringify(await caret()))
    await page.evaluate(() => window.devHost.connection().reveal({ path: 'body:3:5' }))
    const column = await until(async () => {
      const at = await caret()
      return at.line === 3 && at.column === 5 && at.text === '' && at.visible
    }, 50)
    step('a line and column put the caret there', column, JSON.stringify(await caret()))
    await page.evaluate(() => window.devHost.connection().reveal({ blockId: 'far-block' }))
    step('a block id is revealed', await until(async () => (await caret()).text === 'Under the far heading. ^far-block', 50), JSON.stringify(await caret()))
    await page.evaluate(() => window.devHost.connection().reveal({ heading: ['Missing'] }))
    await page.waitForTimeout(300)
    step('a place the note does not hold changes nothing', (await caret()).text === 'Under the far heading. ^far-block')
  }

  // A note with mixed line endings keeps its `\r` characters; typing at a line's end goes before a
  // hidden trailing `\r`, and a line break typed there goes after it.
  const MIXED = 'one\r\ntwo\nthree\r\n'
  await page.evaluate(async (text) => window.devHost.open('Mixed.md', new TextEncoder().encode(text).buffer), MIXED)
  await until(async () => (await canvasReady(page)) && (await engineText().catch(() => '')) === MIXED, 300)
  await focusCanvas(page)
  // The host shows the first line at a column past its end: the caret goes to the line's end.
  await reveal(page, 'body:1:100')
  await until(async () => (await run('selection.get')).line === 1 && (await run('selection.get')).column > 1)
  await page.keyboard.type('X')
  await page.keyboard.press('End')
  await page.keyboard.press('Enter')
  const mixed = await until(async () => (await engineText()) === 'oneX\r\n\ntwo\nthree\r\n')
  step('typing at the end of a CRLF line in a mixed note keeps the line ending', mixed, JSON.stringify(await engineText()))

  // Properties are edited in their block, as Obsidian's: a value where it is shown, a tag after
  // the pills, a new property in Add property's place; the caret never enters the front matter.
  {
    const PROPS = '---\ntitle: Plan\ntags:\n  - work\n---\nBody text.\n'
    await page.evaluate(async (text) => window.devHost.open('Props.md', new TextEncoder().encode(text).buffer), PROPS)
    await until(async () => (await canvasReady(page)) && (await engineText().catch(() => '')) === PROPS, 300)
    await kit().getByTestId('note-property').first().waitFor()
    const value = kit().locator('[data-testid="note-property"][data-key="title"] [data-testid="note-property-text"]')
    await value.click()
    await page.keyboard.press('Meta+A')
    await page.keyboard.type('Launch')
    await page.keyboard.press('Enter')
    const valueSet = await until(async () => (await engineText()).includes('title: Launch'))
    step('a property value is edited where it is shown', valueSet, JSON.stringify(await engineText()))
    await kit().locator('[data-testid="note-property"][data-key="tags"] [data-testid="note-property-new"]').click()
    await page.keyboard.type('plan')
    await page.keyboard.press('Enter')
    const tagged = await until(async () => /- work\n\s*- plan/.test(await engineText()))
    step('a tag typed after the pills is added', tagged, JSON.stringify(await engineText()))
    await kit().getByTestId('note-properties-add').click()
    await page.keyboard.type('status')
    await page.keyboard.press('Enter')
    const added = await until(async () => /\nstatus:/.test(await engineText()))
    step('Add property adds the named key', added, JSON.stringify(await engineText()))
    await kit().locator('[data-testid="note-property"][data-key="status"] [data-testid="note-property-icon"]').click()
    await kit().getByTestId('note-property-remove').click()
    const removed = await until(async () => !/\nstatus:/.test(await engineText()))
    step("a property's icon menu removes it", removed, JSON.stringify(await engineText()))
    await reveal(page, 'body:1:1')
    await until(async () => (await run('selection.get')).line > 1, 30)
    const { caret: at } = await run('selection.get')
    step('the caret given the front matter goes past it', (await run('selection.get')).line > 1, JSON.stringify(at))
  }

  // Hovering a heading shows its fold chevron in the margin; the pointer leaving the note takes
  // it away, as Obsidian hides it.
  {
    const HOVER = 'text\n\n# Head\n\nmore\n'
    await page.evaluate(async (text) => window.devHost.open('Hover.md', new TextEncoder().encode(text).buffer), HOVER)
    await until(async () => (await canvasReady(page)) && (await engineText().catch(() => '')) === HOVER, 300)
    const at = await run('selection.where', { at: HOVER.indexOf('Head') })
    const [x, y, , , zoom] = (await kit().getByTestId('note-editor').getAttribute('data-view')).split(',').map(Number)
    const box = await kit().locator('[data-testid="note-canvas"] canvas').first().boundingBox()
    const left = box.x + (at.x - x) * zoom
    const middle = box.y + (at.y + at.height / 2 - y) * zoom
    // How much of the margin beside the heading is not paper.
    const ink = async () => {
      const shot = await page.screenshot({ clip: { x: left - 30, y: middle - 10, width: 24, height: 20 } })
      return page.evaluate(async (data) => {
        const image = await createImageBitmap(new Blob([new Uint8Array(data)], { type: 'image/png' }))
        const canvas = new OffscreenCanvas(image.width, image.height)
        const context = canvas.getContext('2d')
        context.drawImage(image, 0, 0)
        const pixels = context.getImageData(0, 0, image.width, image.height).data
        let count = 0
        for (let index = 0; index < pixels.length; index += 4) if (pixels[index] < 235) count += 1
        return count
      }, [...shot])
    }
    await page.mouse.move(left + 40, middle - 4)
    await page.mouse.move(left + 60, middle)
    const shown = await until(async () => (await ink()) > 0, 30)
    await page.mouse.move(1, 1)
    const gone = await until(async () => (await ink()) === 0, 30)
    step('a hovered heading shows its chevron, gone when the pointer leaves the note', shown && gone, `${shown} ${gone}`)
  }

  step('no page errors', errors.length === 0, errors.join(' | '))
} catch (error) {
  step('flow ran to the end', false, error instanceof Error ? error.stack : String(error))
} finally {
  await browser.close()
  await server.close()
}
console.log(failures ? `${failures} step(s) failed in ${which}` : `all steps passed in ${which}`)
process.exit(failures ? 1 : 0)
