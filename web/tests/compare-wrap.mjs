// Wrapped lines as Obsidian takes them: the same keys from the same text and selection in a
// running Obsidian and in the note editor, with both wrapping the text at the same places. The
// note's rows are measured first (where each character is drawn); then Obsidian's readable line
// width is set, a pixel at a time, until its rows break at the same characters. A scenario whose
// rows no width matches is skipped and said so. `‸` is the caret, `‹…›` a selection.
//
//   OBSIDIAN_WS=ws://127.0.0.1:9333/devtools/browser/… node tests/compare-wrap.mjs [filter]
//
// VERBOSE=1 prints matching outcomes too. Exits 1 when an outcome differs.
import { chromium } from 'playwright'
import { createServer } from 'vite'

const mod = 'Meta'
const filter = new RegExp(process.argv[2] ?? '')
const LONG = 'The quick brown fox jumps over the lazy dog while the five boxing wizards jump quickly and a wizard job is to vex chumps quickly in fog, then the sphinx of black quartz judges my vow and packs my box with five dozen liquor jugs before the night is over.'
const LIST = `- ${LONG}`
const BOLD = LONG.replace('boxing wizards jump quickly and a wizard', 'boxing wizards jump **quickly and a wizard**')

/** Scenarios: [name, before, keys]; the text wraps in both editors. */
const SCENARIOS = [
  ['Down keeps the column on the next row', `${LONG.slice(0, 20)}‸${LONG.slice(20)}\n\nend`, ['ArrowDown']],
  ['Down twice through rows', `${LONG.slice(0, 20)}‸${LONG.slice(20)}\n\nend`, ['ArrowDown', 'ArrowDown']],
  ['Down from the last row leaves the paragraph', `${LONG.slice(0, 200)}‸${LONG.slice(200)}\nshort line\nend`, ['ArrowDown']],
  ['Up from the last row', `${LONG.slice(0, 200)}‸${LONG.slice(200)}\n\nend`, ['ArrowUp']],
  ['Up from the line below into the last row', `${LONG}\nab‸cdefghijklmnopqrstuvwxyz`, ['ArrowUp']],
  ['Shift+Down twice keeps the column', `${LONG.slice(0, 20)}‸${LONG.slice(20)}\n\nend`, ['Shift+ArrowDown', 'Shift+ArrowDown']],
  ['Shift+Up from the last row', `${LONG.slice(0, 205)}‸${LONG.slice(205)}\n\nend`, ['Shift+ArrowUp']],
  ['Down then Up comes back', `${LONG.slice(0, 33)}‸${LONG.slice(33)}\n\nend`, ['ArrowDown', 'ArrowUp']],
  ['Down past a short row keeps the goal', `${LONG.slice(0, 60)}‸${LONG.slice(60)}\nab\n${LONG}`, ['ArrowDown', 'ArrowDown', 'ArrowDown', 'ArrowDown']],
  ['Home on a wrapped row', `${LONG.slice(0, 120)}‸${LONG.slice(120)}`, ['Home']],
  ['End on a wrapped row', `${LONG.slice(0, 120)}‸${LONG.slice(120)}`, ['End']],
  ['Cmd+Left on a wrapped row', `${LONG.slice(0, 120)}‸${LONG.slice(120)}`, [`${mod}+ArrowLeft`]],
  ['Cmd+Right on a wrapped row', `${LONG.slice(0, 120)}‸${LONG.slice(120)}`, [`${mod}+ArrowRight`]],
  ['Shift+End on a wrapped row', `${LONG.slice(0, 120)}‸${LONG.slice(120)}`, ['Shift+End']],
  ['Shift+Home on a wrapped row', `${LONG.slice(0, 120)}‸${LONG.slice(120)}`, ['Shift+Home']],
  ['Home twice on a wrapped row', `${LONG.slice(0, 120)}‸${LONG.slice(120)}`, ['Home', 'Home']],
  ['End twice on a wrapped row', `${LONG.slice(0, 120)}‸${LONG.slice(120)}`, ['End', 'End']],
  ['End then Down', `${LONG.slice(0, 20)}‸${LONG.slice(20)}`, ['End', 'ArrowDown']],
  ['End then Up from the second row', `${LONG.slice(0, 120)}‸${LONG.slice(120)}`, ['End', 'ArrowUp']],
  ['End then Home', `${LONG.slice(0, 120)}‸${LONG.slice(120)}`, ['End', 'Home']],
  ['End then Shift+Left', `${LONG.slice(0, 20)}‸${LONG.slice(20)}`, ['End', 'Shift+ArrowLeft']],
  ['End then Backspace', `${LONG.slice(0, 20)}‸${LONG.slice(20)}`, ['End', 'Backspace']],
  ['End then Right', `${LONG.slice(0, 20)}‸${LONG.slice(20)}`, ['End', 'ArrowRight']],
  ['End then typing', `${LONG.slice(0, 20)}‸${LONG.slice(20)}`, ['End', 'type:X']],
  ['Cmd+Backspace on a wrapped row', `${LONG.slice(0, 130)}‸${LONG.slice(130)}`, [`${mod}+Backspace`]],
  ['Cmd+Up from a wrapped row', `a\n${LONG.slice(0, 130)}‸${LONG.slice(130)}`, [`${mod}+ArrowUp`]],
  ['Cmd+Shift+Down from a wrapped row', `${LONG.slice(0, 130)}‸${LONG.slice(130)}\nend`, [`${mod}+Shift+ArrowDown`]],
  ['Down in a wrapped list item', `${LIST.slice(0, 22)}‸${LIST.slice(22)}\n- next`, ['ArrowDown']],
  ['Home on a wrapped list item row', `${LIST.slice(0, 130)}‸${LIST.slice(130)}`, ['Home']],
  ['Up in a wrapped list item', `${LIST.slice(0, 130)}‸${LIST.slice(130)}`, ['ArrowUp']],
  ['Down through bold at a wrap', `${BOLD.slice(0, 40)}‸${BOLD.slice(40)}\n\nend`, ['ArrowDown']],
  ['Backspace after bold at a wrap', BOLD.replace('wizard**', 'wizard**‸'), ['Backspace']],
  ['Delete before bold at a wrap', BOLD.replace('**quickly', '‸**quickly'), ['Delete']],
]

