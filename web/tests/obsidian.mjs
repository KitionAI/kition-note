// The editor behaves as Obsidian's live preview does: real keys and clicks in a real
// browser, through the development host: node tests/obsidian.mjs [chromium|webkit]. Each scenario
// sets the note's text and selection (`|` is the caret, `‹…›` a selection), presses keys, and
// checks the text and the caret after. Prints PASS or FAIL per scenario; exits 1 on any FAIL.
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'

const which = process.argv[2] ?? 'chromium'
const mod = process.platform === 'darwin' ? 'Meta' : 'Control'
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const context = await browser.newContext({ viewport: { width: 1280, height: 900 } })
await context.addInitScript(() => localStorage.setItem('kit.note.autosave', 'off'))
if (which === 'chromium') await context.grantPermissions(['clipboard-read', 'clipboard-write'])
const page = await context.newPage()
const errors = []
let failures = 0
page.on('pageerror', (error) => errors.push(error.message))
const step = (name, ok, detail = '') => {
  if (!ok) failures += 1
  console.log(`${ok ? 'PASS' : 'FAIL'} ${name}${detail ? ` - ${detail}` : ''}`)
}
const until = async (check, tries = 60) => {
  for (let i = 0; i < tries; i += 1) {
    if (await check().catch(() => false)) return true
    await page.waitForTimeout(50)
  }
  return false
}
const run = (command, params = {}) => page.evaluate(async ([command, params]) => (await window.devHost.connection().ai.run([{ command, params }], false)).completed[0].result, [command, params])
const kit = () => page.frames().find((frame) => frame !== page.mainFrame())
const canvas = () => kit().locator('[data-testid="note-canvas"] canvas').first()

/** `text` with `|` (the caret) or `‹…›` (a selection) taken out, and where they were (in characters). */
function parse(marked) {
  const chars = [...marked]
  const out = []
  let anchor = null
  let focus = null
  for (const c of chars) {
    if (c === '|') anchor = focus = out.length
    else if (c === '‹') anchor = out.length
    else if (c === '›') focus = out.length
    else out.push(c)
  }
  return { text: out.join(''), anchor: anchor ?? out.length, focus: focus ?? anchor ?? out.length }
}

/** The note now, marked as `parse` reads it. */
async function marked() {
  const text = [...(await run('text.read')).text]
  const selection = await run('selection.get')
  const [from, to] = [Math.min(selection.anchor, selection.focus), Math.max(selection.anchor, selection.focus)]
  if (from === to) return [...text.slice(0, from), '|', ...text.slice(from)].join('')
  return [...text.slice(0, from), '‹', ...text.slice(from, to), '›', ...text.slice(to)].join('')
}

/** Sets the note's text and selection, and gives the canvas the keyboard. */
async function set(markedText) {
  const { text, anchor, focus } = parse(markedText)
  const now = [...(await run('text.read')).text].length
  await run('text.replace', { from: 0, to: now, text })
  await run('selection.set', { anchor, focus })
  await page.waitForTimeout(60)
}

/** A scenario: from `before`, `act`, then the note reads `after`. */
async function scenario(name, before, act, after) {
  try {
    await set(before)
    await act()
    const ok = await until(async () => (await marked()) === after, 40)
    step(name, ok, ok ? '' : `${JSON.stringify(await marked())}, wanted ${JSON.stringify(after)}`)
  } catch (error) {
    step(name, false, String(error))
  }
}

const press = (...keys) => async () => {
  for (const key of keys) await page.keyboard.press(key)
}
const type = (text) => async () => {
  await page.keyboard.type(text)
}

