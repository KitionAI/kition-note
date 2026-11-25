// Takes the README's screenshots of the Note editor (assets/readme/*.png at the repository root):
// the editor in the development host, on a sample note, at a fixed viewport. Not a test, and not
// part of any suite; re-run it by hand when the editor changes:
//
//   cd web && pnpm install && pnpm run engine      # the engine and fonts in public/
//   node scripts/readme-shots.mjs [name ...]       # every shot, or only the named ones
//
// Run it with no HTTP proxy set (the browser reaches a local server). The PDF page is rendered
// with macOS `sips`; elsewhere that shot is skipped. Images are written as PNG at device scale 2;
// shrink them afterwards if they are large (the README's are under 400 KB each).
import { execFileSync } from 'node:child_process'
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from 'playwright'
import { createServer } from 'vite'

const web = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const out = path.resolve(web, '..', 'assets', 'readme')
mkdirSync(out, { recursive: true })
const only = new Set(process.argv.slice(2))
const wanted = (name) => only.size === 0 || only.has(name)

/** The sample note every shot opens. */
const NOTE = `---
status: in progress
owner: Maya Chen
due: 2026-12-01
tags:
  - launch
  - q4
---

The launch moves the new editor from beta to everyone. This note tracks the work, the numbers and the open questions. See [[Pricing notes]] and [[Release checklist]].

> [!tip] Decision
> We ship on **December 1**, with export on by default.

## Tasks

- [x] Freeze the feature list
- [x] Write the release notes
- [ ] Record the demo video
- [ ] Brief the support team #support

## Rollout

| Stage   | Audience      | Date   |
| ------- | ------------- | ------ |
| Beta    | 2,000 testers | Nov 10 |
| Preview | Paid teams    | Nov 20 |
| General | Everyone      | Dec 1  |

Expected reach after the first week is $R = N \\cdot p \\cdot (1 - c)$, where $c$ is churn.

$$
g = \\frac{1}{4} \\sum_{w=1}^{4} \\frac{u_w - u_{w-1}}{u_{w-1}}
$$

## Flow

\`\`\`mermaid
graph LR
  A[Beta feedback] --> B{Ready?}
  B -->|Yes| C[Preview]
  B -->|No| D[Fix]
  D --> B
  C --> E[Launch]
\`\`\`

## Open questions

- Do we keep the old export menu for one more release?
- Who owns the launch post?
`
const NAME = 'Product launch plan.md'

const escape = (text) => text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
/** A terminal-style card for a command and its real output; diff lines are colored. */
function terminal(command, output) {
  const line = (text) => {
    const color = text.startsWith('+') && !text.startsWith('+++') ? '#7ee2a8' : text.startsWith('-') && !text.startsWith('---') ? '#ff9b9b' : text.startsWith('@@') ? '#b9aefc' : '#e6e3de'
    return `<div style="color:${color}">${escape(text) || '&nbsp;'}</div>`
  }
  return `<!doctype html><body style="margin:0;background:#fafaf9;padding:24px">
<main style="display:inline-block;min-width:860px;background:#1a1a1a;border-radius:12px;padding:20px 24px;box-shadow:0 2px 8px rgba(26,26,26,.12);font:15px/1.6 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;white-space:pre">
<div style="color:#8a8780">${escape(command)}</div>${output.split('\n').map(line).join('')}</main></body>`
}

