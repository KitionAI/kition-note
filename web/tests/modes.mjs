// The note editor in each display mode (kit-editor's EditorFrame: Menus, Single line, Classic, and
// Focus), through the development host: node tests/modes.mjs [chromium|webkit]
// kition-sdk's mode smoke (`@kition/sdk/dev-host/mode-smoke`). Run it with no HTTP proxy set.
// Single line opens on Format, a Markdown editor's toolbar: headings, bold, italic, strikethrough,
// inline code, lists, task list, quote, link, table, code block.
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'
import { runModeSmoke } from '@kition/sdk/dev-host/mode-smoke'

// Playwright's WebKit reports a Mac (navigator.platform MacIntel, from its Safari user agent) on
// every operating system, so on Linux the editor (kit-editor's isMac) waits for Cmd+\ while the
// mode smoke's ControlOrMeta presses Ctrl. Without the override the page reports the platform it
// runs on, which is what ControlOrMeta follows.
process.env.PLAYWRIGHT_NO_UA_PLATFORM = '1'

const FORMAT_ROW = ['format.heading1', 'format.heading2', 'format.heading3', 'format.bold', 'format.italic', 'format.strike', 'format.code', 'list.bullet', 'list.numbered', 'list.task', 'format.quote', 'format.link', 'insert.table', 'insert.codeBlock']
const line = { tab: 'Format', primary: FORMAT_ROW.map((command) => `note-cmd-${command}`), min: 12 }
await runModeSmoke({ kit: 'kit.note', line, config: new URL('../vite.config.ts', import.meta.url).pathname, browser: process.argv[2] === 'webkit' ? webkit : chromium, createServer })
