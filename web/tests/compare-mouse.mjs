// The mouse as Obsidian takes it: the same clicks, double and triple clicks, Shift+click,
// and drags on the same characters, in a running Obsidian and in the note editor. Fonts differ,
// so each editor is asked where a character is drawn (Obsidian's `coordsAtPos`, the engine's
// `selection.where`) and the pointer goes there; the outcomes (text and selection) are compared.
//
//   OBSIDIAN_WS=ws://127.0.0.1:9333/devtools/browser/… node tests/compare-mouse.mjs [filter]
//
// Exits 1 when an outcome differs.
import { chromium } from 'playwright'
import { createServer } from 'vite'

const filter = process.argv[2] ?? ''
// [name, text, actions]: an action is [kind, character index (the point inside it: 'mid', or
// 'end' past the line's end), …]; the caret starts at the start of the note's last line.
const SCENARIOS = [
  ['click in a word', 'hello world\n\nend', [['click', 2]]],
  ['double-click a word', 'hello world\n\nend', [['double', 8]]],
  ['double-click a CJK run', '这是中文段落测试\n\nend', [['double', 3]]],
  ['double-click a number', 'price 123.45 now\n\nend', [['double', 8]]],
  ['double-click an apostrophe word', 'we don\'t stop\n\nend', [['double', 4]]],
  // Not compared: a double click on formatted text (Obsidian's first click shows the marks,
  // and its second lands on the shifted text, selecting them too).
  ['double-click punctuation', 'a, b\n\nend', [['double', 1]]],
  ['double-click an emoji', 'a 😀😀 b\n\nend', [['double', 2]]],
  ['double-click CJK next to Latin', '中文abc测试\n\nend', [['double', 3]]],
  ['double-click a punctuation run', 'a ... b\n\nend', [['double', 3]]],
  ['double-click a hyphenated word', 'well-known fact\n\nend', [['double', 2]]],
  ['double-click a space run', 'a   b\n\nend', [['double', 2]]],
  ['double-click full-width punctuation', '你好，世界\n\nend', [['double', 2]]],
  ['double-click an underscore word', 'snake_case name\n\nend', [['double', 3]]],
  ['triple-click then Shift+click', 'one two\nthree four\n\nend', [['triple', 2], ['shift', 12]]],
  // Not compared: clicks on links and addresses (Obsidian opens them, in the system's browser).
  ['triple-click a line', 'one two\nthree four\n\nend', [['triple', 10]]],
  ['click past a line end', 'short\nlonger line here\n\nend', [['click', 2, 'end']]],
  ['Shift+click extends', 'one two three\n\nend', [['click', 1], ['shift', 10]]],
  ['drag across lines', 'first line\nsecond line\n\nend', [['drag', 2, 18]]],
  ['click a task box toggles it', '- [ ] task\n\nend', [['box', 0]]],
  // Not compared: dragging a selection to drop it (CDP's mouse makes no native drag in Obsidian;
  // measured with CodeMirror's drag events instead, in the engine's tests). `select` selects
  // characters `index..where` first.
  // Not compared: clicks in a callout or a table (Obsidian draws both as widgets with editors of
  // their own, so a click there leaves its note editor's selection where it was).
  ['click a heading', '## Heading text\n\nend', [['click', 6]]],
  ['click a quote', '> quoted text\n\nend', [['click', 5]]],
  ['click a list item', '- item text\n\nend', [['click', 5]]],
  ['click inside a selection', 'one two three\n\nend', [['select', 4, 7], ['click', 5]]],
]

const obsidian = await chromium.connectOverCDP(process.env.OBSIDIAN_WS)
const app = obsidian.contexts().flatMap((context) => context.pages()).find((page) => page.url().startsWith('app://obsidian.md/index.html'))
await app.evaluate(async () => {
  const file = window.app.vault.getAbstractFileByPath('Scratch.md') ?? (await window.app.vault.create('Scratch.md', ''))
  await window.app.workspace.getLeaf(false).openFile(file)
})

function mark(text, anchor, focus) {
  const chars = [...text]
  const [from, to] = [Math.min(anchor, focus), Math.max(anchor, focus)]
  if (from === to) return [...chars.slice(0, from), '‸', ...chars.slice(from)].join('')
  return [...chars.slice(0, from), '‹', ...chars.slice(from, to), '›', ...chars.slice(to)].join('')
}

/** Where to point in an editor: `place(index, where)` answers screen coordinates. */
async function act(page, place, actions, select) {
  for (const [kind, index, where] of actions) {
    if (kind === 'select') {
      await select(index, where)
      await page.waitForTimeout(150)
      continue
    }
    const [x, y] = await place(index, kind === 'move' ? 'mid' : where)
    if (kind === 'click' || kind === 'box') await page.mouse.click(x, y)
    else if (kind === 'move') {
      // A press in the selection, a drag, and a drop at character `where`.
      const [x2, y2] = await place(where, 'mid')
      await page.mouse.move(x, y)
      await page.mouse.down()
      await page.mouse.move(x + 4, y, { steps: 2 })
      await page.mouse.move((x + x2) / 2, (y + y2) / 2, { steps: 6 })
      await page.mouse.move(x2, y2, { steps: 6 })
      await page.waitForTimeout(100)
      await page.mouse.up()
    }
    else if (kind === 'double') await page.mouse.dblclick(x, y)
    else if (kind === 'triple') await page.mouse.click(x, y, { clickCount: 3 })
    else if (kind === 'shift') {
      await page.keyboard.down('Shift')
      await page.mouse.click(x, y)
      await page.keyboard.up('Shift')
    } else if (kind === 'drag') {
      const [x2, y2] = await place(where, 'mid')
      await page.mouse.move(x, y)
      await page.mouse.down()
      await page.mouse.move((x + x2) / 2, (y + y2) / 2, { steps: 4 })
      await page.mouse.move(x2, y2, { steps: 4 })
      await page.mouse.up()
    }
    await page.waitForTimeout(250)
  }
}

