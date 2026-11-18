// Line by line against Obsidian: opens each sample in tests/samples in a running Obsidian
// (over its DevTools port) and in the note editor, puts the caret at the note's end in both, and
// prints every source line whose text's first letter sits 1 px or more apart from Obsidian's:
// how much its row's distance from the line before differs (dy), and its x from the note's
// first paragraph's (dx). Lines one of them hides or draws as a widget (callouts, tables,
// properties, diagrams) and code fences are skipped.
//
//   OBSIDIAN_WS=ws://127.0.0.1:9333/devtools/browser/… node tests/compare-geometry.mjs [filter]
//
// FOLD=1 folds everything first in both.
import { readFile, readdir } from 'node:fs/promises'
import path from 'node:path'
import { chromium } from 'playwright'
import { createServer } from 'vite'

const samples = new URL('./samples/', import.meta.url).pathname
const filter = process.argv[2] ?? ''
const names = (await readdir(samples)).filter((name) => name.endsWith('.md') && name.includes(filter)).sort()
const obsidian = await chromium.connectOverCDP(process.env.OBSIDIAN_WS)
const app = obsidian.contexts().flatMap((context) => context.pages()).find((page) => page.url().startsWith('app://obsidian.md/index.html'))
await app.evaluate(([w, h]) => window.require('@electron/remote').getCurrentWindow().setSize(w, h), [1280, 900])
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const browser = await chromium.launch()
const context = await browser.newContext({ viewport: { width: 1280, height: 900 } })
await context.addInitScript(() => localStorage.setItem('kit.note.autosave', 'off'))
const page = await context.newPage()
await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/dev.html?locale=en-US`)
await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 90000 })
const run = (command, params = {}) => page.evaluate(async ([command, params]) => (await window.devHost.connection().ai.run([{ command, params }], false)).completed[0].result, [command, params])

// The first letter or digit of each line (its offset in UTF-16 units), or null.
const firsts = (text) => {
  const out = []
  let at = 0
  for (const line of text.split('\n')) {
    // (A code block's fence draws as its box's padding: skipped.)
    if (/^\s*(?:>\s*)*(?:```|~~~)/.test(line)) {
      out.push(null)
      at += line.length + 1
      continue
    }
    // Past the line's block syntax (quote marks, a list marker, a task box, heading marks, a
    // callout's type): its text's first letter.
    const syntax = /^[\s>]*(?:(?:[-*+]|\d+[.)])\s+)?(?:\[.\]\s+)?(?:#+\s+)?(?:\[![^\]]*\][+-]?\s*)?/u.exec(line)?.[0].length ?? 0
    const match = /\p{L}/u.exec(line.slice(syntax))
    out.push(match ? at + syntax + match.index : null)
    at += line.length + 1
  }
  return out
}

for (const name of names) {
  const text = await readFile(path.join(samples, name), 'utf8')
  const offsets = firsts(text)
  // Obsidian: each line's first letter in document coordinates, scrolling through the note.
  const ours = await app.evaluate(async ([file, offsets, fold]) => {
    const leaf = window.app.workspace.getLeaf(false)
    await leaf.openFile(window.app.vault.getAbstractFileByPath(file))
    const editor = window.app.workspace.activeEditor.editor
    editor.setCursor(editor.lastLine(), 0)
    window.app.commands.executeCommandById(fold ? 'editor:fold-all' : 'editor:unfold-all')
    document.querySelector('.workspace-leaf.mod-active .metadata-container.is-collapsed .metadata-properties-heading')?.click()
    const cm = editor.cm
    const scroller = cm.scrollDOM
    const found = offsets.map(() => null)
    for (let top = 0; top < scroller.scrollHeight + 900; top += 400) {
      scroller.scrollTop = top
      await new Promise((resolve) => setTimeout(resolve, 120))
      const base = scroller.getBoundingClientRect()
      offsets.forEach((offset, index) => {
        if (offset === null || found[index]) return
        // A character a widget renders (a callout, a table, properties, a diagram) has no place
        // of its own: skipped.
        const dom = cm.domAtPos(offset)?.node
        const element = dom?.nodeType === 1 ? dom : dom?.parentElement
        if (!element || element.closest('.cm-embed-block, .metadata-container') || !element.closest('.cm-line')) return
        const rect = cm.coordsAtPos(offset, 1)
        // Its line's top inside the line's padding (where the editor's row starts), so letters
        // of different sizes on one line compare alike.
        const line = element.closest('.cm-line')
        const top = line.getBoundingClientRect().top + Number.parseFloat(getComputedStyle(line).paddingTop)
        if (rect && rect.bottom > base.top && rect.top < base.bottom) found[index] = { x: rect.left - base.left, y: top - base.top + scroller.scrollTop }
      })
    }
    scroller.scrollTop = 0
    return found
  }, [name, offsets, Boolean(process.env.FOLD)])
  // The editor: the same characters (in code points).
  await page.evaluate(async ([file, body]) => window.devHost.open(file, new TextEncoder().encode(body).buffer), [name, text])
  await page.waitForFunction((file) => document.getElementById('status')?.textContent?.includes(`${file} open in`), name, { timeout: 90000 })
  await page.waitForTimeout(1500)
  await run('selection.set', { anchor: 1e9 })
  await run(process.env.FOLD ? 'view.foldAll' : 'view.unfoldAll')
  await page.waitForTimeout(600)
  const kit = []
  for (const offset of offsets) {
    if (offset === null) {
      kit.push(null)
      continue
    }
    const where = await run('selection.where', { at: [...text.slice(0, offset)].length })
    kit.push(where && where.height > 0 ? { x: where.x, y: where.y } : null)
  }
  // Compare: distances between consecutive shown lines, and x from the first paragraph's.
  const lines = text.split('\n')
  const shown = offsets.map((_, index) => index).filter((index) => ours[index] && kit[index])
  const plain = shown.find((index) => /^\p{L}/u.test(lines[index])) ?? shown[0]
  const report = []
  let previous = null
  for (const index of shown) {
    const o = ours[index]
    const k = kit[index]
    const dx = k.x - kit[plain].x - (o.x - ours[plain].x)
    const dy = previous === null ? 0 : k.y - kit[previous].y - (o.y - ours[previous].y)
    if (Math.abs(dx) >= 1 || Math.abs(dy) >= 1) report.push(`  ${String(index + 1).padStart(3)} dx ${dx.toFixed(1).padStart(6)} dy ${dy.toFixed(1).padStart(6)}  ${lines[index].slice(0, 60)}`)
    previous = index
  }
  const missing = offsets.map((offset, index) => offset !== null && Boolean(ours[index]) !== Boolean(kit[index]) ? index + 1 : null).filter(Boolean)
  console.log(`${name}: ${report.length} line(s) apart${missing.length ? `; shown in one only: ${missing.join(', ')}` : ''}`)
  for (const line of report) console.log(line)
  if (process.env.RAW) shown.forEach((index) => console.log(`    raw ${index + 1} o ${ours[index].x.toFixed(1)},${ours[index].y.toFixed(1)} k ${kit[index].x.toFixed(1)},${kit[index].y.toFixed(1)}`))
}
await browser.close()
await server.close()
await obsidian.close()
