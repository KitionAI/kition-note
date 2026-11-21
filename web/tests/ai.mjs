// The note's AI review in a real browser, through tests/ai-host.html (scripted `host/generate`
// and the host action `design.fromRecord`): node tests/ai.mjs [chromium|webkit]
// Prints PASS or FAIL per step; exits 1 on any FAIL. Needs the engine built into public/engine.
// Run it with no HTTP proxy set.
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'
import { canvasReady, caretAfter, focusCanvas, primaryKey, selectText } from './canvasHelpers.mjs'

const which = process.argv[2] ?? 'chromium'
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const context = await browser.newContext({ viewport: { width: 1280, height: 900 } })
// These steps check unsaved edits: the note's autosave is off (src/autosave.tsx).
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

const NOTE = ['---', 'title: Plan', '---', '# Plan', '', 'Goals for the quarter.', '', 'Closing words.', ''].join('\n')
const kit = () => page.frames().find((frame) => frame !== page.mainFrame())
const engineText = () => page.evaluate(async () => (await window.aiHost.connection().ai.run([{ command: 'text.read', params: {} }], false)).completed[0].result.text)
const editorText = engineText
const run = (command, params = {}) => page.evaluate(async ([command, params]) => (await window.aiHost.connection().ai.run([{ command, params }], false)).completed[0].result, [command, params])
const select = (text) => selectText(page, run, engineText, text)
/** Opens a menu: a drop-down in Menus, a ribbon tab in Single line and Classic. */
async function menu(name) {
  const dropDown = kit().getByTestId(`note-menu-${name}`)
  if ((await dropDown.count()) > 0) await dropDown.click()
  else await kit().getByTestId(`ribbon-tab-${name}`).click()
}
async function open(query = '') {
  await page.goto(`${origin}/tests/ai-host.html${query}`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent === 'ready', null, { timeout: 90000 })
  await page.evaluate(async (text) => window.aiHost.open('Ai.md', text), NOTE)
  const ready = await until(async () => (await canvasReady(page)) && (await editorText()) === NOTE, 300)
  if (ready) await focusCanvas(page)
  return ready
}

