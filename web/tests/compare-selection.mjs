// A selection as Obsidian draws it: the same text and selection in a running Obsidian and in the
// note editor, each written as a screenshot (`obsidian.png`, `kition.png`) into OUT, to compare by
// eye. TEXT, FROM and TO (byte offsets into TEXT) choose the text and the selection.
//
//   OBSIDIAN_WS=ws://127.0.0.1:9333/devtools/browser/… OUT=<dir> node tests/compare-selection.mjs
import { mkdirSync } from 'node:fs'
import { chromium } from 'playwright'
import { createServer } from 'vite'
const out = process.env.OUT ?? '/tmp/compare-selection'
mkdirSync(out, { recursive: true })
const TEXT = process.env.TEXT ?? '# Selection\n\nA first paragraph with **bold** text that is long enough to wrap onto a second row in the column.\n\n- an item\n- another item\n\nLast line.\n'
const from = Number(process.env.FROM ?? TEXT.indexOf('paragraph')), to = Number(process.env.TO ?? TEXT.indexOf('another') + 4)
const obsidian = await chromium.connectOverCDP(process.env.OBSIDIAN_WS)
const app = obsidian.contexts().flatMap((c) => c.pages()).find((p) => p.url().startsWith('app://obsidian.md/index.html'))
await app.evaluate(async ([text, from, to]) => {
  const file = window.app.vault.getAbstractFileByPath('Selection.md') ?? (await window.app.vault.create('Selection.md', ''))
  await window.app.vault.modify(file, text)
  const leaf = window.app.workspace.getLeaf(false)
  await leaf.openFile(file, { state: { mode: 'source', source: false } })
  await new Promise((r) => setTimeout(r, 600))
  const editor = leaf.view.editor
  editor.focus()
  editor.setSelection(editor.offsetToPos(from), editor.offsetToPos(to))
}, [TEXT, from, to])
await app.mouse.move(2, 2)
await app.waitForTimeout(800)
await app.locator('.workspace-leaf.mod-active .view-content').screenshot({ path: `${out}/obsidian.png` })
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const browser = await chromium.launch()
const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, deviceScaleFactor: 2 })
await context.addInitScript(() => localStorage.setItem('kit.note.autosave', 'off'))
const page = await context.newPage()
await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/dev.html?locale=en-US`)
await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 90000 })
await page.evaluate(async (text) => window.devHost.open('Selection.md', new TextEncoder().encode(text).buffer), TEXT)
await page.waitForTimeout(2500)
const kit = page.frames().find((f) => f !== page.mainFrame())
await kit.locator('[data-testid="note-canvas"] canvas').first().click({ position: { x: 600, y: 600 } })
const chars = (s) => [...s].length
await page.evaluate(async ([a, f]) => window.devHost.connection().ai.run([{ command: 'select.set', params: { anchor: a, focus: f } }], false), [chars(TEXT.slice(0, from)), chars(TEXT.slice(0, to))])
await page.mouse.move(1, 1)
await page.waitForTimeout(1000)
await kit.getByTestId('note-editor').screenshot({ path: `${out}/kition.png` })
await browser.close(); await server.close(); await obsidian.close()
