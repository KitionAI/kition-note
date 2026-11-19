// The note follows the host's color scheme, through the development host: node tests/scheme.mjs
// [chromium|webkit]. Opens a note with every kind of block in the dark scheme, checks that the
// engine paints its paper dark and that the canvas around it is the same color (no white sheet,
// no frame), then switches the host to light and back without a reload and checks that the note
// repaints each time and nothing moves. Needs the engine and fonts built into public/ (pnpm run
// engine). SHOT=<dir> writes screenshots of both schemes there.
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'

const which = process.argv[2] ?? 'chromium'
const shots = process.env.SHOT ?? null
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const context = await browser.newContext({ viewport: { width: 1280, height: 1000 } })
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

// The engine's papers (crates/view/view/src/theme.rs) and app.css's `--note-paper`.
const PAPER = { light: [255, 255, 255], dark: [30, 30, 30] }
const NOTE = [
  '---',
  'title: Launch',
  'tags: [plan, review]',
  'done: true',
  '---',
  '# Launch notes',
  '',
  'Some **bold**, *emphasis*, `inline code`, a [link](https://kition.ai), a #tag, ==highlighted== and ==🔵blue== text, and math $E = mc^2$.',
  '',
  '## Tasks',
  '',
  '- [ ] Write the brief',
  '- [x] Book the room',
  '- A list item',
  '  - nested under it',
  '1. First',
  '2. Second',
  '',
  '> A quote that is long enough to wrap onto a second row of the column so the bar runs along both rows.',
  '',
  '> [!note] A note',
  '> With a body.',
  '> > [!warning] Nested',
  '> > Inside the note.',
  '',
  '> [!tip] A tip',
  '> Good advice.',
  '',
  '```rust',
  'fn main() {',
  '    // A comment',
  '    let answer = 42;',
  '    println!("hi {answer}");',
  '}',
  '```',
  '',
  '| Name | Owner |',
  '| --- | --- |',
  '| Brief | Ana |',
  '',
  '$$',
  '\\int_0^1 x^2 \\, dx = \\frac{1}{3}',
  '$$',
  '',
  '```mermaid',
  'graph LR; A[Start] --> B[Done]',
  '```',
  '',
  '---',
  '',
  'The end.',
  '',
].join('\n')

const kit = () => page.frames().find((frame) => frame !== page.mainFrame())
const canvas = () => kit().locator('[data-testid="note-canvas"] canvas').first()

/** The colors at points of the page (CSS pixels), from a screenshot of it. */
async function colorsAt(points) {
  const png = (await page.screenshot()).toString('base64')
  return page.evaluate(
    async ([png, points]) => {
      const bitmap = await createImageBitmap(await (await fetch(`data:image/png;base64,${png}`)).blob())
      const board = new OffscreenCanvas(bitmap.width, bitmap.height)
      const context = board.getContext('2d')
      context.drawImage(bitmap, 0, 0)
      const scale = bitmap.width / window.innerWidth
      return points.map(([x, y]) => [...context.getImageData(Math.round(x * scale), Math.round(y * scale), 1, 1).data.slice(0, 3)])
    },
    [png, points],
  )
}
const near = (a, b, by = 3) => a.every((value, index) => Math.abs(value - b[index]) <= by)

/** Points on the canvas: the margin beside the column (the note's paper) and the canvas's corner. */
async function paperPoints() {
  const box = await canvas().boundingBox()
  const frame = await kit().frameElement().then((element) => element.boundingBox())
  // The canvas's top left corner, and a point in the left margin a third of the way down.
  return [
    [box.x + 4, box.y + 4],
    [box.x + 12, box.y + box.height / 3],
    [box.x + box.width - 20, box.y + box.height - 8],
    [frame.x + 2, box.y + box.height / 2],
  ]
}

async function paints(scheme) {
  const points = await paperPoints()
  return until(async () => (await colorsAt(points.slice(0, 3))).every((color) => near(color, PAPER[scheme])), 60)
}

/** Screenshots of the note's top and of the rest below it, for a person to look at. */
async function shoot(name) {
  if (!shots) return
  await page.screenshot({ path: `${shots}/${name}-top-${which}.png` })
  const box = await canvas().boundingBox()
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
  await page.mouse.wheel(0, 780)
  await page.waitForTimeout(1200)
  await page.screenshot({ path: `${shots}/${name}-below-${which}.png` })
  await page.mouse.wheel(0, -780)
  await page.waitForTimeout(600)
}

const where = () => page.evaluate(async () => (await window.devHost.connection().ai.run([{ command: 'selection.where', params: { at: 200 } }], false)).completed[0].result)

try {
  await page.goto(`${origin}/dev.html?locale=en-US&scheme=dark`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 90000 })
  await page.evaluate(async (text) => window.devHost.open('Scheme.md', new TextEncoder().encode(text).buffer), NOTE)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 90000 })
  step('opens the note on the engine canvas', await until(async () => (await canvas().count()) > 0, 300))
  step('the host marks the editor dark', (await kit().evaluate(() => document.documentElement.dataset.colorScheme)) === 'dark')
  step('the note paints its paper dark', await paints('dark'), JSON.stringify(await colorsAt(await paperPoints())))
  const backdrop = await kit().evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--kit-canvas').trim())
  const paper = await kit().evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--note-paper').trim())
  step('the canvas backdrop is the dark paper', backdrop === paper && paper === '#1e1e1e', `${backdrop} / ${paper}`)
  // Diagrams and overlays settle.
  await page.waitForTimeout(1500)
  const placed = await where()
  await shoot('note-dark')

  await page.evaluate(() => window.devHost.setScheme('light'))
  step('switched live to light, the note repaints white', await paints('light'), JSON.stringify(await colorsAt(await paperPoints())))
  step('the canvas backdrop is the light paper', (await kit().evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--kit-canvas').trim())) === '#ffffff')
  step('nothing moves', JSON.stringify(await where()) === JSON.stringify(placed), `${JSON.stringify(await where())} vs ${JSON.stringify(placed)}`)
  await page.waitForTimeout(1500)
  await shoot('note-light')

  await page.evaluate(() => window.devHost.setScheme('dark'))
  step('and back to dark', await paints('dark'), JSON.stringify(await colorsAt(await paperPoints())) + (await kit().evaluate(() => document.documentElement.dataset.colorScheme)))
  step('no page errors', errors.length === 0, errors.join('; '))
} catch (error) {
  step('runs', false, error instanceof Error ? error.stack : String(error))
} finally {
  await browser.close()
  await server.close()
}
console.log(failures ? `${failures} failed` : 'all passed')
process.exit(failures ? 1 : 0)
