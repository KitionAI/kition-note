// An agent's draft and a restored version in the note's editor (Kit UI protocol sections 7d and
// 7m), through the development host with the replica and checkpoints kept, on the sample note:
// the person types, an agent's draft inserts a sentence and is compared (the preview lists it, the
// note does not change), the person comments in the panel meanwhile, and the draft is accepted:
// the editor shows the typing, the comment and the sentence. One Undo (the editor's own button)
// takes back the draft only, Redo brings it back. A draft that writes the text while the person
// types into it is refused with the reason (plain model, rule D5) and changes nothing. A labelled
// checkpoint, a later edit, the checkpoint restored through the host: the note shows the
// checkpoint's text, and one Undo brings the later edit back.
// node tests/drafts.mjs [chromium|webkit]
// Prints PASS or FAIL per step; exits 1 on any FAIL. Needs the engine built into public/engine.
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'
import { canvasReady, caretAfter, focusCanvas, kitFrame, selectText } from './canvasHelpers.mjs'

const which = process.argv[2] ?? 'chromium'
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const context = await browser.newContext({ viewport: { width: 1280, height: 900 } })
// The test saves itself, so a save never lands between its steps.
await context.addInitScript(() => localStorage.setItem('kit.note.autosave', 'off'))
const page = await context.newPage()
const errors = []
let failures = 0
page.on('pageerror', (error) => errors.push(error.message))
const step = (name, ok, detail = '') => {
  if (!ok) failures += 1
  console.log(`${ok ? 'PASS' : 'FAIL'} ${which}: ${name}${detail ? ` - ${detail}` : ''}`)
}
const until = async (check, tries = 200) => {
  for (let i = 0; i < tries; i += 1) {
    if (await check().catch(() => false)) return true
    await page.waitForTimeout(100)
  }
  return false
}

const PATH = 'Sample.md'
const kit = () => kitFrame(page)
/** Runs an engine command through the host's AI bridge (`draft`: in the agent's draft). */
const run = (command, params = {}, options = {}) =>
  page.evaluate(async ([command, params, options]) => (await window.devHost.connection().ai.run([{ command, params }], false, options)).completed[0].result, [command, params, options])
const engineText = async (draft = false) => (await run('text.read', {}, draft ? { draft: true } : {})).text
/** The text the editor shows (its accessible mirror of the canvas). */
const shown = async () => (await kit().getByTestId('note-text').textContent()) ?? ''
/** Whether the editor shows every one of `present` and none of `absent`. */
const shows = (present, absent = []) =>
  until(async () => {
    const text = await shown()
    return present.every((part) => text.includes(part)) && absent.every((part) => !text.includes(part))
  })
/** The person types `text` right after `after`. */
async function typeAfter(after, text) {
  await focusCanvas(page)
  await caretAfter(page, run, engineText, after)
  await page.keyboard.type(text)
  await until(async () => (await engineText()).includes(`${after}${text}`))
}
const press = async (id) => {
  await until(() => kit().getByTestId(id).isEnabled())
  await kit().getByTestId(id).click()
}
/** Whether the comments panel shows the person's comment. */
const commented = () => until(async () => (await kit().getByTestId('note-comment-panel').innerText()).includes(COMMENT))
const version = () => page.evaluate((path) => window.devHost.files.get(path)?.version ?? 0, PATH)

const MINE = ' Mine first.'
const MEANWHILE = ' Typed meanwhile.'
const COMMENT = 'Is this callout needed?'
const CLASH = 'From a colliding draft.\n\n'
const DRAFTED = 'Drafted by the agent. '
const LATER = ' After the checkpoint.'