function parse(marked) {
  const out = []
  let anchor = null
  let focus = null
  for (const c of [...marked]) {
    if (c === '‸') anchor = focus = out.length
    else if (c === '‹') anchor = out.length
    else if (c === '›') focus = out.length
    else out.push(c)
  }
  return { text: out.join(''), anchor: anchor ?? out.length, focus: focus ?? anchor ?? out.length }
}

function mark(text, anchor, focus) {
  const chars = [...text]
  const [from, to] = [Math.min(anchor, focus), Math.max(anchor, focus)]
  if (from === to) return [...chars.slice(0, from), '‸', ...chars.slice(from)].join('')
  return [...chars.slice(0, from), '‹', ...chars.slice(from, to), '›', ...chars.slice(to)].join('')
}

async function act(page, keys) {
  for (const key of keys) {
    if (key.startsWith('type:')) await page.keyboard.type(key.slice(5))
    else await page.keyboard.press(key)
    await page.waitForTimeout(60)
  }
}

// Obsidian.
const obsidian = await chromium.connectOverCDP(process.env.OBSIDIAN_WS)
const app = obsidian.contexts().flatMap((context) => context.pages()).find((page) => page.url().startsWith('app://obsidian.md/index.html'))

/** Sets Obsidian's text and selection (as compare-editing does), the line width `width`, and answers the characters its rows start at. */
async function setObsidian(text, anchor, focus, width) {
  return app.evaluate(
    async ([value, a, f, width]) => {
      const editor = window.app.workspace.getLeavesOfType('markdown')[0].view.editor
      document.querySelectorAll('.popover').forEach((popover) => popover.remove())
      window.app.workspace._activeEditor = null
      document.body.style.setProperty('--file-line-width', `${width}px`)
      const cm = editor.cm
      const Transaction = cm.state.update({}).constructor
      cm.dispatch({ changes: { from: 0, to: cm.state.doc.length, insert: value }, userEvent: 'set', annotations: [Transaction.addToHistory.of(false)] })
      editor.setSelection(editor.offsetToPos(f))
      await new Promise((resolve) => setTimeout(resolve, 60))
      editor.setSelection(editor.offsetToPos(a), editor.offsetToPos(f))
      editor.focus()
      await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)))
      const starts = []
      let top = null
      for (let at = 0; at < value.length; at += 1) {
        if (value[at] === '\n') {
          top = null
          continue
        }
        const box = cm.coordsAtPos(at, 1)
        if (box && top !== null && box.top > top + 2) starts.push(at)
        if (box) top = box.top
      }
      return starts
    },
    [text, anchor, focus, width],
  )
}