try {
  step('opens the note in the AI host', await open())

  // A selection offers the rewrites; nothing is sent until the person asks (UI-G1).
  await select('Goals for the quarter.')
  step('a selection shows the rewrite bar', await until(async () => (await kit().getByTestId('note-ai-selection').count()) === 1))
  step('nothing is generated before a click', (await page.evaluate(() => window.aiHost.generated.length)) === 0)
  await page.evaluate(() => {
    window.aiHost.hold = true
    window.aiHost.answers.push({ text: 'Our goals for this quarter.' })
  })
  await kit().getByTestId('note-ai-improve').click()
  step('the review bar shows while the model writes', await until(async () => (await kit().getByTestId('note-ai-running').count()) === 1))
  await until(async () => (await page.evaluate(() => window.aiHost.generated.length)) > 0)
  const asked = await page.evaluate(() => window.aiHost.generated[0])
  const prompt = asked?.messages?.map((message) => message.parts.map((part) => part.text).join('')).join('\n') ?? ''
  step('host/generate asks for the defined rewrite purpose and carries the selection', asked?.purpose === 'rewrite' && asked?.output === 'text' && prompt.includes('Goals for the quarter.'), JSON.stringify(asked).slice(0, 200))

  // Typing goes on while the model writes; the note does not change by the suggestion yet.
  await focusCanvas(page)
  await caretAfter(page, run, engineText, 'Closing words.')
  await page.keyboard.type(' More.')
  const typed = NOTE.replace('Closing words.', 'Closing words. More.')
  step('typing works while the model writes', await until(async () => (await engineText()) === typed), JSON.stringify(await engineText()))
  await page.evaluate(() => window.aiHost.release())
  step('the suggestion shows as a diff', await until(async () => (await kit().getByTestId('note-ai-diff').count()) === 1))
  step('the note is unchanged until Accept', (await engineText()) === typed)

  // Accept: one engine command over the reviewed text.
  await kit().getByTestId('note-ai-accept').click()
  const accepted = typed.replace('Goals for the quarter.', 'Our goals for this quarter.')
  step('Accept replaces the reviewed text', await until(async () => (await engineText()) === accepted && (await editorText()) === accepted), JSON.stringify(await engineText()))
  step('the review bar closes', (await kit().getByTestId('note-ai-review').count()) === 0)
  await focusCanvas(page)
  await page.keyboard.press(`${mod}+z`)
  step('one Undo takes the suggestion back, keeping the typing', await until(async () => (await engineText()) === typed), JSON.stringify(await engineText()))
  await page.keyboard.press(`${mod}+Shift+z`)
  step('Redo puts it back', await until(async () => (await engineText()) === accepted))
  await page.evaluate(() => window.aiHost.connection().save())
  const saved = await until(async () => (await page.evaluate(() => window.aiHost.file())) === accepted)
  step('the saved file holds the accepted text', saved)

  // A refusal is shown with its reason; the note stays as it was.
  await page.evaluate(() => {
    window.aiHost.hold = false
    window.aiHost.answers.push({ refuse: 'quota' })
  })
  await select('Closing words. More.')
  await kit().getByTestId('note-ai-shorten').click()
  step('a refusal shows its reason', await until(async () => (await kit().getByTestId('note-ai-error').getAttribute('data-reason')) === 'quota'))
  await kit().getByTestId('note-ai-dismiss').click()
  step('a refusal changes nothing', (await engineText()) === accepted)

  // Reject drops the suggestion.
  await page.evaluate(() => window.aiHost.answers.push({ text: 'A much longer closing paragraph.' }))
  await select('Closing words. More.')
  await kit().getByTestId('note-ai-expand').click()
  await until(async () => (await kit().getByTestId('note-ai-reject').count()) === 1)
  await kit().getByTestId('note-ai-reject').click()
  step('Reject keeps the note as it was', (await kit().getByTestId('note-ai-review').count()) === 0 && (await engineText()) === accepted)

  // The AI menu: the agent with the selection, and the host action from the note's properties.
  await select('Our goals')
  await menu('AI')
  await kit().getByTestId('note-control-ai-agent').click()
  await until(async () => (await page.evaluate(() => window.aiHost.agent.length)) > 0)
  const prompts = await page.evaluate(() => window.aiHost.agent)
  step('Ask the agent sends the selection', prompts.length === 1 && String(prompts[0]).includes('Our goals'), JSON.stringify(prompts))
  await menu('AI')
  await kit().getByTestId('note-control-ai-design').click()
  const actions = await until(async () => (await page.evaluate(() => window.aiHost.actions.length)) === 1)
  const action = await page.evaluate(() => window.aiHost.actions[0])
  step('Make a design runs host/action with the title and properties', actions && action.params.title === 'Plan' && action.params.fields?.title === 'Plan', JSON.stringify(action))

  // Without `generate` the rewrites are not offered.
  step('reopens without generate', await open('?generate=0'))
  await select('Goals for the quarter.')
  await page.waitForTimeout(300)
  step('no rewrite bar without generate', (await kit().getByTestId('note-ai-selection').count()) === 0)
  await menu('AI').catch(() => {})
  step('no rewrite rows without generate', (await kit().getByTestId('note-control-ai-improve').count()) === 0 && (await kit().getByTestId('note-control-ai-design').count()) === 0 && (await kit().getByTestId('note-control-ai-agent').count()) === 1)

  step('no page errors', errors.length === 0, errors.join(' | '))
} catch (error) {
  step('flow ran to the end', false, error instanceof Error ? error.stack : String(error))
} finally {
  await browser.close()
  await server.close()
}
console.log(failures ? `${failures} step(s) failed in ${which}` : `all steps passed in ${which}`)
process.exit(failures ? 1 : 0)
