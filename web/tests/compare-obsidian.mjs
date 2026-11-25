// Side by side with Obsidian: opens each sample in tests/samples in a running Obsidian
// (driven over its DevTools port) and in the note editor (the development host), at the same
// window size, caret at the end, and writes `<sample>.png`, Obsidian on the left, into OUT.
//
//   OBSIDIAN_WS=ws://127.0.0.1:9333/devtools/browser/… OBSIDIAN_VAULT=<vault> OUT=<dir> \
//     node tests/compare-obsidian.mjs [sample name filter]
//
// FOLD=1 folds everything in both first (Obsidian's Fold all, the editor's view.foldAll).
//
// CARET=<text> puts the caret right after the first <text> in both instead (to compare how each
// shows the syntax the caret is in).
//
// SCROLL=0,800,1600 also compares the samples scrolled down by those many pixels (`<sample>@<y>.png`).
//
// Start Obsidian with its own profile and a test vault, never the person's:
//   Obsidian --user-data-dir=<profile> --remote-debugging-port=9333
import { execFileSync } from 'node:child_process'
import { copyFile, mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { chromium } from 'playwright'
import { createServer } from 'vite'

const samples = new URL('./samples/', import.meta.url).pathname
const out = process.env.OUT ?? '/tmp/compare-obsidian'
const vault = process.env.OBSIDIAN_VAULT
const filter = process.argv[2] ?? ''
const width = 1280
const height = 900
await mkdir(out, { recursive: true })
const names = (await readdir(samples)).filter((name) => name.endsWith('.md') && name.includes(filter)).sort()
for (const name of await readdir(samples)) if (name.endsWith('.md') && vault) await copyFile(path.join(samples, name), path.join(vault, name))

// Obsidian: its window at the size, the sidebars closed.
const obsidian = await chromium.connectOverCDP(process.env.OBSIDIAN_WS)
const app = obsidian.contexts().flatMap((context) => context.pages()).find((page) => page.url().startsWith('app://obsidian.md/index.html'))
// Electron has no DevTools window commands: the renderer resizes its own window.
await app.evaluate(([w, h]) => window.require('@electron/remote').getCurrentWindow().setSize(w, h), [width, height])
await app.waitForTimeout(500)
// At the editor's scale (2), whatever the display Obsidian's window is on.
const session = await app.context().newCDPSession(app)
await session.send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 2, mobile: false })
// Playwright's screenshots ignore the override: DevTools' own, of the note's pane.
const shootObsidian = async (file) => {
  const clip = await app.evaluate(() => {
    const { x, y, width, height } = document.querySelector('.workspace-leaf.mod-active .view-content').getBoundingClientRect()
    return { x, y, width, height, scale: 1 }
  })
  const { data } = await session.send('Page.captureScreenshot', { format: 'png', clip })
  await writeFile(file, Buffer.from(data, 'base64'))
}
await app.evaluate(() => {
  window.app.workspace.leftSplit.collapse()
  window.app.workspace.rightSplit.collapse()
})

// The note editor.
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const browser = await chromium.launch()
const context = await browser.newContext({ viewport: { width, height }, deviceScaleFactor: 2 })
await context.addInitScript(() => localStorage.setItem('kit.note.autosave', 'off'))
const page = await context.newPage()
await page.goto(`${origin}/dev.html?locale=en-US`)
await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 90000 })