try {
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/dev.html?locale=en-US&replica=1&checkpoints=1`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 90000 })
  await until(async () => (await canvasReady(page)) && (await engineText()).includes('# Sample note'), 300)

  // a. The person types; the agent's draft inserts a sentence before "A note is"; the comparison
  // lists it and changes nothing; meanwhile the person comments in the panel; the draft is accepted.
  await typeAfter('They stay as you wrote them.', MINE)
  const draftText = await engineText(true)
  await run('text.insert', { at: [...draftText.slice(0, draftText.indexOf('A note is'))].length, text: DRAFTED }, { draft: true })
  step('the draft holds the agent\'s sentence', (await engineText(true)).includes(`${DRAFTED}A note is`))
  step('the note does not, yet', !(await engineText()).includes(DRAFTED) && (await shows([MINE], [DRAFTED])))
  const compared = await page.evaluate(() => window.devHost.connection().draft.compare())
  const listed = compared.summary.texts.some((change) => change.after.includes(DRAFTED) && !change.before.includes(DRAFTED))
  step('the comparison lists the draft\'s sentence', listed, JSON.stringify(compared.summary).slice(0, 400))
  step('and the comparison changes nothing in the note', !(await engineText()).includes(DRAFTED) && (await shows([MINE], [DRAFTED])))
  await focusCanvas(page)
  await selectText(page, run, engineText, 'Callouts')
  await kit().getByTestId('note-comments-toggle').click()
  await kit().getByTestId('note-comments-new').click()
  const composer = kit().locator('[data-testid="note-comment-panel"] textarea').first()
  await composer.fill(COMMENT)
  await composer.press('ControlOrMeta+Enter')
  step('the person\'s comment shows in the panel', await commented())
  const merged = await page.evaluate(() => window.devHost.connection().draft.merge({ kind: 'agent', principal: 'agent:test' }))
  step('the draft merges', merged.merged === true, JSON.stringify(merged).slice(0, 300))
  step('the editor shows the person\'s edits and the draft\'s', (await shows([MINE, `${DRAFTED}A note is`])) && (await commented()), (await shown()).slice(0, 400))
  const withDraft = await engineText()

  // b. One Undo takes back the draft's sentence only; Redo brings it back.
  await press('note-undo')
  step('one Undo takes back the draft only', (await shows([MINE], [DRAFTED])) && (await commented()), (await shown()).slice(0, 400))
  step('the note is the person\'s text', (await engineText()) === withDraft.replace(DRAFTED, ''))
  await press('note-redo')
  step('Redo brings the draft back', (await shows([MINE, `${DRAFTED}A note is`])) && (await commented()))
  step('the note is as merged', (await engineText()) === withDraft)

  // c. A draft that writes the text the person also typed into meanwhile is refused, saying why
  // (rule D5), and changes nothing; the person discards it.
  const conflictText = await engineText(true)
  await run('text.insert', { at: [...conflictText.slice(0, conflictText.indexOf('## Tasks'))].length, text: CLASH }, { draft: true })
  await typeAfter('Open the note', MEANWHILE)
  const typed = await engineText()
  const refused = await page.evaluate(() => window.devHost.connection().draft.merge({ kind: 'agent', principal: 'agent:test' }).then(() => 'merged', (error) => String(error?.message ?? error)))
  step('a draft colliding with the person\'s typing is refused, saying why', /collides/.test(refused) && /D5/.test(refused), refused)
  step('and the note is unchanged', (await engineText()) === typed && (await shows([MEANWHILE, MINE, DRAFTED], [CLASH])) && (await commented()))
  await page.evaluate(() => window.devHost.connection().draft.discard())

  // d. A labelled checkpoint of the saved note; a later edit; the checkpoint restored; Undo.
  const before = await version()
  await page.evaluate(() => window.devHost.connection().save())
  await until(async () => (await version()) > before)
  await until(() => page.evaluate((path) => window.devHost.history.replica(path) !== null, PATH))
  const saved = await engineText()
  const checkpoint = await page.evaluate(() => window.devHost.recordCheckpoint({ origin: 'kit', label: 'Before review' }))
  step('a labelled checkpoint is recorded', Boolean(checkpoint?.id), JSON.stringify(checkpoint))
  await typeAfter('Edit one cell of the table', LATER)
  const later = await engineText()
  await page.evaluate((id) => window.devHost.restoreCheckpoint(id), checkpoint.id)
  step('the restore shows the checkpoint\'s note', await shows([MINE, MEANWHILE, DRAFTED], [LATER]), (await shown()).slice(0, 400))
  step('the note is the checkpoint\'s text', (await engineText()) === saved)
  await press('note-undo')
  step('one Undo brings the later edit back', await shows([LATER, DRAFTED]))
  step('the note is as before the restore', (await engineText()) === later)
  step('no page errors', errors.length === 0, errors.slice(0, 5).join(' | '))
} catch (error) {
  failures += 1
  console.log(`FAIL ${which}: ${error instanceof Error ? error.message.split('\n')[0] : String(error)}`)
} finally {
  await browser.close()
  await server.close()
}
process.exit(failures ? 1 : 0)
