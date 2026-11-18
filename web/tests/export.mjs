// The note's exports and printing in a real browser, through the development host:
// node tests/export.mjs [chromium|webkit]
// Prints PASS or FAIL per step; exits 1 on any FAIL. Needs the engine built into public/engine.
// Run it with no HTTP proxy set.
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'

const which = process.argv[2] ?? 'chromium'
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const context = await browser.newContext({ viewport: { width: 1280, height: 900 } })
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
const kit = () => page.frames().find((frame) => frame !== page.mainFrame())
const NOTE = '# Report\n\nSome **bold** text and 中文.\n\n| A | B |\n| - | - |\n| 1 | 2 |\n\n![chart](chart.png)\n'
const fileText = (name) => page.evaluate((name) => {
  const file = window.devHost.files.get(name)
  return file ? new TextDecoder().decode(new Uint8Array(file.bytes)) : null
}, name)

async function open(query) {
  await page.goto(`${origin}/dev.html?locale=en-US&modes=menus,classic,focus${query}`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 90000 })
  await page.evaluate(async (text) => window.devHost.open('Report.md', new TextEncoder().encode(text).buffer), NOTE)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes('Report.md open in '), null, { timeout: 90000 })
  return until(async () => (await kit().locator('[data-testid="note-canvas"] canvas').count()) > 0, 300)
}
async function fileMenu(row) {
  await kit().getByTestId('note-menu-File').click()
  await kit().getByTestId(`note-control-${row}`).click()
}

try {
  // A host without KitFiles: the export goes through its export write, as a new file.
  step('opens the note', await open(''))
  await fileMenu('export-html')
  await kit().getByTestId('note-export-run').click()
  step('Export as HTML writes Report.html', await until(async () => (await fileText('Report.html'))?.includes('<h1 id="report">Report</h1>')), String(await fileText('Report.html')).slice(0, 80))
  await fileMenu('export-pdf')
  step('the PDF dialog counts the pages', await until(async () => (await kit().getByTestId('note-export-pages').innerText()) === '1 pages.'))
  const gaps = await kit().getByTestId('note-export-gaps').innerText()
  step('the PDF dialog says what the pages cannot draw', gaps.includes('2 characters have no glyph') && gaps.includes('中文') && gaps.includes('1 images'), JSON.stringify(gaps))
  await kit().getByTestId('note-export-size-a4').click()
  await kit().getByTestId('note-export-run').click()
  step('Export as PDF writes Report.pdf', await until(async () => (await fileText('Report.pdf'))?.startsWith('%PDF-1.4')))
  const a4 = (await fileText('Report.pdf')) ?? ''
  step('the PDF has A4 pages', a4.includes('/MediaBox [0 0 595.28 841.89]'))
  step('Print is not offered without a printer', (await kit().getByTestId('note-control-print').count()) === 0)
  await page.keyboard.press('Escape')

  // A host with KitFiles: new files through files/create, and printing through files/print.
  step('opens the note with KitFiles', await open('&files=1'))
  await fileMenu('export-pdf')
  await kit().getByTestId('note-export-run').click()
  const created = await until(async () => [...(await page.evaluate(() => [...window.devHost.files.keys()]))].some((name) => name.endsWith('.pdf')))
  const requests = await page.evaluate(() => window.devHost.fileRequests.map((request) => request.method))
  step('Export as PDF creates a file through KitFiles', created && requests.includes('files/create') && requests.includes('files/write'), JSON.stringify(requests))
  const create = await page.evaluate(() => window.devHost.fileRequests.find((request) => request.method === 'files/create')?.params)
  step('files/create names the PDF type', create?.mime === 'application/pdf' && String(create?.suggestedName).endsWith('.pdf'), JSON.stringify(create))
  const leaks = await page.evaluate(() => window.devHost.checkNoPaths())
  step('no path leaves the kit', leaks.length === 0, JSON.stringify(leaks))
  await fileMenu('print')
  await kit().getByTestId('note-export-run').click()
  const printed = await until(async () => (await page.evaluate(() => window.devHost.prints.length)) === 1)
  step('Print hands the PDF to the host', printed && (await page.evaluate(() => window.devHost.prints[0].size)) > 500, JSON.stringify(await page.evaluate(() => window.devHost.prints)))

  // The pages: the engine draws the reading view, not grey bars per line.
  const layout = await page.evaluate(async () => (await window.devHost.connection().ai.run([{ command: 'page.layout', params: {} }], false)).completed[0].result)
  step('page.layout places the blocks with their lines', layout.pages === 1 && layout.blocks.map((block) => `${block.kind}@${block.line}`).join(' ') === 'heading@1 paragraph@3 table@5 paragraph@9', JSON.stringify(layout.blocks))
  const preview = await page.evaluate(async () => (await window.devHost.connection().ai.preview(400)).png.length)
  step('the engine renders a preview of the page', preview > 1000, String(preview))

  step('no page errors', errors.length === 0, errors.join(' | '))
} catch (error) {
  step('flow ran to the end', false, error instanceof Error ? error.stack : String(error))
} finally {
  await browser.close()
  await server.close()
}
console.log(failures ? `${failures} step(s) failed in ${which}` : `all steps passed in ${which}`)
process.exit(failures ? 1 : 0)
