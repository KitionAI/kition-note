// Chinese input as Obsidian takes it: the same IME composition (marked text, then the
// committed characters, over CDP) in a running Obsidian and in the note editor, from the same
// text and selection; prints both outcomes and whether they match.
//
//   OBSIDIAN_WS=ws://127.0.0.1:9333/devtools/browser/… node tests/compare-ime.mjs [filter]
//
// Exits 1 when an outcome differs.
import { chromium } from 'playwright'
import { createServer } from 'vite'

const filter = process.argv[2] ?? ''
/** [name, before (‸ caret, ‹…› selection), steps: ['compose', text] | ['commit', text] | ['key', key]]. */
const SCENARIOS = [
  ['commits in a paragraph', 'start ‸end', [['compose', 'zh'], ['compose', 'zhong'], ['commit', '中文']]],
  ['commits over a selection', 'a ‹old› b', [['compose', 'xin'], ['commit', '新']]],
  ['commits at a bullet', '- ‸', [['compose', 'li'], ['commit', '列表']]],
  ['commits then Enter continues the list', '- 一‸', [['compose', 'er'], ['commit', '二'], ['key', 'Enter']]],
  ['commits inside a pair', 'x ‸', [['key', 'type:('], ['compose', 'kuo'], ['commit', '括号']]],
  ['commits full-width punctuation', '你好‸', [['commit', '，'], ['commit', '世界'], ['commit', '。']]],
  ['commits in a heading', '# 标题‸', [['compose', 'yi'], ['commit', '一']]],
  ['Backspace after a commit', '文‸', [['compose', 'zi'], ['commit', '字'], ['key', 'Backspace']]],
  ['commits then Undo', '开始‸', [['compose', 'ce'], ['commit', '测试'], ['key', 'Meta+z']]],
  // Not compared: a table cell (Obsidian's table widget pads every column again on each edit;
  // the note changes only the line edited, which its table gate holds it to).
  ['commits a 【 bracket', '‸', [['commit', '【'], ['commit', '注']]],
  ['commits in a task', '- [ ] ‸', [['compose', 'ren'], ['commit', '任务']]],
  ['commits in a numbered item then Enter', '1. 一‸', [['compose', 'er'], ['commit', '二'], ['key', 'Enter']]],
  ['commits in a link text', 'a [li‸nk](u) b', [['compose', 'zhong'], ['commit', '中']]],
  ['commits in a wikilink', 'a [[No‸te]] b', [['compose', 'zhong'], ['commit', '中']]],
  ['commits in a quote list item', '> - ‸', [['compose', 'li'], ['commit', '列']]],
  ['commits in a task then Enter', '- [ ] 做‸', [['compose', 'wan'], ['commit', '完'], ['key', 'Enter']]],
  ['commits over a bold selection', 'a **‹b›** c', [['compose', 'xin'], ['commit', '新']]],
  ['commits in a heading then Backspace', '## 标‸', [['compose', 'ti'], ['commit', '题'], ['key', 'Backspace']]],
  ['commits in a code block', '```\na‸\n```', [['compose', 'ma'], ['commit', '码']]],
  ['commits a full-width bracket pair', 'x ‸', [['commit', '（'], ['commit', '）']]],
  ['two commits then Undo', 'x ‸', [['compose', 'zhong'], ['commit', '中'], ['compose', 'wen'], ['commit', '文'], ['key', 'Meta+z']]],
  ['typing then a commit then Undo', 'x ‸', [['key', 'type:ab'], ['compose', 'zhong'], ['commit', '中'], ['key', 'Meta+z']]],
  ['a commit then typing then Undo', 'x ‸', [['compose', 'zhong'], ['commit', '中'], ['key', 'type:ab'], ['key', 'Meta+z']]],
  ['a commit, Enter, a commit, Undo', 'x ‸', [['commit', '一'], ['key', 'Enter'], ['commit', '二'], ['key', 'Meta+z']]],
  ['a commit then Undo then Redo', 'x ‸', [['compose', 'ce'], ['commit', '测试'], ['key', 'Meta+z'], ['key', 'Meta+Shift+z']]],
  ['a commit over a selection then Undo', 'a ‹old› b', [['compose', 'xin'], ['commit', '新'], ['key', 'Meta+z']]],
  ['commits, a pause, a commit, Undo', 'x ‸', [['commit', '一'], ['key', 'wait:800'], ['commit', '二'], ['key', 'Meta+z']]],
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

async function steps(page, cdp, list) {
  for (const [kind, value] of list) {
    if (kind === 'compose') await cdp.send('Input.imeSetComposition', { text: value, selectionStart: value.length, selectionEnd: value.length })
    else if (kind === 'commit') await cdp.send('Input.insertText', { text: value })
    else if (value.startsWith('type:')) await page.keyboard.type(value.slice(5))
    else if (value.startsWith('wait:')) await page.waitForTimeout(Number(value.slice(5)))
    else await page.keyboard.press(value)
    await page.waitForTimeout(80)
  }
}

const obsidian = await chromium.connectOverCDP(process.env.OBSIDIAN_WS)
const app = obsidian.contexts().flatMap((context) => context.pages()).find((page) => page.url().startsWith('app://obsidian.md/index.html'))
await app.evaluate(async () => {
  const file = window.app.vault.getAbstractFileByPath('Scratch.md') ?? (await window.app.vault.create('Scratch.md', ''))
  await window.app.workspace.getLeaf(false).openFile(file)
})
const appCdp = await app.context().newCDPSession(app)

async function inObsidian(before, list) {
  const { text, anchor, focus } = parse(before)
  await app.bringToFront()
  await app.evaluate(async ([value, a, f]) => {
    const editor = window.app.workspace.getLeavesOfType('markdown')[0].view.editor
    // As compare-editing sets it: no hover editor left active, the text outside the history (no
    // renumbering), and the caret on the line before the selection (no hidden marks taken in).
    document.querySelectorAll('.popover').forEach((popover) => popover.remove())
    window.app.workspace._activeEditor = null
    const cm = editor.cm
    const Transaction = cm.state.update({}).constructor
    cm.dispatch({ changes: { from: 0, to: cm.state.doc.length, insert: value }, userEvent: 'set', annotations: [Transaction.addToHistory.of(false)] })
    // The scenario counts characters; the editor's offsets are UTF-16 units (an emoji is two).
    const units = (index) => [...value].slice(0, index).join('').length
    editor.setSelection(editor.offsetToPos(units(f)))
    await new Promise((resolve) => setTimeout(resolve, 50))
    editor.setSelection(editor.offsetToPos(units(a)), editor.offsetToPos(units(f)))
    editor.focus()
    document.querySelector('.workspace-leaf.mod-active .cm-content')?.focus()
  }, [text, anchor, focus])
  await app.waitForTimeout(150)
  await steps(app, appCdp, list)
  await app.waitForTimeout(250)
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
const box = await kit.locator('[data-testid="note-canvas"] canvas').first().boundingBox()
await page.mouse.click(box.x + box.width / 2, box.y + 40)
const pageCdp = await context.newCDPSession(page)

async function inKition(before, list) {
  const { text, anchor, focus } = parse(before)
  const now = [...(await run('text.read')).text].length
  await run('text.replace', { from: 0, to: now, text })
  await run('selection.set', { anchor, focus })
  await page.waitForTimeout(100)
  await steps(page, pageCdp, list)
  await page.waitForTimeout(300)
  const value = (await run('text.read')).text
  const selection = await run('selection.get')
  return { value, anchor: selection.anchor, head: selection.focus }
}

let differences = 0
const chosen = SCENARIOS.filter(([name]) => name.includes(filter))
for (const [name, before, list] of chosen) {
  const want = await inObsidian(before, list)
  const got = await inKition(before, list)
  const [a, b] = [mark(want.value, want.anchor, want.head), mark(got.value, got.anchor, got.head)]
  if (a !== b) differences += 1
  console.log(`${a === b ? 'SAME' : 'DIFF'} ${name}${a === b && !process.env.VERBOSE ? '' : `\n     obsidian ${JSON.stringify(a)}\n     kition   ${JSON.stringify(b)}`}`)
}
console.log(`${differences} of ${chosen.length} differ`)
await browser.close()
await server.close()
await obsidian.close()
process.exit(differences ? 1 : 0)
