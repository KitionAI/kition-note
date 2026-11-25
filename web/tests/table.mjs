// The GFM table on the engine's canvas (the grid the engine draws, the controls the editor puts over
// it), in a real browser through the development host: node tests/table.mjs [chromium|webkit].
// The scenarios of Kition's table gate: the append buttons on the hovered edge rail, row and column
// appends, the column menu (vertical, warning color, keys, closing), alignment,
// cell editing, Tab between cells, and ArrowDown in a wrapped cell. Every structural action is
// checked in the saved file, which changes only in the lines the action is about.
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'
import { canvasReady, focusCanvas, primaryKey } from './canvasHelpers.mjs'

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

const kit = () => page.frames().find((frame) => frame !== page.mainFrame())
const run = (command, params = {}) => page.evaluate(async ([command, params]) => (await window.devHost.connection().ai.run([{ command, params }], false)).completed[0].result, [command, params])
const engineText = async () => (await run('text.read')).text
let current = ''
async function open(name, text) {
  current = name
  await page.evaluate(async ([name, text]) => window.devHost.open(name, new TextEncoder().encode(text).buffer), [name, text])
  await until(async () => (await canvasReady(page)) && (await engineText()) === text && (await kit().getByTestId('note-table').count()) > 0, 300)
}
async function save() {
  const before = await page.evaluate((name) => window.devHost.files.get(name).version, current)
  await page.evaluate(() => window.devHost.connection().save())
  await until(async () => (await page.evaluate((name) => window.devHost.files.get(name).version, current)) > before)
  return page.evaluate((name) => new TextDecoder().decode(new Uint8Array(window.devHost.files.get(name).bytes)), current)
}
/** Lines removed and added, counted with their repeats (two empty rows are two lines). */
function lineDiff(before, after) {
  const count = (lines) => lines.reduce((map, line) => map.set(line, (map.get(line) ?? 0) + 1), new Map())
  const a = count(before.split('\n'))
  const b = count(after.split('\n'))
  const removed = []
  const added = []
  for (const [line, n] of a) for (let i = 0; i < n - (b.get(line) ?? 0); i += 1) removed.push(line)
  for (const [line, n] of b) for (let i = 0; i < n - (a.get(line) ?? 0); i += 1) added.push(line)
  return { removed, added }
}
const table = () => kit().getByTestId('note-table').first()
const columns = async () => Number(await table().getAttribute('data-columns'))
const rows = async () => Number(await table().getAttribute('data-rows'))
/** A document point of the table grid in page coordinates. */
async function at(x, y) {
  const [vx, vy, , , zoom] = (await kit().getByTestId('note-editor').getAttribute('data-view')).split(',').map(Number)
  const box = await kit().locator('[data-testid="note-canvas"] canvas').first().boundingBox()
  return { x: box.x + (x - vx) * zoom, y: box.y + (y - vy) * zoom }
}
/** The center of cell (`row`, `column`), row 0 the header. */
async function cell(row, column) {
  const grid = (await run('table.geometry')).tables[0]
  const left = grid.columns[column]
  const right = grid.columns[column + 1]
  return at((left + right) / 2, (grid.rows[row].top + grid.rows[row].bottom) / 2)
}
const opacity = (locator) => locator.evaluate((element) => getComputedStyle(element).opacity)

