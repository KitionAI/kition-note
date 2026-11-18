// Records how Obsidian colors fenced code: opens the code corpus in a running Obsidian
// (over its DevTools port), reads each code line's color per character from its live preview,
// and writes the view crate's fixture (`code-tokens.txt`), which the highlighter's tests hold it to.
//
//   OBSIDIAN_WS=ws://127.0.0.1:9333/devtools/browser/… OBSIDIAN_VAULT=<vault> \
//     node tests/obsidian-code-tokens.mjs
//
// Start Obsidian with its own profile and a test vault, never the person's.
import { copyFile, readFile, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { chromium } from 'playwright'

const fixtures = new URL('../../crates/view/view/fixtures/', import.meta.url).pathname
const corpora = ['code-corpus.md', 'code-corpus-2.md', 'code-corpus-3.md', 'code-corpus-4.md', 'code-corpus-5.md', 'code-corpus-6.md', 'code-corpus-7.md', 'code-corpus-8.md', 'code-corpus-9.md', 'code-corpus-10.md', 'code-corpus-11.md', 'code-corpus-12.md', 'code-corpus-13.md', 'code-corpus-14.md', 'code-corpus-15.md', 'code-corpus-16.md', 'code-corpus-17.md', 'code-corpus-18.md', 'code-corpus-19.md', 'code-corpus-20.md', 'code-corpus-21.md', 'code-corpus-22.md', 'code-corpus-23.md', 'code-corpus-24.md', 'code-corpus-25.md', 'code-corpus-26.md', 'code-corpus-27.md', 'code-corpus-28.md', 'code-corpus-29.md', 'code-corpus-30.md', 'code-corpus-31.md', 'code-corpus-32.md', 'code-corpus-33.md', 'code-corpus-34.md', 'code-corpus-35.md', 'code-corpus-36.md', 'code-corpus-37.md', 'code-corpus-38.md', 'code-corpus-39.md', 'code-corpus-40.md', 'code-corpus-41.md', 'code-corpus-42.md', 'code-corpus-43.md', 'code-corpus-44.md', 'code-corpus-45.md', 'code-corpus-46.md', 'code-corpus-47.md', 'code-corpus-48.md']
const vault = process.env.OBSIDIAN_VAULT
/** Obsidian's code colors (light theme), by the letter the fixture writes for each. */
const LETTERS = {
  'rgb(34, 34, 34)': '.',
  'rgb(213, 57, 132)': 'k',
  'rgb(8, 185, 78)': 's',
  'rgb(120, 82, 238)': 'n',
  'rgb(171, 171, 171)': 'c',
  'rgb(0, 191, 188)': 'v',
  'rgb(233, 49, 71)': 'o',
  'rgb(224, 172, 0)': 'p',
  'rgb(92, 92, 92)': 'b',
  'rgb(138, 92, 245)': 'l',
}

const obsidian = await chromium.connectOverCDP(process.env.OBSIDIAN_WS)
const app = obsidian.contexts().flatMap((context) => context.pages()).find((page) => page.url().startsWith('app://obsidian.md/index.html'))
const out = {}
for (const name of corpora) {
  const source = path.join(fixtures, name)
  const text = await readFile(source, 'utf8').catch(() => null)
  if (text === null) continue
  await copyFile(source, path.join(vault, name))
  await app.waitForTimeout(500)
  await app.evaluate(async (file) => {
    await window.app.workspace.openLinkText(file, '', false)
    window.app.workspace.activeEditor.editor.setCursor(0, 0)
  }, name)
  await app.waitForTimeout(1000)
  const lines = {}
  for (let y = 0; y < 40000; y += 300) {
    const { got, end } = await app.evaluate(async ([y, letters]) => {
      const scroller = document.querySelector('.workspace-leaf.mod-active .cm-scroller')
      scroller.scrollTop = y
      await new Promise((resolve) => setTimeout(resolve, 200))
      const cm = window.app.workspace.activeEditor.editor.cm
      const got = {}
      for (const element of document.querySelectorAll('.workspace-leaf.mod-active .cm-line.HyperMD-codeblock')) {
        if (element.classList.contains('HyperMD-codeblock-begin') || element.classList.contains('HyperMD-codeblock-end')) continue
        const number = cm.state.doc.lineAt(cm.posAtDOM(element, 0)).number
        let colors = ''
        const walk = (node) => {
          for (const child of node.childNodes) {
            if (child.nodeType === 3) colors += (letters[getComputedStyle(child.parentElement).color] ?? '?').repeat(child.textContent.length)
            else if (child.nodeType === 1 && !child.classList.contains('code-block-flair')) walk(child)
          }
        }
        walk(element)
        got[number] = colors
      }
      return { got, end: scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 2 }
    }, [y, LETTERS])
    Object.assign(lines, got)
    if (end) break
  }
  out[name] = Object.entries(lines)
    .map(([number, colors]) => [Number(number), colors])
    .sort((a, b) => a[0] - b[0])
  console.log(`${name}: ${out[name].length} code lines`)
}
// One corpus after another: its name, then each code line's number and its colors, a letter a
// character (see LETTERS).
const written = Object.entries(out).flatMap(([name, lines]) => [`# ${name}`, ...lines.map(([number, colors]) => `${number}\t${colors}`)])
await writeFile(path.join(fixtures, 'code-tokens.txt'), `${written.join('\n')}\n`)
await obsidian.close()
