// The note in a host with a workspace, through tests/workspace-host.html: node tests/workspace.mjs [chromium|webkit]
// The replica the host keeps with the file (capability `replica`): comments survive save, close and
// reopen, and two editors of one note co-edit through their relayed replicas.
// Prints PASS or FAIL per step; exits 1 on any FAIL. Needs the engine built into public/engine.
// Run it with no HTTP proxy set.
import { chromium, webkit } from 'playwright'
import { readFile } from 'node:fs/promises'
import { createServer } from 'vite'

const which = process.argv[2] ?? 'chromium'
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const context = await browser.newContext({ viewport: { width: 1600, height: 900 } })
// These steps check unsaved edits: the note's autosave is off (src/autosave.tsx).
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

const frameOf = async (slot) => (await page.$(`#kit-${slot}`)).contentFrame()
const run = (steps, slot = 'a') => page.evaluate(async ([steps, slot]) => (await window.workspaceHost.connection(slot).ai.run(steps, false)).completed, [steps, slot])
const command = async (name, params = {}, slot = 'a') => (await run([{ command: name, params }], slot))[0].result
const engineText = async (slot = 'a') => (await command('text.read', {}, slot)).text
/** The engine holds and draws the note: what the editor shows is the engine's text. */
const editorText = engineText
const threads = async (slot = 'a') => command('comment.threads', {}, slot)
const canvasOf = async (slot) => (await frameOf(slot)).locator('[data-testid="note-canvas"] canvas').first()
async function open(path, slot = 'a') {
  await page.evaluate(async ([path, slot]) => window.workspaceHost.open(path, slot), [path, slot])
  return until(async () => (await (await canvasOf(slot)).count()) > 0 && (await engineText(slot).catch(() => null)) !== null, 300)
}
/** A document point of `slot`'s canvas in page coordinates. */
async function pagePoint(slot, x, y) {
  const frame = await frameOf(slot)
  const [vx, vy, , , zoom] = (await frame.getByTestId('note-editor').getAttribute('data-view')).split(',').map(Number)
  const box = await (await canvasOf(slot)).boundingBox()
  return { x: box.x + (x - vx) * zoom, y: box.y + (y - vy) * zoom }
}
/** Unicode characters before the first `text` (at its `offset`) in the note. */
const charsBefore = async (text, offset = 0, slot = 'a') => {
  const all = await engineText(slot)
  return [...all.slice(0, all.indexOf(text) + offset)].length
}
/** Clicks the canvas on the character `index`, where it is drawn now (the caret stays where it is). */
async function clickChar(index, slot = 'a', modifiers = []) {
  const at = await command('selection.where', { at: index }, slot)
  const point = await pagePoint(slot, at.x + 3, at.y + at.height / 2)
  for (const key of modifiers) await page.keyboard.down(key)
  await page.mouse.click(point.x, point.y)
  for (const key of modifiers) await page.keyboard.up(key)
}
/** Puts the caret after `text` through the engine's selection, then focuses the canvas. */
async function caretAfter(text, slot = 'a') {
  const box = await (await canvasOf(slot)).boundingBox()
  await page.mouse.click(box.x + box.width - 20, box.y + 20)
  await command('selection.set', { anchor: await charsBefore(text, text.length, slot) }, slot)
  await page.keyboard.press('ArrowLeft')
  await page.keyboard.press('ArrowRight')
}
const save = (slot = 'a') => page.evaluate(async (slot) => window.workspaceHost.connection(slot).save(), slot)
const fileText = (path) => page.evaluate((path) => window.workspaceHost.text(path), path)

/** The note's KDM schema, which a host sends to merge a draft (Kition's `KitDraftReview` does). */
const SCHEMA = JSON.parse(await readFile(new URL('../../crates/kdm/kdm-model/model/schema.json', import.meta.url), 'utf8'))
const NOTE = '# Plan\n\nGoals for the quarter.\n\nClosing words.\n'