async function inObsidian(text, actions) {
  await app.bringToFront()
  await app.evaluate((value) => {
    const editor = window.app.workspace.getLeavesOfType('markdown')[0].view.editor
    // A hover editor left behind would stay the active one.
    document.querySelectorAll('.popover').forEach((popover) => popover.remove())
    window.app.workspace._activeEditor = null
    editor.setValue(value)
    editor.setCursor(editor.lastLine(), 0)
    editor.focus()
  }, text)
  await app.waitForTimeout(300)
  const place = async (index, where) =>
    app.evaluate(([index, where, value]) => {
      const view = window.app.workspace.getLeavesOfType('markdown')[0].view.editor.cm
      const offset = [...value].slice(0, index).join('').length
      if (where === 'end') {
        const line = view.state.doc.lineAt(offset)
        const end = view.coordsAtPos(line.to)
        return [end.right + 60, (end.top + end.bottom) / 2]
      }
      const box = view.coordsAtPos(offset, 1)
      const next = view.coordsAtPos(offset + 1, -1) ?? box
      // A task's box is a widget before its text: its own middle.
      if (index === 0 && value.startsWith('- [')) {
        const input = document.querySelector('.workspace-leaf.mod-active .cm-content input.task-list-item-checkbox')
        const rect = input.getBoundingClientRect()
        return [rect.left + rect.width / 2, rect.top + rect.height / 2]
      }
      return [(box.left + next.left) / 2, (box.top + box.bottom) / 2]
    }, [index, where, text])
  const select = (from, to) =>
    app.evaluate(([from, to, value]) => {
      const editor = window.app.workspace.getLeavesOfType('markdown')[0].view.editor
      const offset = (index) => [...value].slice(0, index).join('').length
      editor.setSelection(editor.offsetToPos(offset(from)))
      editor.setSelection(editor.offsetToPos(offset(from)), editor.offsetToPos(offset(to)))
    }, [from, to, text])
  await act(app, place, actions, select)
  return app.evaluate(() => {
    const editor = window.app.workspace.getLeavesOfType('markdown')[0].view.editor
    const value = editor.getValue()
    const chars = (offset) => [...value.slice(0, offset)].length
    return { value, anchor: chars(editor.posToOffset(editor.getCursor('anchor'))), head: chars(editor.posToOffset(editor.getCursor('head'))) }
  })
}

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
const canvas = kit.locator('[data-testid="note-canvas"] canvas').first()

async function inKition(text, actions) {
  const now = [...(await run('text.read')).text].length
  await run('text.replace', { from: 0, to: now, text })
  // At the last line's start, as Obsidian's caret is put.
  await run('selection.set', { anchor: [...text].length - [...text.split('\n').at(-1)].length })
  await page.waitForTimeout(400)
  const box = await canvas.boundingBox()
  const [vx, vy, , , zoom] = (await kit.getByTestId('note-editor').getAttribute('data-view')).split(',').map(Number)
  const screen = (x, y) => [box.x + (x - vx) * zoom, box.y + (y - vy) * zoom]
  const place = async (index, where) => {
    if (where === 'end') {
      const line = [...text].slice(0, index).join('').split('\n').length - 1
      const lineEnd = text.split('\n').slice(0, line + 1).join('\n').length
      const end = await run('selection.where', { at: [...text.slice(0, lineEnd)].length })
      return screen(end.x + 60, end.y + end.height / 2)
    }
    const at = await run('selection.where', { at: index })
    const next = await run('selection.where', { at: index + 1 })
    const x = next.y === at.y ? (at.x + next.x) / 2 : at.x + 3
    if (index === 0 && text.startsWith('- [')) {
      // The task's box: drawn in its `[` (16 px, 7 px in at 16 px text).
      const box = await run('selection.where', { at: 2 })
      return screen(box.x + 7 + 8, box.y + box.height / 2)
    }
    return screen(x, at.y + at.height / 2)
  }
  await act(page, place, actions, (from, to) => run('selection.set', { anchor: from, focus: to }))
  await page.waitForTimeout(300)
  const value = (await run('text.read')).text
  const selection = await run('selection.get')
  return { value, anchor: selection.anchor, head: selection.focus }
}

let differences = 0
const chosen = SCENARIOS.filter(([name]) => name.includes(filter))
for (const [name, text, actions] of chosen) {
  const want = await inObsidian(text, actions)
  const got = await inKition(text, actions)
  const [a, b] = [mark(want.value, want.anchor, want.head), mark(got.value, got.anchor, got.head)]
  if (a !== b) differences += 1
  console.log(`${a === b ? 'SAME' : 'DIFF'} ${name}${a === b && !process.env.VERBOSE ? '' : `\n     obsidian ${JSON.stringify(a)}\n     kition   ${JSON.stringify(b)}`}`)
}
console.log(`${differences} of ${chosen.length} differ`)
await browser.close()
await server.close()
await obsidian.close()
process.exit(differences ? 1 : 0)
