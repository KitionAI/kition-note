/**
 * The editor's context menu, as Obsidian's (its items read over its Menu API): Add link and Add
 * external link; Format, Paragraph (the caret line's kind checked), and Insert submenus of the
 * engine's commands; then Cut, Copy, Paste, Paste as plain text, and Select all. A read-only note
 * keeps Copy and Select all. Clipboard reads and writes go through the browser's clipboard (a host
 * that withholds it leaves the keyboard's Cmd+C, Cmd+X, and Cmd+V).
 */
import { useEffect, useState } from 'react'
import { ContextMenu, MenuItem, MenuSeparator, SubMenu, createKitStrings, toast, type ContextMenuPoint } from '@kition/kit-editor'
import type { Phrases } from '@kition/kit-editor'
import { pastedMarkdown } from './pasteHtml'
import { message, type NoteRun } from './useNote'

type Strings = ReturnType<typeof createKitStrings>
type Row = [string, string, string | null] | null

const FORMAT: Row[] = [
  ['Bold', 'format.bold', 'Mod+B'],
  ['Italic', 'format.italic', 'Mod+I'],
  ['Strikethrough', 'format.strike', null],
  ['Highlight', 'format.highlight', null],
  null,
  ['Code', 'format.code', null],
  ['Maths', 'format.math', null],
  ['Comment', 'format.comment', 'Mod+/'],
  null,
  ['Clear formatting', 'format.clear', null],
]
const HIGHLIGHT_COLORS: Array<[string, string]> = [
  ['Red', 'red'],
  ['Orange', 'orange'],
  ['Yellow', 'yellow'],
  ['Green', 'green'],
  ['Blue', 'blue'],
  ['Purple', 'purple'],
]
const LISTS: Row[] = [
  ['Bullet list', 'list.bullet', null],
  ['Numbered list', 'list.numbered', null],
  ['Task list', 'list.task', null],
]
const HEADINGS: Array<[string, string, number]> = [
  ['Heading 1', 'format.heading1', 1],
  ['Heading 2', 'format.heading2', 2],
  ['Heading 3', 'format.heading3', 3],
  ['Heading 4', 'format.heading4', 4],
  ['Heading 5', 'format.heading5', 5],
  ['Heading 6', 'format.heading6', 6],
  ['Body', 'format.paragraph', 0],
]
const INSERT: Row[] = [
  ['Footnote', 'insert.footnote', null],
  ['Table', 'insert.table', null],
  ['Callout', 'insert.callout', null],
  ['Horizontal rule', 'insert.rule', null],
  null,
  ['Code block', 'insert.codeBlock', null],
  ['Maths block', 'insert.mathBlock', null],
]

const MAC = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform)

/** A shortcut as the platform writes it: `⌘B` on a Mac, `Ctrl+B` elsewhere. */
export function shortcutLabel(shortcut: string | null) {
  if (!shortcut) return null
  if (!MAC) return shortcut.replace(/\b(Mod|Cmd)\b/g, 'Ctrl')
  const glyphs: Record<string, string> = { Mod: '⌘', Cmd: '⌘', Ctrl: '⌃', Alt: '⌥', Shift: '⇧' }
  return shortcut
    .split('+')
    .map((part) => glyphs[part] ?? part)
    .join('')
}

/** Copies `text` with the clipboard API, or the page's copy command where that is withheld. */
async function writeClipboard(text: string) {
  try {
    await navigator.clipboard.writeText(text)
  } catch {
    const field = document.createElement('textarea')
    field.value = text
    field.style.position = 'fixed'
    field.style.opacity = '0'
    document.body.append(field)
    field.select()
    document.execCommand('copy')
    field.remove()
  }
}

