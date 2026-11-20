// The note saves itself (src/autosave.tsx), through the development host: node tests/autosave.mjs [chromium|webkit]
// Typing is saved without Save; a change on disk the person has not decided on, a host that
// disabled Save, and a read-only note are never saved on their own.
// Prints PASS or FAIL per step; exits 1 on any FAIL. Needs the engine built into public/engine.
// Run it with no HTTP proxy set.
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'
import { canvasReady, caretAfter, focusCanvas } from './canvasHelpers.mjs'

const which = process.argv[2] ?? 'chromium'
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const context = await browser.newContext({ viewport: { width: 1280, height: 900 } })
const errors = []
let failures = 0
const step = (name, ok, detail = '') => {
  if (!ok) failures += 1
  console.log(`${ok ? 'PASS' : 'FAIL'} ${name}${detail ? ` - ${detail}` : ''}`)
}

const NOTE = '# Plan\n\nGoals for the quarter.\n\nClosing words.\n'

/** A page of the development host (`query` as dev.html takes it) with `name` open, and its helpers. */
async function openPage(name, query = '') {
  const page = await context.newPage()
  page.on('pageerror', (error) => errors.push(error.message))
  const until = async (check, tries = 100) => {
    for (let i = 0; i < tries; i += 1) {
      if (await check().catch(() => false)) return true
      await page.waitForTimeout(100)
    }
    return false
  }
  await page.goto(`${origin}/dev.html?locale=en-US${query}`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 90000 })
  await page.evaluate(async ([name, text]) => window.devHost.open(name, new TextEncoder().encode(text).buffer), [name, NOTE])
  // Once open, the status line names the note; it may already say the revision a view change moved.
  await page.waitForFunction((name) => document.getElementById('status')?.textContent?.startsWith(name), name, { timeout: 90000 })
  const kit = () => page.frames().find((frame) => frame !== page.mainFrame())
  const run = (command, params = {}) => page.evaluate(async ([command, params]) => (await window.devHost.connection().ai.run([{ command, params }], false)).completed[0].result, [command, params])
  const editorText = async () => (await run('text.read')).text
  await until(async () => (await canvasReady(page)) && (await editorText()) === NOTE, 300)
  await focusCanvas(page)
  const version = () => page.evaluate((name) => window.devHost.files.get(name).version, name)
  const fileText = () => page.evaluate((name) => new TextDecoder().decode(new Uint8Array(window.devHost.files.get(name).bytes)), name)
  /** Types `text` at the end of the line holding `after`. */
  const typeAfter = async (after, text) => {
    await caretAfter(page, run, editorText, after)
    await page.keyboard.type(text)
  }
  return { page, until, kit, version, fileText, typeAfter }
}

try {
  // Typing is saved a moment later, without Save; nothing is left unsaved.
  {
    const { page, until, kit, version, fileText, typeAfter } = await openPage('Typed.md')
    await typeAfter('Goals for the quarter.', ' Saved by itself.')
    const saved = await until(async () => (await version()) > 1 && (await fileText()).includes('Saved by itself.'), 60)
    step('typing is saved without Save', saved, JSON.stringify(await fileText()))
    step('and the note is not left unsaved', await until(async () => (await kit().getByTestId('note-unsaved').count()) === 0, 30))
    await page.close()
  }

  // A change on disk while there are unsaved edits waits for the person: no save over it.
  {
    const { page, until, kit, version, fileText, typeAfter } = await openPage('Waiting.md')
    // The change on disk lands first; the typing right after makes the note unsaved against it.
    await page.evaluate((text) => {
      const file = window.devHost.files.get('Waiting.md')
      window.devHost.files.set('Waiting.md', { bytes: new TextEncoder().encode(text).buffer, version: file.version + 1 })
    }, NOTE.replace('Closing words.', 'Closing words, edited outside.'))
    await typeAfter('Goals for the quarter.', ' Mine.')
    const asked = await until(async () => (await kit().getByTestId('note-disk-changed').count()) === 1, 60)
    const held = await version()
    await page.waitForTimeout(2500)
    step('a change on disk the person has not decided on is not saved over', asked && (await version()) === held && (await fileText()).includes('edited outside') && !(await fileText()).includes('Mine.'), JSON.stringify(await fileText()))
    await kit().getByTestId('note-disk-merge').click()
    const merged = await until(async () => (await fileText()).includes('edited outside') && (await fileText()).includes('Mine.'), 60)
    step('once merged, both sides are saved by themselves', merged, JSON.stringify(await fileText()))
    await page.close()
  }

  // A host that disabled Save gets no save from the note.
  {
    const { page, version, typeAfter } = await openPage('Disabled.md')
    await page.evaluate(() => window.devHost.configure({ disable: ['action:save'] }))
    await page.waitForTimeout(300)
    await typeAfter('Goals for the quarter.', ' Not saved.')
    await page.waitForTimeout(2500)
    step('a host that disabled Save gets no save', (await version()) === 1, String(await version()))
    await page.close()
  }

  // A read-only note is never saved.
  {
    const { page, version, kit } = await openPage('Viewed.md', '&role=viewer')
    await focusCanvas(page)
    await page.keyboard.type('x')
    await page.waitForTimeout(2500)
    step('a read-only note is never saved', (await version()) === 1, String(await version()))
    await page.close()
  }

  step('no page errors', errors.length === 0, errors.join(' | '))
} catch (error) {
  step('autosave ran to the end', false, error instanceof Error ? error.stack : String(error))
} finally {
  await browser.close()
  await server.close()
}
console.log(failures ? `${failures} step(s) failed in ${which}` : `all steps passed in ${which}`)
process.exit(failures ? 1 : 0)