for (const name of names) {
  const text = await readFile(path.join(samples, name), 'utf8')
  const base = name.replace(/\.md$/, '')
  const caretAt = process.env.CARET && text.includes(process.env.CARET) ? text.indexOf(process.env.CARET) + process.env.CARET.length : null
  await app.evaluate(async ([file, at]) => {
    await window.app.workspace.openLinkText(file, '', false)
    const editor = window.app.workspace.activeEditor?.editor
    if (editor) {
      editor.setCursor(at === null ? { line: editor.lastLine(), ch: 0 } : editor.offsetToPos(at))
      editor.scrollTo(0, 0)
    }
  }, [name, caretAt])
  await app.waitForTimeout(600)
  // Obsidian keeps a note's folds: fold or unfold everything, as the editor is.
  await app.evaluate((fold) => window.app.commands.executeCommandById(fold ? 'editor:fold-all' : 'editor:unfold-all'), Boolean(process.env.FOLD))
  // Its properties block open, as the editor's is.
  await app.evaluate(() => document.querySelector('.workspace-leaf.mod-active .metadata-container.is-collapsed .metadata-properties-heading')?.click())
  await app.waitForTimeout(300)
  // The caret's move scrolls the view down: back to the top once it has, the pointer away.
  await app.evaluate(() => {
    window.app.workspace.activeEditor?.editor?.scrollTo(0, 0)
    const scroller = document.querySelector('.workspace-leaf.mod-active .cm-scroller')
    if (scroller) scroller.scrollTop = 0
  })
  await app.mouse.move(2, 2)
  await app.waitForTimeout(800)
  const obsidianShot = path.join(out, `${base}.obsidian.png`)
  await shootObsidian(obsidianShot)

  await page.evaluate(async ([file, body]) => window.devHost.open(file, new TextEncoder().encode(body).buffer), [name, text])
  await page.waitForFunction((file) => document.getElementById('status')?.textContent?.includes(`${file} open in`), name, { timeout: 90000 })
  const kit = page.frames().find((frame) => frame !== page.mainFrame())
  await kit.locator('[data-testid="note-canvas"] canvas').first().waitFor()
  await page.waitForTimeout(1500)
  // (The editor counts in code points.)
  const anchor = caretAt === null ? 1e9 : [...text.slice(0, caretAt)].length
  await page.evaluate(async (at) => window.devHost.connection().ai.run([{ command: 'select.set', params: { anchor: at } }], false), anchor)
  const folds = process.env.FOLD ? 'view.foldAll' : 'view.unfoldAll'
  await page.evaluate(async (command) => window.devHost.connection().ai.run([{ command, params: {} }], false), folds)
  await page.waitForTimeout(800)
  // Back to the top, as Obsidian is.
  const box = await kit.locator('[data-testid="note-canvas"] canvas').first().boundingBox()
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
  await page.mouse.wheel(0, -100000)
  await page.waitForTimeout(800)
  for (const offset of (process.env.SCROLL ?? '0').split(',').map(Number)) {
    const shot = offset ? `${base}@${offset}` : base
    if (offset) {
      await app.evaluate((y) => {
        const scroller = document.querySelector('.workspace-leaf.mod-active .cm-scroller')
        if (scroller) scroller.scrollTop = y
      }, offset)
      await app.mouse.move(2, 2)
      await app.waitForTimeout(800)
      await shootObsidian(path.join(out, `${shot}.obsidian.png`))
      await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
      await page.mouse.wheel(0, -100000)
      await page.waitForTimeout(300)
      // Wheel steps until the view is there (Chromium at device scale 2 delivers half a
      // synthetic wheel's delta).
      const shownY = async () => Number((await kit.getByTestId('note-editor').getAttribute('data-view'))?.split(',')[1] ?? 0)
      for (let tries = 0; tries < 400; tries += 1) {
        const left = offset - (await shownY())
        if (left <= 0) break
        await page.mouse.wheel(0, Math.min(200, left * 2))
        await page.waitForTimeout(30)
      }
      await page.waitForTimeout(300)
    }
    // The pointer off the note, as Obsidian's.
    await page.mouse.move(1, 1)
    await page.waitForTimeout(800)
    const kitShot = path.join(out, `${shot}.kition.png`)
    await kit.getByTestId('note-editor').screenshot({ path: kitShot })
    execFileSync('magick', [path.join(out, `${shot}.obsidian.png`), kitShot, '-gravity', 'north', '-background', '#cccccc', '-splice', '0x0', '+append', path.join(out, `${shot}.png`)])
    console.log(`wrote ${path.join(out, `${shot}.png`)}`)
  }
}

await session.send('Emulation.clearDeviceMetricsOverride')
await browser.close()
await server.close()
await obsidian.close()
