/**
 * Pasting from a web page, as Obsidian does it: HTML on the clipboard becomes Markdown (headings,
 * emphasis, links, lists, tasks, tables, code), through Turndown (7.2.0, Obsidian's version) with
 * Obsidian's rules, after DOMPurify makes it safe. Markdown on the clipboard (`text/markdown`)
 * goes in as it is; HTML that Obsidian itself copied (its `<!-- obsidian -->` mark) and a paste
 * with no HTML (Cmd+Shift+V, plain text) keep the clipboard's text. Pictures inlined as large data
 * addresses are left out.
 */
import DOMPurify from 'dompurify'
import TurndownService from 'turndown'

/** The mark Obsidian puts in the HTML it copies. */
const OBSIDIAN_MARK = '<!-- obsidian -->'

/** A title or alt text on one line, as Obsidian writes them. */
const oneLine = (value: string | null) => (value ? value.replace(/(\n+\s*)+/g, '\n') : '')
const address = (value: string | null) => (value ? value.replace(/ /g, '%20').replace(/([()])/g, '\\$1') : '')
const quoted = (title: string) => (title ? ` "${title.replace(/"/g, '\\"')}"` : '')

const ALIGN: Record<string, string> = { left: ':--', right: '--:', center: ':-:' }

/** A cell's extra columns (`colspan` above 1). */
function spanned(cell: Element) {
  const span = Number.parseInt(cell.getAttribute('colspan') ?? '', 10)
  return Number.isNaN(span) ? 0 : Math.max(0, span - 1)
}

/** Whether `row` is a table's header row (every cell a `th`, at the head of the table). */
function isHeading(row: HTMLTableRowElement | null | undefined) {
  if (!row) return false
  const parent = row.parentNode as Element | null
  if (!parent) return false
  if (parent.nodeName === 'THEAD') return true
  const first = parent.firstChild === row
  const previous = parent.previousSibling
  const atHead = parent.nodeName === 'TABLE' || (parent.nodeName === 'TBODY' && (!previous || (previous.nodeName === 'THEAD' && /^\s*$/i.test(previous.textContent ?? ''))))
  return first && atHead && Array.prototype.every.call(row.childNodes, (child: Node) => child.nodeName === 'TH')
}

let service: TurndownService | null = null