async function inObsidian(text, anchor, focus, keys, width) {
  await app.bringToFront()
  await setObsidian(text, anchor, focus, width)
  await app.waitForTimeout(150)
  await act(app, keys)
  await app.waitForTimeout(200)
  return app.evaluate(() => {
    const editor = window.app.workspace.getLeavesOfType('markdown')[0].view.editor
    const value = editor.getValue()
    const chars = (offset) => [...value.slice(0, offset)].length
    return { value, anchor: chars(editor.posToOffset(editor.getCursor('anchor'))), head: chars(editor.posToOffset(editor.getCursor('head'))) }
  })
}

// The note editor.
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const browser = await chromium.launch()
const context = await browser.newContext({ viewport: { width: 1280, height: 900 } })
await context.addInitScript(() => localStorage.setItem('kit.note.autosave', 'off'))
const page = await context.newPage()
await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/dev.html?locale=en-US`)
await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 90000 })
await page.evaluate(async () => window.devHost.open('Scratch.md', new TextEncoder().encode('scratch\n').buffer))
await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes('Scratch.md open in'), null, { timeout: 90000 })
const kit = page.frames().find((frame) => frame !== page.mainFrame())
await kit.locator('[data-testid="note-canvas"] canvas').first().waitFor()
const run = (command, params = {}) => page.evaluate(async ([command, params]) => (await window.devHost.connection().ai.run([{ command, params }], false)).completed[0].result, [command, params])
const box = await kit.locator('[data-testid="note-canvas"] canvas').first().boundingBox()
await page.mouse.click(box.x + box.width / 2, box.y + 40)

/** Sets the note's text and selection, and answers the characters its rows start at. */
async function setKition(text, anchor, focus) {
  const now = [...(await run('text.read')).text].length
  await run('text.replace', { from: 0, to: now, text })
  await run('view.unfoldAll')
  await run('selection.set', { anchor, focus })
  await page.waitForTimeout(100)
  const starts = []
  let top = null
  const chars = [...text]
  for (let at = 0; at < chars.length; at += 1) {
    if (chars[at] === '\n') {
      top = null
      continue
    }
    const where = await run('selection.where', { at })
    if (top !== null && where.y > top + 2) starts.push(at)
    top = where.y
  }
  return starts
}

async function inKition(text, anchor, focus, keys) {
  await setKition(text, anchor, focus)
  await act(page, keys)
  await page.waitForTimeout(250)
  const value = (await run('text.read')).text
  const selection = await run('selection.get')
  return { value, anchor: selection.anchor, head: selection.focus }
}

/** The line width at which Obsidian's rows start where the note's do, near 700 px. */
async function matchingWidth(text, anchor, focus, starts) {
  const wanted = JSON.stringify(starts)
  for (let step = 0; step <= 80; step += 1) {
    for (const width of step ? [700 - step, 700 + step] : [700]) {
      if (JSON.stringify(await setObsidian(text, anchor, focus, width)) === wanted) return width
    }
  }
  return null
}

let differences = 0
let skipped = 0
const chosen = SCENARIOS.filter(([name]) => filter.test(name))
for (const [name, before, keys] of chosen) {
  const { text, anchor, focus } = parse(before)
  const starts = await setKition(text, anchor, focus)
  const width = await matchingWidth(text, anchor, focus, starts)
  if (width === null) {
    skipped += 1
    console.log(`SKIP ${name} (no line width wraps Obsidian's rows at ${JSON.stringify(starts)})`)
    continue
  }
  const expected = await inObsidian(text, anchor, focus, keys, width)
  const got = await inKition(text, anchor, focus, keys)
  const want = mark(expected.value, expected.anchor, expected.head)
  const have = mark(got.value, got.anchor, got.head)
  const same = want === have
  if (!same) differences += 1
  const show = (marked) => JSON.stringify(marked.replace(LONG, '‹LONG›').slice(0, 400))
  console.log(`${same ? 'SAME' : 'DIFF'} ${name} (rows at ${JSON.stringify(starts)}, ${width} px)${same && !process.env.VERBOSE ? '' : `\n     obsidian ${show(want)}\n     kition   ${show(have)}`}`)
}
console.log(`${differences} of ${chosen.length - skipped} differ${skipped ? `, ${skipped} skipped` : ''}`)
await app.evaluate(() => document.body.style.removeProperty('--file-line-width'))
await browser.close()
await server.close()
await obsidian.close()
process.exit(differences ? 1 : 0)