export function EditorMenu({
  at,
  onClose,
  run,
  selectedText,
  readOnly,
  onComment,
  phrase,
  t,
}: {
  at: ContextMenuPoint | null
  onClose: () => void
  run: NoteRun
  selectedText: () => Promise<string>
  readOnly: boolean
  /** A new thread in the host on the selection (a host that keeps comment threads, the person allowed to comment). */
  onComment?: () => void
  phrase: Phrases
  t: Strings
}) {
  // The caret line's heading level (0: body text), for Paragraph's check, as Obsidian's.
  const [level, setLevel] = useState<number | null>(null)
  useEffect(() => {
    if (!at) return
    setLevel(null)
    void run<{ before?: string }>('selection.get', {})
      .then((answer) => {
        const hashes = /^(#{1,6}) /.exec(answer.before ?? '')
        const marked = /^(?:\s*(?:[-*+]|\d+[.)])\s|>)/.test(answer.before ?? '')
        setLevel(hashes ? hashes[1].length : marked ? -1 : 0)
      })
      .catch(() => setLevel(null))
  }, [at, run])
  const command = (id: string) => () => {
    void run(id, {}).catch((error: unknown) => toast('error', t('commandFailed'), message(error)))
  }
  const colored = (color: string) => () => {
    void run('format.highlightColor', { color }).catch((error: unknown) => toast('error', t('commandFailed'), message(error)))
  }
  const rows = (list: Row[]) =>
    list.map((row, index) => (row ? <MenuItem key={row[1]} label={phrase(row[0])} shortcut={shortcutLabel(row[2])} onSelect={command(row[1])} testId={`note-menu-${row[1]}`} /> : <MenuSeparator key={`separator-${index}`} />))
  const copy = async () => writeClipboard(await selectedText())
  const cut = async () => {
    const text = await selectedText()
    if (!text) return
    await writeClipboard(text)
    await run('edit.cut', {})
  }
  // Paste takes a web page's HTML as Markdown (as a paste does); Paste as plain text its text.
  const paste = async (plain: boolean) => {
    try {
      if (!plain && navigator.clipboard.read) {
        const items = await navigator.clipboard.read().catch(() => [])
        for (const item of items) {
          if (!item.types.includes('text/html')) continue
          const html = await (await item.getType('text/html')).text()
          const data = new DataTransfer()
          data.setData('text/html', html)
          const markdown = pastedMarkdown(data)
          if (markdown !== null) {
            await run('selection.replace', { text: markdown })
            return
          }
        }
      }
      const text = await navigator.clipboard.readText()
      if (text) await run('selection.replace', { text, paste: true })
    } catch (error) {
      toast('error', t('commandFailed'), message(error))
    }
  }
  return (
    <ContextMenu at={at} onClose={onClose} label={phrase('Edit')} testId="note-context-menu">
      {readOnly ? null : (
        <>
          <MenuItem label={phrase('Add link')} onSelect={command('insert.wikilink')} testId="note-menu-insert.wikilink" />
          <MenuItem label={phrase('Add external link')} shortcut={shortcutLabel('Mod+K')} onSelect={command('format.link')} testId="note-menu-format.link" />
          <MenuSeparator />
          <SubMenu label="Format" path="context/format" testId="note-menu-format">
            {rows(FORMAT.slice(0, 3))}
            {/* Obsidian 1.14's Highlight: the plain one, then its six colours. */}
            <SubMenu label={phrase('Highlight')} path="context/format/highlight" testId="note-menu-highlight">
              <MenuItem label={phrase('Default')} onSelect={command('format.highlight')} testId="note-menu-format.highlight" />
              {HIGHLIGHT_COLORS.map(([label, color]) => (
                <MenuItem key={color} label={phrase(label)} onSelect={colored(color)} testId={`note-menu-highlight-${color}`} />
              ))}
            </SubMenu>
            {rows(FORMAT.slice(4))}
          </SubMenu>
          <SubMenu label="Paragraph" path="context/paragraph" testId="note-menu-paragraph">
            {rows(LISTS)}
            <MenuSeparator />
            {HEADINGS.map(([label, id, heading]) => (
              <MenuItem key={id} label={phrase(label)} checked={level === null ? undefined : level === heading} onSelect={command(id)} testId={`note-menu-${id}`} />
            ))}
            <MenuSeparator />
            <MenuItem label={phrase('Quote')} onSelect={command('format.quote')} testId="note-menu-format.quote" />
          </SubMenu>
          <SubMenu label="Insert" path="context/insert" testId="note-menu-insert">
            {rows(INSERT)}
          </SubMenu>
          <MenuSeparator />
          <MenuItem label={phrase('Cut')} shortcut={shortcutLabel('Mod+X')} onSelect={() => void cut()} testId="note-menu-cut" />
        </>
      )}
      {onComment ? (
        <>
          <MenuItem label={t('commentAddTitle')} shortcut={shortcutLabel('Mod+Alt+M')} onSelect={onComment} testId="note-menu-comment-add" />
          <MenuSeparator />
        </>
      ) : null}
      <MenuItem label={phrase('Copy')} shortcut={shortcutLabel('Mod+C')} onSelect={() => void copy()} testId="note-menu-copy" />
      {readOnly ? null : (
        <>
          <MenuItem label={phrase('Paste')} shortcut={shortcutLabel('Mod+V')} onSelect={() => void paste(false)} testId="note-menu-paste" />
          <MenuItem label={phrase('Paste as plain text')} shortcut={shortcutLabel('Mod+Shift+V')} onSelect={() => void paste(true)} testId="note-menu-paste-plain" />
        </>
      )}
      <MenuItem label={phrase('Select all')} shortcut={shortcutLabel('Mod+A')} onSelect={command('edit.selectAll')} testId="note-menu-select-all" />
    </ContextMenu>
  )
}