try {
  await page.goto(`${origin}/dev.html?locale=en-US`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 90000 })

  // The append controls show only on the hovered edge rail.
  await open('Rails.md', ['# Test', '', '| Col 1 | Col 2 | Col 3 |', '|---|---|---|', '| a | b | c |', ''].join('\n'))
  step('the table is drawn as a grid', (await columns()) === 3 && (await rows()) === 2, `${await columns()}x${await rows()}`)
  const rowButton = kit().getByTestId('note-table-row-add').first()
  const colButton = kit().getByTestId('note-table-col-add').first()
  const inside = await cell(1, 0)
  await page.mouse.move(inside.x, inside.y)
  step('no append control over the table body', (await opacity(rowButton)) === '0' && (await opacity(colButton)) === '0')
  await kit().getByTestId('note-table-col-rail').first().hover()
  step('the right rail shows only the column control', (await until(async () => (await opacity(colButton)) === '1', 20)) && (await opacity(rowButton)) === '0')
  await kit().getByTestId('note-table-row-rail').first().hover()
  step('the bottom rail shows only the row control', (await until(async () => (await opacity(rowButton)) === '1', 20)) && (await opacity(colButton)) === '0')

  // appendRow on an all-empty body row adds one row.
  {
    const original = ['# Test', '', '| Col 1 | Col 2 | Col 3 |', '|---|---|---|', '|  |  |  |', ''].join('\n')
    await open('Empty.md', original)
    await kit().getByTestId('note-table-row-rail').first().hover()
    await rowButton.click()
    step('appendRow turns an all-empty body row into two rows', await until(async () => (await rows()) === 3), String(await rows()))
    const diff = lineDiff(original, await save())
    step('the saved file gains one table row only', diff.removed.length === 0 && diff.added.length === 1 && /^\|.*\|$/.test(diff.added[0]), JSON.stringify(diff))
  }

  // appendCol adds one cell to every line of the table.
  {
    const original = ['# Test', '', '| Col 1 | Col 2 | Col 3 |', '|---|---|---|', '| a | b | c |', '| d | e | f |', ''].join('\n')
    await open('Columns.md', original)
    await kit().getByTestId('note-table-col-rail').first().hover()
    await colButton.click()
    step('appendCol adds a column', await until(async () => (await columns()) === 4), String(await columns()))
    const saved = await save()
    const tableLines = original.split('\n').filter((line) => line.startsWith('|'))
    const savedLines = saved.split('\n').filter((line) => line.startsWith('|'))
    step('every table line gains one cell, nothing else changes', savedLines.length === tableLines.length && savedLines.every((line, index) => line.startsWith(tableLines[index]) && line.split('|').length === tableLines[index].split('|').length + 1) && saved.startsWith('# Test\n\n'), JSON.stringify(savedLines))
    await focusCanvas(page)
    await page.keyboard.press(`${mod}+z`)
    step('one Undo takes the column back', await until(async () => (await engineText()) === original), JSON.stringify(await engineText()))
  }

  // appendRow keeps the body text and appends an empty row.
  {
    const original = ['# Test', '', '| A | B |', '|---|---|', '| x1 | y1 |', ''].join('\n')
    await open('Keep.md', original)
    await kit().getByTestId('note-table-row-rail').first().hover()
    await rowButton.click()
    await until(async () => (await rows()) === 3)
    const diff = lineDiff(original, await save())
    step('appendRow keeps the body text and adds an empty row', diff.removed.length === 0 && diff.added.length === 1 && diff.added[0].replace(/\s/g, '') === '|||', JSON.stringify(diff))
  }

  // Kition's gate: append a row, click a body cell found from the table's data-column-edges and
  // data-row-edges (as a host's test does), and one Undo takes the append back.
  {
    const original = '| A | B | C |\n|---|---|---|\n| 1 | 2 | 3 |\n| x | y | z |\n'
    await open('Undo.md', original)
    await kit().getByTestId('note-table-row-rail').first().hover()
    await rowButton.click()
    step('the append adds a fourth row', await until(async () => (await rows()) === 4), String(await rows()))
    const box = await table().boundingBox()
    const edges = (await table().getAttribute('data-column-edges')).split(',').map(Number)
    const tops = (await table().getAttribute('data-row-edges')).split(',').map(Number)
    const expected = await cell(1, 0)
    const clicked = { x: box.x + (edges[0] + edges[1]) / 2, y: box.y + (tops[1] + tops[2]) / 2 }
    step('the data edges find the cell the grid has', Math.abs(clicked.x - expected.x) < 2 && Math.abs(clicked.y - expected.y) < 2, `${JSON.stringify(clicked)} vs ${JSON.stringify(expected)}`)
    await page.mouse.click(clicked.x, clicked.y)
    await page.keyboard.press(`${mod}+z`)
    step('one Undo in the idle cell takes the append back', await until(async () => (await rows()) === 3), String(await rows()))
    // As the gate reads it: the accessible text, once the row count shows the Undo.
    const shown = await kit().getByTestId('note-text').textContent()
    step('the accessible text agrees with the row count', shown === original && (await engineText()) === original, JSON.stringify(shown))
  }

  // The column menu: a right click on a header cell; vertical; the warning color; keys; outside click.
  {
    const original = '| A | B | C |\n|---|---|---|\n| 1 | 2 | 3 |\n'
    await open('Menu.md', original)
    const header = await cell(0, 1)
    await page.mouse.click(header.x, header.y, { button: 'right' })
    const menu = kit().getByTestId('note-table-menu')
    step('a right click on a header cell opens the column menu', await until(async () => (await menu.count()) === 1))
    const items = kit().getByTestId('note-table-menu-item')
    const boxes = []
    for (let i = 0; i < (await items.count()); i += 1) boxes.push(await items.nth(i).boundingBox())
    step('the menu is laid out vertically', boxes.length > 3 && boxes.every((box, i) => i === 0 || box.y >= boxes[i - 1].y + boxes[i - 1].height - 2))
    step('Delete column shows the warning color', /is-warning/.test((await items.filter({ hasText: 'Delete column' }).getAttribute('class')) ?? ''))
    step('the first item is selected', /is-selected/.test((await items.nth(0).getAttribute('class')) ?? ''))
    await page.keyboard.press('ArrowDown')
    step('ArrowDown selects the next item', /is-selected/.test((await items.nth(1).getAttribute('class')) ?? ''))
    await page.keyboard.press('Enter')
    step('Enter runs it (Insert column right) and closes the menu', (await until(async () => (await columns()) === 4)) && (await menu.count()) === 0)
    const saved = await save()
    const lines = saved.split('\n')
    step('every table line gains one cell; nothing else changes', lines.slice(0, 3).every((line) => line.split('|').length === 6), JSON.stringify(lines))
    await page.mouse.click(header.x, header.y, { button: 'right' })
    await until(async () => (await menu.count()) === 1)
    await page.mouse.click(5, 5)
    step('a click outside the frame closes the menu', await until(async () => (await menu.count()) === 0))
    await page.mouse.click(header.x, header.y, { button: 'right' })
    await until(async () => (await menu.count()) === 1)
    const body = await cell(1, 0)
    await page.mouse.click(body.x, body.y)
    step('a click elsewhere in the note closes the menu', await until(async () => (await menu.count()) === 0))
  }

  // A body row's menu: the row's items first; Insert row below and Delete row change one line each.
  {
    const original = '| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n'
    await open('Rows.md', original)
    const body = await cell(1, 0)
    await page.mouse.click(body.x, body.y, { button: 'right' })
    const items = kit().getByTestId('note-table-menu-item')
    await until(async () => (await items.count()) > 0)
    step('a body row opens a menu with the row first', (await items.nth(0).innerText()) === 'Insert row above' && (await items.filter({ hasText: 'Delete row' }).count()) === 1 && (await items.filter({ hasText: 'Delete column' }).count()) === 1)
    await items.filter({ hasText: 'Insert row below' }).dispatchEvent('mousedown')
    step('Insert row below adds one row under it', await until(async () => (await engineText()).split('\n')[3].replace(/\s/g, '') === '|||'), JSON.stringify(await engineText()))
    const again = await cell(1, 0)
    await page.mouse.click(again.x, again.y, { button: 'right' })
    await until(async () => (await items.count()) > 0)
    await items.filter({ hasText: 'Delete row' }).dispatchEvent('mousedown')
    step('Delete row takes that row out', await until(async () => !(await engineText()).includes('| 1 | 2 |') && (await engineText()).includes('| 3 | 4 |')), JSON.stringify(await engineText()))
  }

  // Align center: the delimiter row says so; the cells' text is unchanged.
  {
    const original = '| A | B | C |\n|---|---|---|\n| 1 | 2 | 3 |\n| x | y | z |\n'
    await open('Align.md', original)
    const header = await cell(0, 1)
    await page.mouse.click(header.x, header.y, { button: 'right' })
    await until(async () => (await kit().getByTestId('note-table-menu').count()) === 1)
    await kit().getByTestId('note-table-menu-item').filter({ hasText: 'Align center' }).dispatchEvent('mousedown')
    const aligned = await until(async () => (await engineText()).split('\n')[1].split('|')[2].trim().startsWith(':') && (await engineText()).split('\n')[1].split('|')[2].trim().endsWith(':'))
    step('Align center marks the column centered', aligned, JSON.stringify(await engineText()))
    const values = (text) => text.split('\n').filter((_, index) => index !== 1).map((line) => line.split('|').map((part) => part.trim()))
    step('the cells keep their text', JSON.stringify(values(await engineText())) === JSON.stringify(values(original)))
  }

  // Cells: a click edits one, Tab goes to the next, and only that line changes.
  {
    const original = '| A | B |\n|---|---|\n| one | two |\n'
    await open('Cells.md', original)
    const first = await cell(1, 0)
    await page.mouse.click(first.x, first.y)
    await page.keyboard.press('End')
    await page.keyboard.type('!')
    // Tab selects the next cell's text (as Obsidian's table editor); End goes to its end.
    await page.keyboard.press('Tab')
    await page.keyboard.press('End')
    await page.keyboard.type('?')
    const typed = await until(async () => (await engineText()) === '| A | B |\n|---|---|\n| one! | two? |\n')
    step('typing in a cell and Tab to the next change only their text', typed, JSON.stringify(await engineText()))
  }

  // ArrowDown on a wrapped cell's first line stays in the cell.
  {
    await open('Wrap.md', `| A | B |\n|---|---|\n| ${'word '.repeat(60).trim()} | x |\n| next | y |\n`)
    const first = await cell(1, 0)
    await page.mouse.click(first.x, first.y)
    await page.keyboard.press('Home')
    const before = await run('select.get')
    await page.keyboard.press('ArrowDown')
    const after = await run('select.get')
    step('ArrowDown in a wrapped cell stays in it', after.line === before.line && after.focus > before.focus, `${JSON.stringify(before)} -> ${JSON.stringify(after)}`)
  }

  // Kition's gate: a cell whose word is wider than the column wraps it in the cell, and ArrowDown
  // from the cell's start goes to its second row, not to the next table row.
  {
    await open('Long.md', '| A | B |\n|---|---|\n| line1 | x |\n| next | y |\n')
    const first = await cell(1, 0)
    await page.mouse.click(first.x, first.y)
    await page.keyboard.press('End')
    await page.keyboard.type(' ' + 'a'.repeat(300))
    await until(async () => (await engineText()).includes('a'.repeat(300)))
    const grid = (await run('table.geometry')).tables[0]
    const width = Number((await kit().getByTestId('note-editor').getAttribute('data-view')).split(',')[2])
    step('a word wider than the column keeps the table in the column', grid.columns.at(-1) - grid.columns[0] <= 701, JSON.stringify(grid.columns))
    await page.keyboard.press('Home')
    await page.keyboard.press('ArrowDown')
    await page.keyboard.type('Z')
    const typed = await until(async () => /^\| line1 a*Za* \| x \|$/.test((await engineText()).split('\n')[2]))
    step('ArrowDown from a long word cell\'s start stays in the cell', typed, `${width} ${JSON.stringify((await engineText()).split('\n')[2].slice(0, 40))}`)
  }
  step('no page errors', errors.length === 0, errors.join(' | '))
} catch (error) {
  step('runs without throwing', false, String(error?.stack ?? error))
} finally {
  await browser.close()
  await server.close()
}
console.log(failures ? `${failures} step(s) failed in ${which}` : `all steps passed in ${which}`)
process.exit(failures ? 1 : 0)