/** Turndown set up as Obsidian sets it up. */
function turndown() {
  if (service) return service
  const made = new TurndownService({ headingStyle: 'atx', hr: '---', bulletListMarker: '-', codeBlockStyle: 'fenced', fence: '```', linkStyle: 'inlined' })
  made.remove(['script', 'style', 'title'])
  made.addRule('strikethrough', { filter: ['del', 's'], replacement: (content) => `~~${content}~~` })
  made.addRule('highlight', { filter: ['mark'], replacement: (content) => `==${content}==` })
  made.addRule('link', {
    filter: (node, options) => options.linkStyle === 'inlined' && node.nodeName === 'A' && !!node.getAttribute('href'),
    replacement: (content, node) => `[${content}](${address((node as Element).getAttribute('href'))}${quoted(oneLine((node as Element).getAttribute('title')))})`,
  })
  made.addRule('media', {
    filter: ['img'],
    replacement: (_content, node) => {
      const element = node as Element
      const source = address(element.getAttribute('src'))
      return source ? `![${oneLine(element.getAttribute('alt'))}](${source}${quoted(oneLine(element.getAttribute('title')))})` : ''
    },
  })
  const highlighted = /highlight-(?:text|source)-([a-z0-9]+)/
  made.addRule('highlightedCodeBlock', {
    filter: (node) => node.nodeName === 'DIV' && highlighted.test(node.className) && node.firstChild?.nodeName === 'PRE',
    replacement: (_content, node, options) => {
      const language = ((node as Element).className.match(highlighted) ?? [null, ''])[1]
      return `\n\n${options.fence}${language}\n${node.firstChild?.textContent ?? ''}\n${options.fence}\n\n`
    },
  })
  made.addRule('listItem', {
    filter: 'li',
    replacement: (content, node, options) => {
      const text = content.replace(/^\n+/, '').replace(/\n+$/, '\n').replace(/\n/gm, '\n    ')
      let marker = `${options.bulletListMarker} `
      const parent = node.parentNode as Element | null
      if (parent?.nodeName === 'OL') {
        const start = parent.getAttribute('start')
        const index = Array.prototype.indexOf.call(parent.children, node)
        marker = `${start ? Number(start) + index : index + 1}. `
      }
      return marker + text + (node.nextSibling && !/\n$/.test(text) ? '\n' : '')
    },
  })
  made.addRule('taskListItems', {
    filter: (node) => node instanceof HTMLInputElement && node.type === 'checkbox' && node.parentNode?.nodeName === 'LI',
    replacement: (_content, node) => ((node as HTMLInputElement).checked ? '[x] ' : '[ ] '),
  })
  made.addRule('tableCell', {
    filter: ['th', 'td'],
    replacement: (content, node) => {
      const cell = node as Element
      const first = Array.prototype.indexOf.call(cell.parentNode?.childNodes ?? [], cell) === 0
      return `${first ? '|' : ''}${content.trim().replace(/\|+/g, '\\|').replace(/\n\r?/g, '<br>')}|${'   |'.repeat(spanned(cell))}`
    },
  })
  made.addRule('tableRow', {
    filter: 'tr',
    replacement: (content, node) => {
      const row = node as HTMLTableRowElement
      let border = ''
      if (isHeading(row)) {
        for (let index = 0; index < row.cells.length; index += 1) {
          const cell = row.cells[index]
          const align = ALIGN[(cell.getAttribute('align') ?? '').toLowerCase()] ?? '---'
          border += `${index === 0 ? '|' : ''}${align}|${`${align}|`.repeat(spanned(cell))}`
        }
      }
      return `\n${content}${border ? `\n${border}` : ''}`
    },
  })
  made.addRule('table', {
    filter: 'table',
    replacement: (content, node) => {
      const table = node as HTMLTableElement
      const first = table.rows[0]
      let body = content
      if (!isHeading(first)) {
        let columns = first?.cells.length ?? 0
        for (const cell of Array.from(first?.cells ?? [])) columns += spanned(cell)
        body = `|${'   |'.repeat(columns)}\n|${'---|'.repeat(columns)}\n${body.replace(/^[\r\n]+/, '')}`
      }
      return `\n\n${body.replace(/[\r\n]+/g, '\n')}\n\n`
    },
  })
  made.addRule('tableSection', { filter: ['thead', 'tbody', 'tfoot'], replacement: (content) => content })
  made.escape = (text) => text
  service = made
  return made
}

/**
 * What a paste of `data` types into the note, or `null` to let the clipboard's text go in as it is.
 */
export function pastedMarkdown(data: DataTransfer | null): string | null {
  if (!data) return null
  const html = data.getData('text/html')
  if (html && html.includes(OBSIDIAN_MARK) && data.getData('text/plain')) return null
  const markdown = data.getData('text/markdown')
  if (markdown) return markdown
  if (!html) return null
  const fragment = DOMPurify.sanitize(html, { RETURN_DOM_FRAGMENT: true, FORBID_TAGS: ['iframe', 'object', 'embed', 'form'] })
  const box = document.createElement('div')
  box.appendChild(fragment)
  // A picture alone with its file on the clipboard is the file's paste.
  if (data.files.length > 0 && /^<img [^>]+>$/.test(box.innerHTML.trim())) return null
  for (const media of Array.from(box.querySelectorAll('img, audio, video'))) {
    const source = media.getAttribute('src') ?? ''
    if (source.startsWith('data:') && source.length > 1000) media.remove()
  }
  const converted = turndown().turndown(box.innerHTML.trim())
  return converted || null
}
