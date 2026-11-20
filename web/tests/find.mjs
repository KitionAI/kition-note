// Find in a real browser, through the development host: node tests/find.mjs [chromium|webkit]
// Starts the Vite dev server itself and prints PASS or FAIL per step; exits 1 on any FAIL.
// Needs the engine and fonts built into public/ (pnpm run engine). Run it with no HTTP proxy set.
//
// Enter in the Find field selects the next match once the engine has found it (`text.find`, then
// `selection.set`). Escape right after Enter must not hand the keyboard back to the note before
// that match is selected: keys typed next would land where the caret was before Find. Every
// `text.find` reaching the engine is held back here for a moment, so Escape always comes first.
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'
import { canvasReady, focusCanvas, selectChars } from './canvasHelpers.mjs'

const which = process.argv[2] ?? 'chromium'
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const context = await browser.newContext({ viewport: { width: 1280, height: 900 } })
await context.addInitScript(() => {
  localStorage.setItem('kit.note.autosave', 'off')
  const post = Worker.prototype.postMessage
  Worker.prototype.postMessage = function (message, ...rest) {
    if (message?.request?.method === 'execute' && message.request.command === 'text.find') {
      setTimeout(() => post.call(this, message, ...rest), 600)
      return
    }
    return post.call(this, message, ...rest)
  }
})
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

const NAME = 'Find.md'
const ORIGINAL = '# Cells\n\nfirst one here\n'
const kit = () => page.frames().find((frame) => frame !== page.mainFrame())
const run = (command, params = {}) => page.evaluate(async ([command, params]) => (await window.devHost.connection().ai.run([{ command, params }], false)).completed[0].result, [command, params])
const engineText = async () => (await run('text.read')).text

try {
  await page.goto(`${origin}/dev.html?locale=en-US`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 90000 })
  await page.evaluate(async ([name, text]) => window.devHost.open(name, new TextEncoder().encode(text).buffer), [NAME, ORIGINAL])
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 90000 })
  step('opens the note', await until(async () => (await canvasReady(page)) && (await engineText()) === ORIGINAL, 300))
  await focusCanvas(page)
  await selectChars(page, run, 0)
  await page.keyboard.press('ArrowRight')
  await page.keyboard.press('ArrowLeft')

  const mac = await kit().evaluate(() => /Mac|iPhone|iPad/i.test(navigator.platform))
  await page.keyboard.press(`${mac ? 'Meta' : 'Control'}+f`)
  const input = kit().getByTestId('note-find-input')
  await input.waitFor()
  await input.fill('one')
  await input.press('Enter')
  await input.press('Escape')
  await kit().getByTestId('kit-text-sink').waitFor({ state: 'attached' })
  await until(async () => (await kit().evaluate(() => document.activeElement?.dataset?.testid)) === 'kit-text-sink')
  await page.keyboard.press('ArrowRight')
  await page.keyboard.type('!')
  const expected = '# Cells\n\nfirst one! here\n'
  const landed = await until(async () => (await engineText()) !== ORIGINAL, 50)
  const text = await engineText()
  step('a key typed right after Enter and Escape in Find goes after the match', landed && text === expected, JSON.stringify(text))

  step('no page errors', errors.length === 0, errors.join(' | '))
} catch (error) {
  step('find ran to the end', false, error instanceof Error ? error.stack : String(error))
} finally {
  await browser.close()
  await server.close()
}
console.log(failures ? `${failures} step(s) failed in ${which}` : `all steps passed in ${which}`)
process.exit(failures ? 1 : 0)
