// The note editor in each display mode (kit-editor's EditorFrame: Menus, Single line, Classic, and
// Focus), through the development host: node tests/modes.mjs [chromium|webkit]
// kition-sdk's mode smoke (`@kition/kit-dev-host`'s modeSmoke.mjs) from the checkout beside this
// repository (or KITION_SDK). Run it with no HTTP proxy set. Single line opens on Format, a Markdown
// editor's toolbar: headings, bold, italic, strikethrough, inline code, lists, task list, quote,
// link, table, code block.
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'

const sdk = process.env.KITION_SDK ?? new URL('../../../kition-sdk', import.meta.url).pathname
const { runModeSmoke } = await import(new URL('packages/kit-dev-host/src/modeSmoke.mjs', `file://${sdk}/`).href)
const FORMAT_ROW = ['format.heading1', 'format.heading2', 'format.heading3', 'format.bold', 'format.italic', 'format.strike', 'format.code', 'list.bullet', 'list.numbered', 'list.task', 'format.quote', 'format.link', 'insert.table', 'insert.codeBlock']
const line = { tab: 'Format', primary: FORMAT_ROW.map((command) => `note-cmd-${command}`), min: 12 }
await runModeSmoke({ kit: 'kit.note', line, config: new URL('../vite.config.ts', import.meta.url).pathname, browser: process.argv[2] === 'webkit' ? webkit : chromium, createServer })