try {
  await page.goto(`${origin}/tests/workspace-host.html`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent === 'ready', null, { timeout: 90000 })
  await page.evaluate((text) => window.workspaceHost.write('Plan.md', text), NOTE)
  step('opens the note', await open('Plan.md'))
  step('the editor lists the replica capability', (await page.evaluate(() => window.workspaceHost.connection('a').capabilities)).includes('replica'))

  // A comment is kept in the replica, never in the file, and comes back with the note.
  await run([{ command: 'comment.add', params: { quote: 'Goals for the quarter', body: 'Which quarter?' } }])
  const saved = await save()
  step('saves the note', saved.saved === true, JSON.stringify(saved))
  step('the comment stays out of the file', (await fileText('Plan.md')) === NOTE, JSON.stringify(await fileText('Plan.md')))
  step('the host keeps the replica with the file', await page.evaluate(() => window.workspaceHost.replicas.has('Plan.md')))
  await page.evaluate(() => window.workspaceHost.close('a'))
  step('reopens the note', await open('Plan.md'))
  const kept = JSON.stringify(await threads())
  step('the comment survives save, close and reopen', kept.includes('Which quarter?'), kept)
  const count = (await frameOf('a')).getByTestId('note-comments-count')
  step('the comments count shows it', await until(async () => (await count.count()) === 1 && (await count.innerText()) === '1'))

  // A change made outside while the note was closed merges into the stored replica: both stay.
  await page.evaluate(() => window.workspaceHost.close('a'))
  await page.evaluate((text) => window.workspaceHost.write('Plan.md', text), NOTE.replace('Closing words.', 'Closing words, edited outside.'))
  step('reopens the note changed outside', await open('Plan.md'))
  step('the note shows the change made outside', (await editorText()).includes('Closing words, edited outside.'), JSON.stringify(await editorText()))
  step('and keeps its comment', JSON.stringify(await threads()).includes('Which quarter?'))

  // Two editors of one note: typing in one reaches the other, and a comment in the other reaches the first.
  await save()
  step('opens the note in a second editor', await open('Plan.md', 'b'))
  step('the second editor has the comment', JSON.stringify(await threads('b')).includes('Which quarter?'))
  await page.evaluate(() => window.workspaceHost.coEdit())
  await caretAfter('Goals for the quarter.', 'a')
  await page.keyboard.type(' From A.')
  step('typing in one editor reaches the other', await until(async () => (await editorText('b')).includes('Goals for the quarter. From A.')), JSON.stringify(await editorText('b').catch(() => '')))
  await run([{ command: 'comment.add', params: { quote: 'Closing words', body: 'From B.' } }], 'b')
  step('a comment in the other editor reaches the first', await until(async () => JSON.stringify(await threads('a')).includes('From B.')))
  {
    const box = await (await canvasOf('b')).boundingBox()
    await page.mouse.click(box.x + box.width - 20, box.y + 20)
  }
  await command('selection.set', { anchor: 0 }, 'b')
  await page.keyboard.press('ArrowRight')
  await page.keyboard.press('ArrowLeft')
  await page.keyboard.type('Draft: ')
  const converged = await until(async () => {
    const [a, b] = [await editorText('a'), await editorText('b')]
    return a === b && a.startsWith('Draft: # Plan') && a.includes('From A.')
  })
  step('both editors converge', converged, JSON.stringify([await editorText('a').catch(() => ''), await editorText('b').catch(() => '')]))

  // Live co-editing (managed co-editing L3): the other person's typing shows in this editor at
  // once, this person's caret stays on its word while text comes in before it, both typing in one
  // paragraph keep their runs whole, and each sees the other's caret with their name.
  {
    const shown = async (slot) => (await frameOf(slot)).getByTestId('note-text').textContent()
    await caretAfter('Closing words', 'b')
    await page.keyboard.type(' (B)')
    step('the other editor draws what this one types, live', await until(async () => (await shown('a')).includes('Closing words (B)')), JSON.stringify(await shown('a').catch(() => '')))
    // B's caret is after "(B)"; A types a line at the top.
    await caretAfter('Draft: ', 'a')
    await page.keyboard.type('Top line from A. ')
    step('the text reaches the second editor', await until(async () => (await editorText('b')).includes('Top line from A.')))
    const beforeCaret = async (slot) => {
      const got = await command('selection.get', {}, slot)
      return [...(await engineText(slot))].slice(0, got.focus).join('')
    }
    step("the second editor's caret stays on its word", (await beforeCaret('b')).endsWith('Closing words (B)'), JSON.stringify((await beforeCaret('b')).slice(-30)))
    // Both type in the same paragraph, one after the other without waiting for the relay.
    await caretAfter('Goals for the quarter.', 'a')
    await caretAfter('Goals for the quarter.', 'b')
    const sinkOf = async (slot) => (await frameOf(slot)).getByTestId('kit-text-sink')
    await (await sinkOf('a')).pressSequentially(' [alpha]')
    await (await sinkOf('b')).pressSequentially(' [beta]')
    await (await sinkOf('a')).pressSequentially(' [gamma]')
    const together = await until(async () => {
      const [a, b] = [await editorText('a'), await editorText('b')]
      return a === b && ['[alpha]', '[beta]', '[gamma]'].every((word) => a.includes(word))
    })
    step('typing in one paragraph at once converges with every word whole', together, JSON.stringify([await editorText('a').catch(() => ''), await editorText('b').catch(() => '')]))
    step('and the saved file is what both typed', (await save('a')).saved === true && (await fileText('Plan.md')) === (await editorText('b')), JSON.stringify(await fileText('Plan.md')))
    // Presence: A sees B's caret, with B's name, where B's caret is drawn in B.
    const peerCaret = (await frameOf('a')).getByTestId('note-peer-caret')
    step("one editor shows the other person's caret with their name", await until(async () => (await peerCaret.count()) === 1 && (await peerCaret.innerText()) === 'Person B'), String(await peerCaret.count()))
    const shared = await page.evaluate(() => window.workspaceHost.presences.b)
    step('the caret travels as a replica text position', shared?.caret?.object === 'note' && shared?.caret?.field === 'body' && Number.isInteger(shared?.caret?.offset), JSON.stringify(shared))
    // B selects a word: A shows B's selection.
    await command('selection.set', { anchor: await charsBefore('[beta]', 0, 'b'), focus: await charsBefore('[beta]', 6, 'b') }, 'b')
    await (await sinkOf('b')).press('Shift+ArrowRight')
    await (await sinkOf('b')).press('Shift+ArrowLeft')
    const peerSelection = (await frameOf('a')).getByTestId('note-peer-selection')
    step("one editor shows the other person's selection", await until(async () => (await peerSelection.count()) > 0), String(await peerSelection.count()))
    // SHOT=<dir> keeps a picture of the two editors there.
    if (process.env.SHOT) await page.screenshot({ path: `${process.env.SHOT}/coedit-${which}.png` })
  }

  // The agent changes an existing note through a draft the person merges: the note is
  // untouched until the merge, which is one step of the person's Undo.
  {
    await page.evaluate(() => window.workspaceHost.close('b'))
    await page.evaluate((text) => window.workspaceHost.write('Draft.md', text), NOTE)
    await open('Draft.md')
    const capabilities = await page.evaluate(() => window.workspaceHost.connection('a').capabilities)
    step('the editor serves drafts', ['draft', 'draft-model', 'model'].every((name) => capabilities.includes(name)), JSON.stringify(capabilities))
    const drafted = await page.evaluate(async () => {
      const connection = window.workspaceHost.connection('a')
      const text = (await connection.ai.run([{ command: 'text.read', params: {} }], false, { draft: true })).completed[0].result.text
      const at = text.indexOf('Closing words.')
      return connection.ai.run([{ command: 'text.insert', params: { at, text: 'Agent paragraph.\n\n' } }], false, { draft: true })
    })
    step('the agent edits a draft', drafted.completed?.length === 1, JSON.stringify(drafted).slice(0, 200))
    step('the note is untouched while the draft is open', (await editorText()) === NOTE && (await engineText()) === NOTE)
    const draftText = await page.evaluate(async () => (await window.workspaceHost.connection('a').ai.run([{ command: 'text.read', params: {} }], false, { draft: true })).completed[0].result.text)
    step('the draft holds the agent\'s change', draftText.includes('Agent paragraph.'), JSON.stringify(draftText))
    const merged = await page.evaluate(async (schema) => window.workspaceHost.connection('a').draft.merge({ kind: 'agent', principal: 'test-agent' }, schema), SCHEMA)
    const withAgent = NOTE.replace('Closing words.', 'Agent paragraph.\n\nClosing words.')
    step('merging the draft brings its change into the note', merged.merged === true && (await until(async () => (await editorText()) === withAgent && (await engineText()) === withAgent)), JSON.stringify(await editorText()))
    {
      const box = await (await canvasOf('a')).boundingBox()
      await page.mouse.click(box.x + box.width - 20, box.y + 20)
    }
    await page.keyboard.press('ControlOrMeta+z')
    step('one Undo takes the merged draft back', await until(async () => (await editorText()) === NOTE && (await engineText()) === NOTE), JSON.stringify(await editorText()))
    await page.keyboard.press('ControlOrMeta+Shift+z')
    step('Redo puts it back', await until(async () => (await editorText()) === withAgent), JSON.stringify(await editorText()))
    // A discarded draft leaves the note as it is.
    await page.evaluate(async () => window.workspaceHost.connection('a').ai.run([{ command: 'text.insert', params: { at: 0, text: 'Unwanted. ' } }], false, { draft: true }))
    await page.evaluate(async () => window.workspaceHost.connection('a').draft.discard())
    await page.waitForTimeout(300)
    step('discarding a draft leaves the note as it is', (await editorText()) === withAgent && (await engineText()) === withAgent, JSON.stringify(await editorText()))
  }

  // Links open in the host (KitLinks 1.0 section 5): a wikilink, in the text or in a table cell
  // (the mandatory gate's internal-link scenarios), a missing note offered to create, a Markdown
  // link to a file, a web address, and a heading of this note shown here.
  {
    const LINKS = [
      '# Links',
      '',
      'See [[Plan#Goals]] and [the guide](Guides/Guide%20one.md#Setup) and [the site](https://example.com/docs).',
      '',
      '| Included document | Pattern |',
      '|---|---|',
      '| [[Plan]] | Clickable internal link |',
      '| [[Missing note.md]] | Missing |',
      '',
      'Down to [[#Far away]].',
      '',
      ...Array.from({ length: 80 }, (_, i) => `Filler ${i + 1}.\n`),
      '## Far away',
      '',
      'Back to [the top](#links).',
      '',
    ].join('\n')
    await page.evaluate((text) => window.workspaceHost.write('Links.md', text), LINKS)
    await page.evaluate(() => window.workspaceHost.write('Guides/Guide one.md', '# Setup\n'))
    await open('Links.md')
    const kitFrame = await frameOf('a')
    const opens = () => page.evaluate(() => window.workspaceHost.opens)
    // The caret on the line after the link's (blank in this note), so the link is drawn as a link
    // (its syntax hidden) and stays in view.
    let target = ''
    const away = async () => {
      const all = await engineText()
      const line = all.slice(0, all.indexOf(target)).split('\n').length
      await command('selection.set', { line: line + 1, column: 1 })
      await page.waitForTimeout(150)
    }
    const clickLink = async (text, offset) => {
      target = text
      await away()
      await clickChar(await charsBefore(text, offset))
    }
    // A table cell's link: a click on it opens it (the gate's internal-link scenario).
    await clickLink('[[Plan]]', 3)
    const cellOpen = await until(async () => (await opens()).some((entry) => entry.method === 'host/open' && entry.params.link?.name === 'Plan'))
    step('a wikilink in a table cell asks the host to open its note', cellOpen, JSON.stringify(await opens()))
    // A missing note: the host answers not-found, the note offers to create it.
    await clickLink('[[Missing note.md]]', 3)
    const asked = kitFrame.getByTestId('note-link-missing-message')
    step('a missing note is offered to create', await until(async () => (await asked.count()) === 1 && (await asked.innerText()) === 'Missing note.md not found. Create a new note?'), await asked.innerText().catch(() => ''))
    await kitFrame.getByTestId('note-link-cancel').click()
    step('Cancel creates nothing', await until(async () => (await asked.count()) === 0) && !(await page.evaluate(() => window.workspaceHost.files.has('Missing note.md'))))
    await clickLink('[[Missing note.md]]', 3)
    await until(async () => (await asked.count()) === 1)
    await kitFrame.getByTestId('note-link-create').click()
    const created = await until(async () => (await opens()).some((entry) => entry.params.link?.path === 'Missing note.md' && entry.params.create?.kit === 'kit.note'))
    step('Create note asks the host to create and open it', created && (await page.evaluate(() => window.workspaceHost.files.has('Missing note.md'))), JSON.stringify((await opens()).at(-1)))
    // A wikilink in the text, with its heading as the opened note's view.
    await clickLink('[[Plan#Goals]]', 3)
    const withView = await until(async () => (await opens()).some((entry) => entry.params.link?.name === 'Plan' && JSON.stringify(entry.params.view) === JSON.stringify({ heading: ['Goals'] })))
    step('a wikilink opens its note at the heading it names', withView, JSON.stringify((await opens()).at(-1)))
    // Markdown links: a file (decoded, with its section), a web address.
    await clickLink('[the guide]', 2)
    step('a Markdown link to a file asks the host to open it', await until(async () => (await opens()).some((entry) => entry.params.link?.path === 'Guides/Guide one.md' && JSON.stringify(entry.params.view) === JSON.stringify({ heading: ['Setup'] }))), JSON.stringify((await opens()).at(-1)))
    await clickLink('[the site]', 2)
    step('a web address goes to the host', await until(async () => (await opens()).some((entry) => entry.method === 'host/openUrl' && entry.params.url === 'https://example.com/docs')), JSON.stringify((await opens()).at(-1)))
    // On the caret's own line a link shows its Markdown; Cmd/Ctrl+click opens it there too.
    await command('selection.set', { anchor: await charsBefore('See [[Plan') })
    const before = (await opens()).length
    await clickChar(await charsBefore('[[Plan#Goals]]', 4), 'a', [process.platform === 'darwin' ? 'Meta' : 'Control'])
    step('Cmd/Ctrl+click opens a link shown as Markdown', await until(async () => (await opens()).length > before), JSON.stringify((await opens()).at(-1)))
    // A heading of this note shows here and sends nothing.
    const sent = (await opens()).length
    const line = async () => {
      const all = await engineText()
      const at = (await command('selection.get')).line
      return all.split('\n')[at - 1]
    }
    await clickLink('[[#Far away]]', 4)
    step('[[#Heading]] shows the heading of this note', await until(async () => (await line()) === '## Far away'), await line())
    await clickLink('[the top]', 2)
    step('a #anchor link shows the heading of this note', await until(async () => (await line()) === '# Links'), await line())
    step('links to this note send nothing to the host', (await opens()).length === sent)
  }

  // Workspace images: an image the note links by path shows through the host's read, and a
  // pasted one becomes a workspace file in the note's attachments folder, linked relative to it.
  {
    const PNG = 'iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEElEQVR4nGMIc70CRAwQCgAmOgW99kNaOgAAAABJRU5ErkJggg=='
    await page.evaluate((base64) => {
      const bytes = Uint8Array.from(atob(base64), (character) => character.charCodeAt(0))
      window.workspaceHost.files.set('Notes/pics/dot.png', { bytes: bytes.buffer, version: 1 })
    }, PNG)
    const IMAGES = '# Pictures\n\nA dot:\n\n![A dot](pics/dot.png)\n\nThe end.\n'
    await page.evaluate((text) => window.workspaceHost.write('Notes/Pictures.md', text), IMAGES)
    await open('Notes/Pictures.md')
    const kitFrame = await frameOf('a')
    const drawn = async () => (await command('image.wanted')).drawn
    step('an image the note links by path is drawn from the workspace', await until(async () => (await drawn()).includes('pics/dot.png'), 100), JSON.stringify(await command('image.wanted')))
    // Pasted at the end: kept in the attachments folder, linked relative to the note, drawn.
    await command('selection.set', { line: 10000, column: 1 })
    await kitFrame.evaluate((base64) => {
      const bytes = Uint8Array.from(atob(base64), (character) => character.charCodeAt(0))
      const data = new DataTransfer()
      data.items.add(new File([bytes], 'dot.png', { type: 'image/png' }))
      document.querySelector('[data-testid="kit-text-sink"]').dispatchEvent(new ClipboardEvent('paste', { clipboardData: data, bubbles: true, cancelable: true }))
    }, PNG)
    const kept = () => page.evaluate(() => [...window.workspaceHost.files.keys()].filter((path) => path.startsWith('attachments/Notes/Pictures.md/')))
    step('a pasted image becomes a file in the note\'s attachments folder', await until(async () => (await kept()).length === 1 && /\/pasted-[a-z0-9]+-1\.png$/.test((await kept())[0])), JSON.stringify(await kept()))
    const linked = await until(async () => (await editorText()).includes('](../attachments/Notes/Pictures.md/pasted-'))
    step('the note links it relative to its folder', linked, JSON.stringify(await editorText()))
    // Its line is drawn once the caret leaves it.
    await command('selection.set', { line: 1, column: 1 })
    {
      const box = await (await canvasOf('a')).boundingBox()
      await page.mouse.click(box.x + box.width - 20, box.y + 20)
    }
    step('and draws it', await until(async () => (await drawn()).some((name) => name.includes('pasted-')), 100), JSON.stringify(await command('image.wanted')))
    // Dropped on the first line: kept the same way, linked where it was dropped.
    const first = await command('selection.where', { at: 0 })
    const point = await pagePoint('a', first.x + 1, first.y + first.height / 2)
    await kitFrame.evaluate(([base64, x, y]) => {
      const bytes = Uint8Array.from(atob(base64), (character) => character.charCodeAt(0))
      const data = new DataTransfer()
      data.items.add(new File([bytes], 'dropped.png', { type: 'image/png' }))
      const target = document.querySelector('[data-testid="note-editor"]')
      const drop = new DragEvent('drop', { clientX: x, clientY: y, bubbles: true, cancelable: true, dataTransfer: data })
      target.dispatchEvent(drop)
    }, [PNG, point.x - (await (await page.$('#kit-a')).boundingBox()).x, point.y - (await (await page.$('#kit-a')).boundingBox()).y])
    step('a dropped image is kept too', await until(async () => (await kept()).length === 2), JSON.stringify(await kept()))
    step('and linked where it was dropped', await until(async () => (await editorText()).split('\n')[0].includes('![dropped](../attachments/Notes/Pictures.md/pasted-')), JSON.stringify((await editorText()).slice(0, 80)))

    // HTML pasted from a document site: its pictures are web addresses, signed to expire within
    // minutes. Each is downloaded by the host (host/fetchImage), kept in the attachments folder,
    // and linked in place of its address; one the host cannot download stays linked by address.
    const SIGNED = 'https://docs.example/uploader/x/i/Ab3dPictureId42x.jpg?g=Fx7docFileGuid9z&accessToken=eyJhbGciOiJIUzI1NiJ9.e30.sig'
    const GONE = 'https://docs.example/uploader/gone.png'
    await page.evaluate(([base64, url]) => {
      const bytes = Uint8Array.from(atob(base64), (character) => character.charCodeAt(0))
      window.workspaceHost.web.set(url, { bytes: bytes.buffer, mime: 'image/png' })
    }, [PNG, SIGNED])
    await command('selection.set', { line: 10000, column: 1 })
    await kitFrame.evaluate(([signed, gone]) => {
      const data = new DataTransfer()
      data.setData('text/html', `<ul><li>Writer</li></ul><p><img src="${signed.replaceAll('&', '&amp;')}"></p><p><img src="${gone}" alt="gone"></p>`)
      data.setData('text/plain', 'Writer')
      document.querySelector('[data-testid="kit-text-sink"]').dispatchEvent(new ClipboardEvent('paste', { clipboardData: data, bubbles: true, cancelable: true }))
    }, [SIGNED, GONE])
    step('pasted HTML asks the host for each picture by its address', await until(async () => (await page.evaluate(() => window.workspaceHost.fetched)).length === 2), JSON.stringify(await page.evaluate(() => window.workspaceHost.fetched)))
    step('a picture pasted by its web address becomes a file in the attachments folder', await until(async () => (await kept()).length === 3), JSON.stringify(await kept()))
    const pastedHtml = /!\[\]\(\.\.\/attachments\/Notes\/Pictures\.md\/pasted-[a-z0-9]+-1\.png\)/
    step('the note links the kept file, not the address', await until(async () => pastedHtml.test(await editorText()) && !(await editorText()).includes(SIGNED)), JSON.stringify((await editorText()).slice(-400)))
    step('a picture the host could not download stays linked by its address', (await editorText()).includes(`![gone](${GONE})`), JSON.stringify((await editorText()).slice(-200)))
    step('and the text around the pictures goes in', (await editorText()).includes('- Writer'), JSON.stringify((await editorText()).slice(-400)))
    const toast = (await kitFrame.locator('body').innerText()).includes('Pictures not kept: 1.')
    step('the person is told of the picture that was not kept', toast)
  }

  step('no page errors', errors.length === 0, errors.join(' | '))
} catch (error) {
  step('workspace flow ran to the end', false, error instanceof Error ? error.stack : String(error))
} finally {
  await browser.close()
  await server.close()
}
console.log(failures ? `${failures} step(s) failed in ${which}` : `all steps passed in ${which}`)
process.exit(failures ? 1 : 0)