try {
  await page.goto(`${origin}/dev.html?locale=en-US`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 90000 })
  await page.evaluate(async () => window.devHost.open('Parity.md', new TextEncoder().encode('# Parity\n').buffer))
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 90000 })
  await until(async () => (await canvas().count()) > 0, 300)
  const box = await canvas().boundingBox()
  await page.mouse.click(box.x + box.width / 2, box.y + 60)

  // Lists: Enter continues a list item, an empty one ends the list; Tab and Shift+Tab nest.
  await scenario('Enter continues a bullet', '- one|', press('Enter'), '- one\n- |')
  await scenario('Enter continues a numbered item with the next number', '1. one|', press('Enter'), '1. one\n2. |')
  await scenario('Enter continues a task, unchecked', '- [x] done|', press('Enter'), '- [x] done\n- [ ] |')
  await scenario('Enter continues a quote', '> said|', press('Enter'), '> said\n> |')
  await scenario('Enter on an empty bullet ends the list', '- one\n- |', press('Enter'), '- one\n|')
  await scenario('Tab nests a list item', '- one\n- two|', press('Tab'), '- one\n\t- two|')
  await scenario('Shift+Tab un-nests it', '- one\n\t- two|', press('Shift+Tab'), '- one\n- two|')
  // Measured in Obsidian (tests/compare-editing.mjs): a character at a time.
  await scenario('Backspace after an empty bullet takes its space', '- one\n- |', press('Backspace'), '- one\n-|')
  // In a quote, lists go on after the `>` (measured, as below).
  await scenario('Tab nests an item in a quote after its >', '> - one\n> - two|', press('Tab'), '> - one\n> \t- two|')
  await scenario('Enter on an empty item in a quote keeps the quote', '> - one\n> - |', press('Enter'), '> - one\n> |')
  // Cmd+Left and Home stop at an item's text first.
  await scenario(`${mod}+Left goes to an item's text start`, '- item tex|t', press(`${mod}+ArrowLeft`), '- |item text')
  // A third backtick closes the code block it opens.
  await scenario('``` closes its code block', 'x\n|', type('```'), 'x\n```|\n```')
  // Words go by CodeMirror's groups: a CJK run is one, punctuation another.
  await scenario('Alt+Backspace takes a CJK run', 'abc 中文段落|', press('Alt+Backspace'), 'abc |')
  await scenario('Alt+Left stops at punctuation', 'one, two|', press('Alt+ArrowLeft', 'Alt+ArrowLeft'), 'one|, two')
  // Undo puts back the selection; a Cmd+B joins the word typed before it.
  await scenario(`Undo of ${mod}+B selects the word again`, 'a ‹word› b', press(`${mod}+b`, `${mod}+z`), 'a ‹word› b')
  await scenario(`Undo takes back a word and the ${mod}+B after it`, 'x |', async () => {
    await page.keyboard.type('abc')
    await page.keyboard.press(`${mod}+b`)
    await page.keyboard.press(`${mod}+z`)
  }, 'x |')

  // A change in a numbered list renumbers it (measured in Obsidian).
  await scenario('typing a numbered item under one takes the next number', '1. one\n|', type('5. x'), '1. one\n2. x|')
  await scenario('typing in a list renumbers the items after', '1. o|ne\n3. three', type('x'), '1. ox|ne\n2. three')
  // macOS's Control editing keys.
  if (process.platform === 'darwin') {
    await scenario('Ctrl+A goes to the line start', '- item tex|t', press('Control+a'), '|- item text')
    await scenario('Ctrl+K deletes to the line end', 'one |two\nthree', press('Control+k'), 'one |\nthree')
    await scenario('Ctrl+T swaps the characters', 'ab|c', press('Control+t'), 'acb|')
    await scenario('Ctrl+F moves forward', 'a|bc', press('Control+f'), 'ab|c')
  }
  // Toggle checkbox over several lines, and a bold that takes the bold it overlaps in.
  await scenario(`${mod}+L checks every task unless all are done`, '‹- [x] a\n- [ ] b›', press(`${mod}+l`), '‹- [x] a\n- [x] b›')
  await scenario(`${mod}+B merges the bold it overlaps`, 'a ‹**b** c› d', press(`${mod}+b`), 'a **‹b c›** d')

  // Brackets and quotes pair; a closing one types over; Backspace takes an empty pair.
  await scenario('( pairs', 'a|', type('('), 'a(|)')
  await scenario(') types over the pair\'s closer', 'a(|)', type(')'), 'a()|')
  await scenario('[[ pairs as a wikilink', 'a|', type('[['), 'a[[|]]')
  await scenario('" pairs', 'say |', type('"'), 'say "|"')
  await scenario('Backspace inside an empty pair takes both', 'a(|)', press('Backspace'), 'a|')
  await scenario('Markdown syntax wraps a selection: *', 'a ‹word› b', type('*'), 'a *‹word›* b')
  await scenario('Markdown syntax wraps a selection: =', 'a ‹word› b', type('='), 'a =‹word›= b')

  // Shortcuts as in Obsidian.
  await scenario(`${mod}+B bolds the selection`, 'a ‹word› b', press(`${mod}+b`), 'a **‹word›** b')
  await scenario(`${mod}+B again unbolds it`, 'a **‹word›** b', press(`${mod}+b`), 'a ‹word› b')
  await scenario(`${mod}+I italicizes the selection`, 'a ‹word› b', press(`${mod}+i`), 'a *‹word›* b')
  await scenario(`${mod}+K links the selection, the caret in the address`, 'a ‹word› b', press(`${mod}+k`), 'a [word](|) b')
  await scenario(`${mod}+L toggles the line's checkbox`, '- [ ] task|', press(`${mod}+l`), '- [x] task|')
  await scenario(`${mod}+] indents, ${mod}+[ outdents`, '- one\n- two|', press(`${mod}+BracketRight`), '- one\n\t- two|')
  // Alt+Up and Alt+Down move by paragraphs (macOS), as Obsidian does (measured).
  await scenario('Alt+Up goes to the line start', 'one\ntwo|', press('Alt+ArrowUp'), 'one\n|two')
  await scenario('Alt+Down goes to the next line end', 'one|\ntwo', press('Alt+ArrowDown'), 'one\ntwo|')

  // A paste is an undo step of its own: Undo takes it back and keeps the typing before it.
  if (which === 'chromium') {
    await set('a |')
    await page.keyboard.type('x')
    await page.evaluate(() => navigator.clipboard.writeText('pasted'))
    await page.keyboard.press(`${mod}+v`)
    await until(async () => (await run('text.read')).text === 'a xpasted')
    await page.keyboard.press(`${mod}+z`)
    const kept = await until(async () => (await run('text.read')).text === 'a x')
    step('Undo takes back a paste, not the typing before it', kept, (await run('text.read')).text)
  }

  // Pasting an address over a selection makes a link.
  if (which === 'chromium') {
    await set('see ‹the docs› here')
    await page.evaluate(() => navigator.clipboard.writeText('https://kition.ai/docs'))
    await page.keyboard.press(`${mod}+v`)
    const linked = await until(async () => (await run('text.read')).text === 'see [the docs](https://kition.ai/docs) here')
    step('pasting an address over a selection links it', linked, (await run('text.read')).text)
  }

  // Mouse: a double click selects a word, a triple click the line, Shift+click extends.
  {
    await set('alpha beta gamma|')
    const at = async (index) => {
      const where = await run('selection.where', { at: index })
      const [vx, vy, , , zoom] = (await kit().getByTestId('note-editor').getAttribute('data-view')).split(',').map(Number)
      return { x: box.x + (where.x - vx) * zoom + 0.5, y: box.y + (where.y + where.height / 2 - vy) * zoom }
    }
    const beta = await at(8)
    await page.mouse.dblclick(beta.x, beta.y)
    step('a double click selects the word', await until(async () => (await marked()) === 'alpha ‹beta› gamma'), await marked())
    await page.mouse.click(beta.x, beta.y, { clickCount: 3 })
    step('a triple click selects the line', await until(async () => (await marked()).startsWith('‹alpha beta gamma')), await marked())
    const alpha = await at(1)
    const gamma = await at(12)
    await page.mouse.click(alpha.x, alpha.y)
    await page.keyboard.down('Shift')
    await page.mouse.click(gamma.x, gamma.y)
    await page.keyboard.up('Shift')
    step('Shift+click extends the selection', await until(async () => /^a‹lpha beta g›amma/.test(await marked())), await marked())
    // A selection dragged from inside it moves where it is dropped, selected (CodeMirror's drag).
    await set('alpha ‹beta› gamma')
    const inside = await at(8)
    const end = await at(16)
    await page.mouse.move(inside.x, inside.y)
    await page.mouse.down()
    await page.mouse.move((inside.x + end.x) / 2, inside.y, { steps: 5 })
    await page.mouse.move(end.x, end.y, { steps: 5 })
    await page.mouse.up()
    step('a dragged selection moves where it is dropped', await until(async () => (await marked()) === 'alpha  gamma‹beta›'), await marked())
  }

  // Find opens with the selected text to find (as Obsidian's); Mod+Alt+F finds and replaces;
  // Enter in the replace field replaces the shown match and shows the next.
  {
    await set('alpha ‹beta› alpha beta gamma beta')
    await page.keyboard.press(`${mod}+f`)
    const input = kit().getByTestId('note-find-input')
    step('Find opens with the selected text', await until(async () => (await input.count()) === 1 && (await input.inputValue()) === 'beta'), await input.inputValue().catch(() => ''))
    await page.keyboard.press('Enter')
    step('Enter shows the first match', await until(async () => (await marked()) === 'alpha ‹beta› alpha beta gamma beta'), await marked())
    await page.keyboard.press('Escape')
    await until(async () => (await input.count()) === 0)
    await set('alpha ‹beta› alpha beta gamma beta')
    await page.keyboard.press(`${mod}+Alt+f`)
    const replace = kit().getByTestId('note-replace-input')
    step('Mod+Alt+F opens find and replace', await until(async () => (await replace.count()) === 1))
    await input.press('Enter')
    await replace.fill('B')
    await replace.press('Enter')
    step('Enter in the replace field replaces the shown match', await until(async () => (await run('text.read')).text === 'alpha B alpha beta gamma beta'), (await run('text.read')).text)
    step('and shows the next one', await until(async () => (await marked()) === 'alpha B alpha ‹beta› gamma beta'), await marked())
    await kit().getByTestId('note-replace-all').click()
    step('Replace all replaces the rest', await until(async () => (await run('text.read')).text === 'alpha B alpha B gamma B'), (await run('text.read')).text)
    await kit().getByTestId('note-find-close').click()
    await until(async () => (await input.count()) === 0)
  }

  // The caret scrolls into view the nearest way, 5 px from the edge (measured in Obsidian).
  {
    await set(`${Array.from({ length: 120 }, (_, index) => `line ${index}`).join('\n')}`.replace(/^/, '|'))
    for (let index = 0; index < 50; index += 1) await page.keyboard.press('ArrowDown')
    await page.waitForTimeout(400)
    const gap = async () => {
      const selection = await run('selection.get')
      const [, y, , height] = (await kit().getByTestId('note-editor').getAttribute('data-view')).split(',').map(Number)
      return { below: Math.round(y + height - (selection.caret.y + selection.caret.height)), above: Math.round(selection.caret.y - y) }
    }
    const down = await gap()
    step('Down past the bottom keeps the caret 5 px above it', Math.abs(down.below - 5) <= 2, JSON.stringify(down))
    for (let index = 0; index < 35; index += 1) await page.keyboard.press('ArrowUp')
    await page.waitForTimeout(400)
    const up = await gap()
    step('Up past the top keeps the caret 5 px below it', Math.abs(up.above - 5) <= 2, JSON.stringify(up))
  }

  // A code block shows its language at the top right; under the pointer it copies the code.
  {
    await set('# Code\n\n```rust\nfn x() {}\n```\n\nafter|')
    const flair = kit().getByTestId('note-code-flair')
    step('a code block shows its language', await until(async () => (await flair.count()) === 1 && (await flair.innerText()).trim() === 'Rust'), await flair.innerText().catch(() => ''))
    if (which === 'chromium') {
      await flair.hover()
      await flair.click()
      const copied = await until(async () => (await page.evaluate(() => navigator.clipboard.readText())) === 'fn x() {}')
      step('its button copies the code', copied, await page.evaluate(() => navigator.clipboard.readText()))
    }
  }

  // A callout's code block (rendered, as Obsidian renders a callout's content): no language label,
  // only a copy button while the pointer is over the block.
  {
    await set('# Code\n\n> [!note] Callout\n> ```python\n> print(1)\n> ```\n\nafter|')
    const flair = kit().getByTestId('note-code-flair')
    await page.mouse.move(1, 1)
    step('a callout\'s code block shows no label away from the pointer', await until(async () => (await flair.count()) === 0), String(await flair.count()))
    const at = await run('selection.where', { at: '# Code\n\n> [!note] Callout\n> ```python\n> pri'.length })
    const [vx, vy, , , zoom] = (await kit().getByTestId('note-editor').getAttribute('data-view')).split(',').map(Number)
    await page.mouse.move(box.x + (at.x - vx) * zoom, box.y + (at.y + at.height / 2 - vy) * zoom)
    step('under the pointer it shows the copy button only', await until(async () => (await flair.count()) === 1 && (await flair.innerText()).trim() === '' && (await flair.locator('svg').count()) === 1), String(await flair.count()))
    await page.mouse.move(1, 1)
  }

  // A heading's fold chevron shows under the pointer; a link takes the pointing hand.
  {
    await set('# Heading\n\nSee [[Plan]] here\n\nend|')
    const where = async (index) => {
      const at = await run('selection.where', { at: index })
      const [vx, vy, , , zoom] = (await kit().getByTestId('note-editor').getAttribute('data-view')).split(',').map(Number)
      return { x: box.x + (at.x - vx) * zoom, y: box.y + (at.y + at.height / 2 - vy) * zoom, zoom, at, vx, vy }
    }
    const start = await where(0)
    const heading = await where(4)
    const margin = { x: Math.round(box.x + (start.at.x - 24 - start.vx) * start.zoom), y: Math.round(heading.y - 10) }
    const chevron = () => page.screenshot({ clip: { x: margin.x, y: margin.y, width: 16, height: 20 } }).then((png) => png.toString('base64'))
    await page.mouse.move(box.x + box.width - 30, box.y + box.height - 30)
    await page.waitForTimeout(300)
    const away = await chevron()
    await page.mouse.move(heading.x + 10, heading.y)
    const shown = await until(async () => (await chevron()) !== away)
    step('a heading\'s fold chevron shows under the pointer', shown)
    const link = await where(18)
    await page.mouse.move(link.x + 3, link.y)
    const cursor = () => canvas().evaluate((element) => getComputedStyle(element).cursor)
    step('a link takes the pointing hand', await until(async () => (await cursor()) === 'pointer'), await cursor())
    const plain = await where(25)
    await page.mouse.move(plain.x + 2, plain.y)
    step('plain text takes the text cursor', await until(async () => (await cursor()) === 'text'), await cursor())
  }

  // Right-click: the editor's menu, with Format, Paragraph, and Insert, as Obsidian's.
  {
    await set('make ‹this› bold')
    const at = await run('selection.where', { at: 6 })
    const [vx, vy, , , zoom] = (await kit().getByTestId('note-editor').getAttribute('data-view')).split(',').map(Number)
    await page.mouse.click(box.x + (at.x - vx) * zoom + 2, box.y + (at.y + at.height / 2 - vy) * zoom, { button: 'right' })
    const menu = kit().getByTestId('note-context-menu')
    step('a right click opens the editor menu', await until(async () => (await menu.count()) === 1))
    await kit().getByTestId('note-menu-format').hover()
    await kit().getByTestId('note-menu-format.bold').click()
    step('Format > Bold bolds the selection', await until(async () => (await run('text.read')).text === 'make **this** bold'), (await run('text.read')).text)
    // Its items in Obsidian's order (read over Obsidian's Menu API).
    await set('some ‹text› here')
    await page.mouse.click(box.x + (at.x - vx) * zoom + 2, box.y + (at.y + at.height / 2 - vy) * zoom, { button: 'right' })
    await until(async () => (await menu.count()) === 1)
    const labels = (await menu.locator('.kui-menu-item').allInnerTexts()).map((label) => label.split('\n')[0].trim())
    step('the menu has Obsidian\'s items in its order', ['Add link', 'Add external link', 'Format', 'Paragraph', 'Insert', 'Cut', 'Copy', 'Paste', 'Paste as plain text', 'Select all'].every((label, index, all) => labels.indexOf(label) >= 0 && (index === 0 || labels.indexOf(label) > labels.indexOf(all[index - 1]))), JSON.stringify(labels))
    await kit().getByTestId('note-menu-format').hover()
    await kit().getByTestId('note-menu-format.clear').click()
    step('Format > Clear formatting is there', await until(async () => (await menu.count()) === 0))
    // Format > Highlight > Red writes Obsidian 1.14's coloured highlight.
    await set('some ‹text› here')
    await page.mouse.click(box.x + (at.x - vx) * zoom + 2, box.y + (at.y + at.height / 2 - vy) * zoom, { button: 'right' })
    await until(async () => (await menu.count()) === 1)
    await kit().getByTestId('note-menu-format').hover()
    await kit().getByTestId('note-menu-highlight').hover()
    await kit().getByTestId('note-menu-highlight-red').click()
    step('Highlight > Red highlights in red', await until(async () => (await run('text.read')).text === 'some ==🔴text== here'), (await run('text.read')).text)
    // A right click in a callout drawn as one opens no menu, as in Obsidian.
    await set('> [!note] Title\n> body text\n\nend|')
    const body = await run('selection.where', { at: 22 })
    await page.mouse.click(box.x + (body.x - vx) * zoom + 2, box.y + (body.y + body.height / 2 - vy) * zoom, { button: 'right' })
    await page.waitForTimeout(500)
    step('a right click in a callout opens no menu', (await menu.count()) === 0)
    if (which === 'chromium') {
      await set('copy ‹these words› now')
      await page.mouse.click(box.x + box.width / 2, box.y + 30, { button: 'right' })
      await until(async () => (await menu.count()) === 1)
      await kit().getByTestId('note-menu-copy').click()
      step('Copy copies the selection', await until(async () => (await page.evaluate(() => navigator.clipboard.readText())) === 'these words'))
    }
  }

  // Properties: the front matter as Obsidian's block; a checkbox toggles, × removes a tag, and a
  // click on a value edits it in the block (Obsidian's live preview never shows the YAML).
  {
    await set('---\ntitle: Plan\ntags:\n  - plan\n  - q4\ndone: false\n---\n# Body\n\ntext|')
    const block = kit().getByTestId('note-properties')
    const rows = kit().getByTestId('note-property')
    step('the front matter draws as a properties block', await until(async () => (await block.count()) === 1 && (await rows.count()) === 3), String(await rows.count()))
    await kit().locator('[data-key="done"] input[type=checkbox]').click()
    step('a checkbox property toggles', await until(async () => (await run('text.read')).text.includes('done: true')), (await run('text.read')).text)
    await kit().locator('[data-key="tags"] .note-property-remove').first().click()
    step('× removes a tag', await until(async () => !(await run('text.read')).text.includes('plan') || !/- plan/.test((await run('text.read')).text)), (await run('text.read')).text)
    await kit().locator('[data-key="title"] [data-testid="note-property-text"]').click()
    const editing = await until(async () => (await block.count()) === 1 && (await kit().evaluate(() => document.activeElement?.getAttribute('data-testid'))) === 'note-property-text')
    step('a click on a value edits it in the block', editing, String(await kit().evaluate(() => document.activeElement?.className)))
    // A click on its heading folds the block to it (Obsidian's Toggle fold properties): only the
    // heading's room, nothing written; a second click opens it.
    await page.keyboard.press('Escape')
    const before = (await run('text.read')).text
    const height = async () => {
      const shown = await run('properties.view')
      return shown.bottom - shown.top
    }
    const open = await height()
    await kit().getByTestId('note-properties-heading').click()
    step('the heading folds the properties block to it', await until(async () => (await rows.count()) === 0 && (await height()) < open - 40), `${open} -> ${await height()}`)
    step('folding writes nothing', (await run('text.read')).text === before)
    await kit().getByTestId('note-properties-heading').click()
    step('a second click opens it', await until(async () => (await rows.count()) > 0 && (await height()) === open), String(await height()))
    // Mod+; asks for a new property's key (Obsidian's Add file property).
    await set('---\ntitle: Plan\n---\n# Body\n\ntext|')
    await until(async () => (await block.count()) === 1)
    // The keyboard back in the note (the click above left it in a value's field).
    await kit().getByTestId('kit-text-sink').focus()
    await page.keyboard.press(`${mod}+Semicolon`)
    const asked = await until(async () => (await kit().getByTestId('note-property-new-key').count()) === 1)
    step('Mod+; asks for a new property', asked)
    if (asked) {
      await kit().getByTestId('note-property-new-key').fill('status')
      await kit().getByTestId('note-property-new-key').press('Enter')
      step('the new property is added', await until(async () => /^---\ntitle: Plan\nstatus:/m.test((await run('text.read')).text)), (await run('text.read')).text)
    }
    // With no front matter, Mod+; shows an empty properties block; a key named writes it.
    await set('# Body\n\ntext|')
    await kit().getByTestId('kit-text-sink').focus()
    await page.keyboard.press(`${mod}+Semicolon`)
    const first = await until(async () => (await kit().getByTestId('note-property-new-key').count()) === 1)
    step('Mod+; with no front matter asks for a property', first)
    if (first) {
      await kit().getByTestId('note-property-new-key').fill('status')
      await kit().getByTestId('note-property-new-key').press('Enter')
      step('the front matter is written with it', await until(async () => (await run('text.read')).text === '---\nstatus:\n---\n# Body\n\ntext'), JSON.stringify((await run('text.read')).text))
    }
  }

  // A web address value is an external link; cssclasses is a list even with one value (Obsidian).
  {
    await set('---\nurl: https://example.com\ncssclasses: wide\n---\n# Body\n\ntext|')
    const url = kit().locator('[data-key="url"] .note-property-url')
    step('a web address property shows as an external link', await until(async () => (await url.count()) === 1 && (await url.locator('svg').count()) === 1), String(await url.count()))
    const pills = kit().locator('[data-key="cssclasses"] .note-property-pill')
    step('cssclasses shows its value as a pill', await until(async () => (await pills.count()) === 1 && (await pills.first().innerText()).includes('wide')), String(await pills.count()))
    // The link's edit button shows under the pointer over its cell, not over the link.
    const flair = kit().locator('[data-key="url"] [data-testid="note-property-flair"]')
    const cell = await kit().locator('[data-key="url"] .note-property-value').boundingBox()
    await page.mouse.move(cell.x + cell.width - 40, cell.y + cell.height / 2)
    step('a link value shows its edit button under the pointer', await until(async () => Number(await flair.evaluate((element) => getComputedStyle(element).opacity)) > 0.5))
    await flair.click()
    step('the edit button edits the value', await until(async () => (await kit().evaluate(() => document.activeElement?.getAttribute('data-testid'))) === 'note-property-text'))
    await page.mouse.move(1, 1)
  }

  // A gantt chart fills the column it is drawn in (Obsidian's 700 px at 16 px text), its text
  // at its own size, not drawn for the page and shrunk.
  {
    await set('intro|\n\n```mermaid\ngantt\n  title Plan\n  section A\n  Task :a1, 2024-01-01, 3d\n```\n\nend\n')
    const svg = kit().locator('[data-testid="note-diagram"] .note-diagram-svg > svg')
    const width = async () => Number((await svg.getAttribute('viewBox'))?.split(' ')[2] ?? 0)
    step('a gantt chart is drawn as wide as the column', await until(async () => (await svg.count()) === 1 && Math.abs((await width()) - 700) < 2, 120), String(await width().catch(() => 0)))
    // A narrower column draws it again for its width.
    await page.setViewportSize({ width: 640, height: 900 })
    step('a narrower column draws the chart again for it', await until(async () => (await svg.count()) === 1 && (await width()) < 640 && (await width()) > 300, 120), String(await width().catch(() => 0)))
    await page.setViewportSize({ width: 1280, height: 900 })
    await until(async () => Math.abs((await width()) - 700) < 2, 120)
  }

  // Mermaid: a block away from the caret draws as its diagram (Obsidian's Mermaid and settings);
  // its source takes no room; a click on it leaves the caret; its edit button puts the caret at
  // its code; leaving draws the edited diagram; the wheel over it scrolls the note; a block that
  // does not parse shows Mermaid's error.
  {
    const filler = Array.from({ length: 40 }, (_, index) => `line ${index}`).join('\n\n')
    await set(`intro|\n\n\`\`\`mermaid\ngraph TD\n  A[Start] --> B[End]\n\`\`\`\n\n${filler}\n`)
    const diagram = kit().getByTestId('note-diagram')
    const drawn = async (label) => (await diagram.count()) === 1 && (await diagram.locator('.note-diagram-svg > svg').textContent()).includes(label)
    step('a mermaid block away from the caret draws its diagram', await until(() => drawn('Start'), 120), String(await diagram.count()))
    const code = [...'intro\n\n```mermaid\n'].length
    step('its source takes no room', (await run('selection.where', { at: code })).height === 0)
    const shape = await diagram.boundingBox()
    await page.mouse.click(shape.x + 40, shape.y + 40)
    step('a click on the diagram leaves the caret', (await run('selection.get')).anchor === 5 && (await diagram.count()) === 1, JSON.stringify(await run('selection.get')))
    await page.mouse.move(shape.x + 200, shape.y + 40)
    await kit().getByTestId('note-diagram-edit').click()
    step('its edit button puts the caret at its code', await until(async () => (await run('selection.get')).anchor === code && (await diagram.count()) === 0), JSON.stringify(await run('selection.get')))
    await page.keyboard.type('%% a comment\n')
    step('the editor keeps the keyboard', await until(async () => (await run('text.read')).text.includes('```mermaid\n%% a comment\ngraph TD')), (await run('text.read')).text.slice(0, 60))
    const text = (await run('text.read')).text
    const at = [...text.slice(0, text.indexOf('End'))].length
    await run('text.replace', { from: at, to: at + 3, text: 'Finish' })
    await run('selection.set', { anchor: 0 })
    step('away from the caret, the edited diagram draws', await until(() => drawn('Finish'), 120))
    const editor = kit().getByTestId('note-editor')
    const y = async () => Number((await editor.getAttribute('data-view')).split(',')[1])
    const back = await diagram.boundingBox()
    await page.mouse.move(back.x + 40, back.y + 40)
    await page.mouse.wheel(0, 400)
    step('the wheel over a diagram scrolls the note', await until(async () => (await y()) > 0), String(await y()))
    await set('a|\n\n```mermaid\ngraph TD\n  A[Start --> \n```\n')
    step('a diagram that does not parse shows its error', await until(async () => (await diagram.count()) === 1 && (await diagram.getAttribute('data-error')) === 'true' && (await diagram.textContent()).includes('Error parsing Mermaid diagram!'), 120))
  }

  // HTML blocks draw as their HTML, as Obsidian's: `<details>` folds and opening it makes room;
  // scripts, handlers, and remote pictures are dropped (the kit loads nothing from the network).
  {
    await set('top|\n\n<details>\n<summary>More</summary>\n\nHidden body.\n\n</details>\n\n<div onclick="alert(1)"><script>window.hacked = 1</script><img src="https://example.com/x.png" alt="x">Safe text</div>\n\nend\n')
    const blocks = kit().locator('[data-testid="note-diagram"][data-kind="html"]')
    step('HTML blocks draw as their HTML', await until(async () => (await blocks.count()) === 2, 120), String(await blocks.count()))
    const details = blocks.first()
    step('a details block shows its summary, folded', (await details.locator('summary').textContent()) === 'More' && !(await details.locator('details').evaluate((element) => element.open)))
    const height = async () => (await details.boundingBox())?.height ?? 0
    const folded = await height()
    await details.locator('summary').click()
    step('opening it makes room for its body', await until(async () => (await height()) > folded + 10), `${folded} -> ${await height()}`)
    const other = blocks.nth(1)
    const html = await other.innerHTML()
    step('scripts, handlers, and remote pictures are dropped', html.includes('Safe text') && !html.includes('<script') && !html.includes('onclick') && !html.includes('example.com'), html.slice(0, 200))
    step('nothing ran', !(await kit().locator('body').evaluate(() => 'hacked' in window)))
    // An HTML table is drawn as Obsidian draws it: bordered cells, padded 4px 8px.
    await set('top|\n\n<table><tr><td>HTML</td><td>table</td></tr></table>\n\nend\n')
    const cell = kit().locator('[data-testid="note-diagram"][data-kind="html"] td').first()
    await until(async () => (await cell.count()) === 1, 120)
    const look = await cell.evaluate((element) => [getComputedStyle(element).borderTopWidth, getComputedStyle(element).paddingLeft, getComputedStyle(element).paddingTop].join(' ')).catch(() => '')
    step('an HTML table has bordered, padded cells', look === '1px 8px 4px', look)
  }

  // Pasting from a web page: its HTML goes in as Markdown, converted as Obsidian converts it.
  if (which === 'chromium') {
    await set('|')
    await page.mouse.click(box.x + box.width / 2, box.y + 30)
    await run('selection.set', { anchor: 0 })
    const html = '<h2>Title</h2><p>Hello <b>bold</b>, <i>italic</i>, <a href="https://example.com/page">link</a>.</p><ol start="3"><li>three</li></ol><table><tr><th>A</th><th>B</th></tr><tr><td>1</td><td>2</td></tr></table>'
    await page.evaluate(async (html) => navigator.clipboard.write([new ClipboardItem({ 'text/html': new Blob([html], { type: 'text/html' }), 'text/plain': new Blob(['Title'], { type: 'text/plain' }) })]), html)
    await page.keyboard.press(`${mod}+v`)
    const wanted = '## Title\n\nHello **bold**, _italic_, [link](https://example.com/page).\n\n3. three\n\n|A|B|\n|---|---|\n|1|2|'
    step('HTML pastes as Markdown, as Obsidian converts it', await until(async () => (await run('text.read')).text === wanted), JSON.stringify((await run('text.read')).text))
    await page.keyboard.press(`${mod}+z`)
    step('one Undo takes the paste back', await until(async () => (await run('text.read')).text === ''), JSON.stringify((await run('text.read')).text))
  }

  // The caret blinks while the editor has the keyboard, and goes when it loses it.
  {
    await set('blink here|')
    await page.mouse.click(box.x + box.width / 2, box.y + 30)
    await run('selection.set', { anchor: 10 })
    const caret = kit().getByTestId('note-caret')
    step('a caret is drawn while the editor has the keyboard', await until(async () => (await caret.count()) === 1))
    const seen = new Set()
    for (let i = 0; i < 24; i += 1) {
      seen.add(await caret.evaluate((element) => getComputedStyle(element).opacity).catch(() => 'gone'))
      await page.waitForTimeout(100)
    }
    step('the caret blinks', seen.has('1') && seen.has('0'), [...seen].join(','))
    await kit().locator('body').evaluate(() => document.activeElement instanceof HTMLElement && document.activeElement.blur())
    step('the caret goes when the editor loses the keyboard', await until(async () => (await caret.count()) === 0))
  }

  // Scrolled away from the caret, the note stays where it was scrolled: an answer that leaves the
  // caret where it was (a hover) does not pull it back to the caret; and lines scrolled into view
  // are painted while the note scrolls, not only once it stops.
  {
    const long = Array.from({ length: 400 }, (_, i) => `Line ${i} of a long note, with **bold** and a [link](https://example.com).`).join('\n\n')
    await set(`|${long}`)
    await page.waitForTimeout(300)
    const viewY = async () => Number((await kit().getByTestId('note-editor').getAttribute('data-view')).split(',')[1])
    const bar = await kit().getByTestId('note-scrollbar').boundingBox()
    const thumb = await kit().locator('.note-scrollbar-thumb').boundingBox()
    await page.mouse.move(thumb.x + thumb.width / 2, thumb.y + thumb.height / 2)
    await page.mouse.down()
    for (let i = 1; i <= 10; i += 1) await page.mouse.move(thumb.x + thumb.width / 2, thumb.y + ((bar.y + bar.height + 40 - thumb.y) * i) / 10)
    await page.mouse.up()
    // The canvas shows the end once its render lands (it keeps the painted view until then).
    let bottom = await viewY()
    await until(async () => {
      await page.waitForTimeout(150)
      const now = await viewY()
      const settled = now === bottom
      bottom = now
      return settled
    })
    // Hovers over the note (links, bold) answer without moving the caret.
    for (let i = 0; i < 12; i += 1) await page.mouse.move(box.x + 200 + i * 40, box.y + 100 + i * 30)
    await page.waitForTimeout(400)
    step('a scroll to the end stays there when the pointer moves over the note', bottom > 10000 && (await viewY()) === bottom, `${bottom} then ${await viewY()}`)
    // A wheel scroll paints the lines it brings into view without waiting for the scrolling to stop.
    // The tallest run of blank rows in the note's column, as a share of the view.
    const blankBand = async () => kit().evaluate(() => {
      const c = document.querySelector('[data-testid="note-canvas"] canvas')
      const x0 = Math.round(c.width * 0.3)
      const w = Math.round(c.width * 0.4)
      const data = c.getContext('2d').getImageData(x0, 0, w, c.height).data
      let run = 0
      let longest = 0
      for (let y = 0; y < c.height; y += 1) {
        let blank = true
        for (let x = 0; x < w * 4; x += 16) if (data[y * w * 4 + x] < 200) { blank = false; break }
        run = blank ? run + 1 : 0
        longest = Math.max(longest, run)
      }
      return longest / c.height
    })
    // Back to the top with the bar, then down with the wheel.
    const end = await kit().locator('.note-scrollbar-thumb').boundingBox()
    await page.mouse.move(end.x + end.width / 2, end.y + end.height / 2)
    await page.mouse.down()
    await page.mouse.move(end.x + end.width / 2, bar.y - 40, { steps: 5 })
    await page.mouse.up()
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
    await page.waitForTimeout(300)
    let blankWhileScrolling = 0
    const bands = []
    for (let i = 0; i < 10; i += 1) {
      await page.mouse.wheel(0, 500)
      await page.waitForTimeout(70)
      const band = await blankBand()
      bands.push(band.toFixed(2))
      if (band > 0.4) blankWhileScrolling += 1
    }
    step('lines come into view painted while the note scrolls', blankWhileScrolling === 0, `${blankWhileScrolling} mostly blank of 10 (${bands.join(' ')})`)
    // A drag of the scroll bar (many screens a step) never flashes blank canvas: the canvas keeps
    // the painted view until the new one is painted.
    {
      const top = await kit().locator('.note-scrollbar-thumb').boundingBox()
      await page.mouse.move(top.x + top.width / 2, top.y + top.height / 2)
      await page.mouse.down()
      await page.mouse.move(top.x + top.width / 2, bar.y - 40, { steps: 4 })
      await page.waitForTimeout(300)
      await kit().evaluate(() => {
        window.__bands = []
        const c = document.querySelector('[data-testid="note-canvas"] canvas')
        const tick = () => {
          const x0 = Math.round(c.width * 0.3)
          const w = Math.round(c.width * 0.4)
          const data = c.getContext('2d').getImageData(x0, 0, w, c.height).data
          let run = 0
          let longest = 0
          for (let y = 0; y < c.height; y += 2) {
            let blank = true
            for (let x = 0; x < w * 4; x += 32) if (data[y * w * 4 + x] < 200) { blank = false; break }
            run = blank ? run + 2 : 0
            longest = Math.max(longest, run)
          }
          window.__bands.push(longest / c.height)
          if (!window.__stopBands) requestAnimationFrame(tick)
        }
        requestAnimationFrame(tick)
      })
      const thumb = await kit().locator('.note-scrollbar-thumb').boundingBox()
      // Eight steps: each a few screens, past what the canvas paints ahead of the view.
      for (let i = 1; i <= 8; i += 1) {
        await page.mouse.move(thumb.x + thumb.width / 2, thumb.y + thumb.height / 2 + ((bar.height - thumb.height) * 0.8 * i) / 8)
        await page.waitForTimeout(40)
      }
      await page.mouse.up()
      const bands = await kit().evaluate(() => {
        window.__stopBands = true
        return window.__bands
      })
      const blank = bands.filter((band) => band > 0.4).length
      step('a scroll-bar drag never flashes blank canvas', blank === 0, `${blank} of ${bands.length} frames mostly blank`)
    }
  }

  // A table's columns and body rows move by their drag handles, as in Obsidian 1.14: the handle
  // shows under the pointer, a press outlines the column or row, a bar marks where it goes, release
  // moves it (one undo step), Escape cancels.
  {
    const table = '| A | B | C |\n| --- | --- | --- |\n| 1 | 2 | 3 |\n| 4 | 5 | 6 |\n'
    // Set directly: `|` would be read as the caret.
    const doc = `Intro\n\n${table}\nAfter\n`
    await run('text.replace', { from: 0, to: [...(await run('text.read')).text].length, text: doc })
    await run('selection.set', { anchor: [...doc].length - 1 })
    const ready = await until(async () => (await kit().getByTestId('note-table-col-handle').count()) === 3)
    step('a table shows a drag handle over each column', ready)
    const handles = kit().getByTestId('note-table-col-handle')
    const first = await handles.nth(0).boundingBox()
    const third = await handles.nth(2).boundingBox()
    await page.mouse.move(first.x + first.width / 2, first.y + first.height / 2)
    const shown = await handles.nth(0).evaluate((element) => getComputedStyle(element).opacity)
    await page.mouse.down()
    const outlined = await kit().getByTestId('note-table-selection').count()
    await page.mouse.move(third.x + third.width - 4, third.y + third.height / 2, { steps: 6 })
    const bar = await kit().getByTestId('note-table-drop').count()
    await page.mouse.up()
    const moved = await until(async () => (await run('text.read')).text.includes('| B | C | A |'))
    step('dragging a column handle moves the column', shown === '1' && outlined === 1 && bar === 1 && moved, `${shown} ${outlined} ${bar} ${JSON.stringify((await run('text.read')).text)}`)
    // A body row: the first dragged below the second.
    await until(async () => (await kit().getByTestId('note-table-row-handle').count()) === 2)
    const rows = kit().getByTestId('note-table-row-handle')
    const one = await rows.nth(0).boundingBox()
    const two = await rows.nth(1).boundingBox()
    await page.mouse.move(one.x + one.width / 2, one.y + one.height / 2)
    await page.mouse.down()
    await page.mouse.move(two.x + two.width / 2, two.y + two.height - 2, { steps: 6 })
    await page.mouse.up()
    const rowMoved = await until(async () => /\| 5 \| 6 \| 4 \|\n\| 2 \| 3 \| 1 \|/.test((await run('text.read')).text))
    step('dragging a row handle moves the row', rowMoved, JSON.stringify((await run('text.read')).text))
    // Escape cancels: nothing moves.
    const before = (await run('text.read')).text
    const again = await kit().getByTestId('note-table-col-handle').nth(0).boundingBox()
    await page.mouse.move(again.x + again.width / 2, again.y + again.height / 2)
    await page.mouse.down()
    await page.mouse.move(third.x + third.width - 4, third.y + third.height / 2, { steps: 4 })
    await page.keyboard.press('Escape')
    await page.mouse.up()
    await page.waitForTimeout(200)
    step('Escape cancels a column drag', (await run('text.read')).text === before && (await kit().getByTestId('note-table-drop').count()) === 0, JSON.stringify((await run('text.read')).text))
  }

  step('no page errors', errors.length === 0, errors.join(' | '))
} finally {
  await browser.close()
  await server.close()
}
process.exit(failures ? 1 : 0)