const server = await createServer({ configFile: path.join(web, 'vite.config.ts'), server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const browser = await chromium.launch()
const MOD = process.platform === 'darwin' ? 'Meta' : 'Control'

/** A page with the note open in the development host; screenshots take only the kit's frame. */
async function open(height = 900) {
  const context = await browser.newContext({ viewport: { width: 1440, height }, deviceScaleFactor: 2 })
  await context.addInitScript(() => localStorage.setItem('kit.note.autosave', 'off'))
  const page = await context.newPage()
  await page.goto(`${origin}/dev.html?locale=en-US&files=1`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 90000 })
  await page.evaluate(async ([name, text]) => window.devHost.open(name, new TextEncoder().encode(text).buffer), [NAME, NOTE])
  await page.waitForFunction((name) => document.getElementById('status')?.textContent?.includes(`${name} open in `), NAME, { timeout: 90000 })
  const kit = page.frames().find((frame) => frame !== page.mainFrame())
  await kit.locator('[data-testid="note-canvas"] canvas').first().waitFor({ timeout: 90000 })
  await page.waitForTimeout(2500)
  const run = (command, params = {}) => page.evaluate(async ([command, params]) => (await window.devHost.connection().ai.run([{ command, params }], false)).completed[0].result, [command, params])
  const text = async () => (await run('text.read')).text
  /** Unicode characters before `part` in the note. */
  const indexOf = async (part) => {
    const body = await text()
    const index = body.indexOf(part)
    if (index < 0) throw new Error(`no ${JSON.stringify(part)} in the note`)
    return [...body.slice(0, index)].length
  }
  const canvas = () => kit.locator('[data-testid="note-canvas"] canvas').first()
  /** Where the engine draws Unicode character `index`, in the page's pixels (a query: it moves nothing). */
  const point = async (index) => {
    const where = await run('select.where', { at: index })
    const [x, y, , , zoom] = (await kit.getByTestId('note-editor').getAttribute('data-view')).split(',').map(Number)
    const box = await canvas().boundingBox()
    return { x: box.x + (where.x - x) * zoom, y: box.y + (where.y - y + (where.height ?? 20) / 2) * zoom, top: box.y }
  }
  /** Scrolls the note with the wheel until `part` is `offset` pixels below the top of the view. */
  const scrollTo = async (part, offset = 90) => {
    const box = await canvas().boundingBox()
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
    for (let i = 0; i < 12; i += 1) {
      const at = await point(await indexOf(part))
      const delta = at.y - (at.top + offset)
      if (Math.abs(delta) < 30) break
      await page.mouse.wheel(0, delta)
      await page.waitForTimeout(400)
    }
  }
  /** Clicks right after `part`, or selects it with a click and a shift-click, as a person would. */
  const caret = async (part, select = false) => {
    const start = await indexOf(part)
    const end = await point(start + [...part].length)
    if (select) {
      const from = await point(start)
      await page.mouse.click(from.x + 1, from.y)
      await page.keyboard.down('Shift')
      await page.mouse.click(end.x - 1, end.y)
      await page.keyboard.up('Shift')
    } else {
      await page.mouse.click(end.x, end.y)
    }
    await page.waitForTimeout(300)
  }
  const shot = async (name) => {
    await page.waitForTimeout(600)
    await page.locator('#kit').screenshot({ path: path.join(out, `${name}.png`) })
    console.log(`wrote assets/readme/${name}.png`)
  }
  return { context, page, kit, run, text, caret, point, indexOf, scrollTo, shot }
}

try {
  if (wanted('hero')) {
    // The note as it opens: properties, a callout, tasks, a table.
    const { context, shot } = await open(1100)
    await shot('hero')
    await context.close()
  }

  if (wanted('live-preview')) {
    // The caret in a construct shows its Markdown (here inline math); everything else stays drawn.
    const { context, caret, scrollTo, shot } = await open()
    await scrollTo('## Rollout')
    await caret('$R = N')
    await shot('live-preview')
    await context.close()
  }

  if (wanted('table')) {
    // A table cell under the caret: row and column handles, and the column menu.
    const { context, kit, page, caret, point, indexOf, scrollTo, shot } = await open()
    await scrollTo('## Rollout')
    await caret('Paid teams')
    // A right click on a header cell opens the column menu.
    const header = await point(await indexOf('Date '))
    await page.mouse.click(header.x + 20, header.y, { button: 'right' })
    await kit.getByTestId('note-table-menu').waitFor()
    await shot('table')
    await context.close()
  }

  if (wanted('comments')) {
    // A comment thread on a phrase, in the panel beside the note. The thread is made through the
    // engine's comment commands, with names and times, as a host that knows its people would.
    const { context, kit, run, text, caret, scrollTo, shot } = await open()
    const before = await text()
    const { thread } = await run('comment.add', { quote: 'with export on by default', body: 'Can support handle the export questions in week one?', name: 'Maya Chen', created: '2026-11-12T09:30:00Z' })
    await run('comment.reply', { thread, body: 'Yes: the help article is ready, and the team has a briefing on Nov 28.', name: 'Sam Ortiz', created: '2026-11-12T10:05:00Z' })
    if ((await text()) !== before) throw new Error('a comment changed the note text')
    await scrollTo('> [!tip] Decision')
    await kit.getByTestId('note-comments-toggle').click()
    await caret('Record the demo video')
    await shot('comments')
    await context.close()
  }

  if (wanted('find-outline')) {
    // Find with its matches counted, and the outline of the note's headings.
    const { context, kit, page, caret, shot } = await open()
    await kit.getByTestId('ribbon-tab-View').click()
    await kit.getByTestId('note-control-outline').click()
    await kit.getByTestId('note-outline-row').first().waitFor()
    await caret('The launch')
    await page.keyboard.press(`${MOD}+f`)
    await kit.getByTestId('note-find-input').fill('launch')
    await kit.getByTestId('note-find-input').press('Enter')
    await page.waitForTimeout(500)
    await shot('find-outline')
    await context.close()
  }

  if (wanted('export')) {
    // The note exported as a PDF (`file.export`), its first page rendered as a picture.
    const { context, run } = await open()
    const pdf = await run('file.export', { format: 'pdf', pageSize: 'a4' })
    const dir = mkdtempSync(path.join(tmpdir(), 'note-readme-'))
    writeFileSync(path.join(dir, 'export.pdf'), Buffer.from(pdf.data, 'base64'))
    if (process.platform === 'darwin') {
      // The page at 1600 pixels high, cut to its top part (the text, the callout, the tasks, the table).
      const png = path.join(out, 'export-pdf.png')
      execFileSync('sips', ['-s', 'format', 'png', '-Z', '1600', path.join(dir, 'export.pdf'), '--out', png], { stdio: 'ignore' })
      execFileSync('sips', ['-c', '700', '1131', '--cropOffset', '90', '0', png], { stdio: 'ignore' })
      console.log('wrote assets/readme/export-pdf.png')
    } else {
      console.log('skipped export-pdf.png: needs macOS sips')
    }
    await context.close()
  }

  if (wanted('byte-for-byte')) {
    // One table cell edited in the editor and saved: the file on disk differs from the one opened in
    // that cell only. The shot is the editor; byte-for-byte-diff.png is `diff -u` of the two files.
    const { context, page, kit, caret, scrollTo, shot } = await open()
    await scrollTo('## Rollout')
    await caret('2,000')
    for (let i = 0; i < 5; i += 1) await page.keyboard.press('Backspace')
    await page.keyboard.type('3,500')
    await kit.getByTestId('note-save').click()
    await page.waitForFunction((name) => window.devHost.files.get(name)?.version > 1, NAME)
    const saved = await page.evaluate((name) => [...new Uint8Array(window.devHost.files.get(name).bytes)], NAME)
    await shot('byte-for-byte')
    const dir = mkdtempSync(path.join(tmpdir(), 'note-readme-'))
    writeFileSync(path.join(dir, 'opened.md'), NOTE)
    writeFileSync(path.join(dir, 'saved.md'), Buffer.from(saved))
    let diff = ''
    try {
      execFileSync('diff', ['-u', 'opened.md', 'saved.md'], { cwd: dir, encoding: 'utf8' })
    } catch (error) {
      diff = error.stdout
    }
    diff = diff.split('\n').map((line) => (line.startsWith('---') || line.startsWith('+++') ? line.split('\t')[0] : line)).join('\n').trimEnd()
    console.log(diff)
    const card = await context.newPage()
    await card.setViewportSize({ width: 960, height: 400 })
    await card.setContent(terminal(`$ diff -u opened.md saved.md`, diff))
    await card.locator('main').screenshot({ path: path.join(out, 'byte-for-byte-diff.png') })
    console.log('wrote assets/readme/byte-for-byte-diff.png')
    await context.close()
  }
} finally {
  await browser.close()
  await server.close()
}
