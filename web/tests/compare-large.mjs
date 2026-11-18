// A large note's speed against Obsidian's: the same 1 MB note (OBSIDIAN_VAULT/Large.md)
// opened in a running Obsidian and in the note editor; times opening, a keystroke in the middle
// (until the line holds it), Enter, Undo, and a scroll, in milliseconds (medians).
//
//   npm run build && OBSIDIAN_WS=ws://127.0.0.1:9333/devtools/browser/… OBSIDIAN_VAULT=<vault> node tests/compare-large.mjs
import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { createServer } from 'node:http'
import { chromium } from 'playwright'

// NOTE (default Large.md) and the heading AT (default `## Section 1000`) the keystrokes go after.
const NOTE = process.env.NOTE ?? 'Large.md'
const HEADING = process.env.AT ?? '## Section 1000'
const NAME = HEADING.replace(/^#+ /, '')
const text = await readFile(path.join(process.env.OBSIDIAN_VAULT, NOTE), 'utf8')
const middle = text.indexOf(HEADING)
const median = (values) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)]
const until = async (check, limit = 20000) => {
  const start = Date.now()
  while (Date.now() - start < limit) {
    if (await check()) return Date.now() - start
  }
  return Infinity
}

async function measure(name, page, { line, setCaret, open, scroll }) {
  const opened = await open()
  await setCaret()
  const keys = []
  for (const c of 'abcdefghij') {
    const t = Date.now()
    await page.keyboard.press(c)
    await until(async () => (await line()).includes(`${NAME}${'abcdefghij'.slice(0, 'abcdefghij'.indexOf(c) + 1)}`))
    keys.push(Date.now() - t)
  }
  const enters = []
  for (let i = 0; i < 5; i += 1) {
    const t = Date.now()
    await page.keyboard.press('Enter')
    await until(async () => (await line()) === '')
    enters.push(Date.now() - t)
    await page.keyboard.press('Backspace')
    await until(async () => (await line()).includes(NAME))
  }
  const t = Date.now()
  await page.keyboard.press('Meta+z')
  await page.waitForTimeout(50)
  const undo = Date.now() - t
  const scrolled = await scroll()
  // What one look at the line costs by itself (the measuring's own share of each number).
  const looks = []
  for (let i = 0; i < 10; i += 1) {
    const t0 = Date.now()
    await line()
    looks.push(Date.now() - t0)
  }
  console.log(`${name.padEnd(9)} one look at the line ${median(looks)} ms`)
  console.log(`${name.padEnd(9)} open ${opened} ms | keystroke ${median(keys)} ms (max ${Math.max(...keys)}) | Enter ${median(enters)} ms | scroll ${scrolled} ms`)
  return undo
}

// Obsidian.
const obsidian = await chromium.connectOverCDP(process.env.OBSIDIAN_WS)
const app = obsidian.contexts().flatMap((context) => context.pages()).find((page) => page.url().startsWith('app://obsidian.md/index.html'))
await app.bringToFront()
await measure('Obsidian', app, {
  open: async () => {
    await app.evaluate(async () => window.app.workspace.openLinkText('Scratch.md', '', false))
    const t = Date.now()
    await app.evaluate(async (note) => window.app.workspace.openLinkText(note, '', false), NOTE)
    await app.waitForFunction(() => (window.app.workspace.activeEditor?.editor?.lineCount() ?? 0) > 10)
    await app.waitForFunction(() => document.querySelectorAll('.workspace-leaf.mod-active .cm-line').length > 10)
    return Date.now() - t
  },
  setCaret: async () => {
    await app.evaluate(([offset, heading]) => {
      const editor = window.app.workspace.activeEditor.editor
      const pos = editor.offsetToPos(offset + heading.length)
      editor.setCursor(pos)
      editor.scrollIntoView({ from: pos, to: pos }, true)
      editor.focus()
      document.querySelector('.workspace-leaf.mod-active .cm-content')?.focus()
    }, [middle, HEADING])
    await app.waitForTimeout(500)
  },
  line: () => app.evaluate(() => { const e = window.app.workspace.activeEditor.editor; return e.getLine(e.getCursor().line) }),
  scroll: async () => {
    const scroller = app.locator('.workspace-leaf.mod-active .cm-scroller')
    const t = Date.now()
    for (let i = 0; i < 20; i += 1) await scroller.evaluate((s) => { s.scrollTop += 600 })
    await app.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))))
    return Date.now() - t
  },
})
// Leave Obsidian's copy as it was.
await app.evaluate(async (name) => { const e = window.app.workspace.activeEditor.editor; while (e.getValue().includes(`${name}a`)) e.undo() }, NAME)

// The note editor.
// The editor as hosts get it: the built package (web/dist, from `npm run build`), not the dev server.
const root = new URL('../dist', import.meta.url).pathname
const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm', '.ttf': 'font/ttf', '.json': 'application/json' }
const server = createServer(async (request, response) => {
  const file = path.join(root, decodeURIComponent(new URL(request.url, 'http://local').pathname))
  try {
    const body = await readFile(file)
    response.writeHead(200, { 'content-type': types[path.extname(file)] ?? 'application/octet-stream' })
    response.end(body)
  } catch {
    response.writeHead(404)
    response.end()
  }
}).listen(0, '127.0.0.1')
await new Promise((resolve) => server.once('listening', resolve))
const browser = await chromium.launch()
const context = await browser.newContext({ viewport: { width: 1280, height: 900 } })
await context.addInitScript(() => localStorage.setItem('kit.note.autosave', 'off'))
const page = await context.newPage()
await page.goto(`http://127.0.0.1:${server.address().port}/dev.html?locale=en-US`)
await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 90000 })
const run = (command, params = {}) => page.evaluate(async ([command, params]) => (await window.devHost.connection().ai.run([{ command, params }], false)).completed[0].result, [command, params])
await measure('Kition', page, {
  open: async () => {
    const t = Date.now()
    await page.evaluate(async ([file, body]) => window.devHost.open(file, new TextEncoder().encode(body).buffer), [NOTE, text])
    await page.waitForFunction((file) => document.getElementById('status')?.textContent?.includes(`${file} open in`), NOTE, { timeout: 120000 })
    const kit = page.frames().find((frame) => frame !== page.mainFrame())
    await kit.locator('[data-testid="note-canvas"] canvas').first().waitFor()
    return Date.now() - t
  },
  setCaret: async () => {
    const kit = page.frames().find((frame) => frame !== page.mainFrame())
    const box = await kit.locator('[data-testid="note-canvas"] canvas').first().boundingBox()
    await page.mouse.click(box.x + box.width / 2, box.y + 40)
    await run('selection.set', { anchor: [...text.slice(0, middle + HEADING.length)].length })
    await page.waitForTimeout(800)
  },
  line: async () => (await run('selection.get')).before ?? '',
  scroll: async () => {
    const kit = page.frames().find((frame) => frame !== page.mainFrame())
    const box = await kit.locator('[data-testid="note-canvas"] canvas').first().boundingBox()
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
    const t = Date.now()
    for (let i = 0; i < 20; i += 1) await page.mouse.wheel(0, 1200)
    await kit.locator('body').evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))))
    return Date.now() - t
  },
})
await browser.close()
await server.close()
await obsidian.close()
process.exit(0)
