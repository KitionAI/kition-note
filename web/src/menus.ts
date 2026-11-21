/**
 * The note's menus: one catalog of rows, drawn by kit-editor's `EditorFrame` in every display
 * mode: drop-down menus in Menus, a generated row in Single line, the generated ribbon in Classic.
 * Rows that edit run as engine commands on the engine's selection (`format.bold`, `insert.table`;
 * their shortcuts are the engine's); `kit:` rows run here (panels, exports, find, AI).
 */
import type { MenuRow } from '@kition/sdk/editor'

const row = (path: string[], label: string, command: string, shortcut: string | null = null): MenuRow => ({ path, label, command, shortcut })
const kit = (path: string[], label: string, name: string, shortcut: string | null = null) => row(path, label, `kit:${name}`, shortcut)
const SEPARATOR = (path: string[]): MenuRow => ({ path, label: '---', command: '---' })

/** Rows the app runs itself (panels, exports, find, AI). */
export const APP_COMMANDS = ['outline', 'comments', 'export-html', 'export-pdf', 'print', 'find', 'goto-line', 'ai-improve', 'ai-shorten', 'ai-expand', 'ai-agent', 'ai-design'] as const
export type AppCommand = (typeof APP_COMMANDS)[number]

const FILE = ['File']
const EDIT = ['Edit']
const FORMAT = ['Format']
const HEADING = ['Format', 'Heading']
const INSERT = ['Insert']
const VIEW = ['View']
const AI = ['AI']

/** Every menu row, in menu order (declared in `defineKitUi`, so each has a Kit UI id). */
export const NOTE_MENUS: readonly MenuRow[] = [
  kit(FILE, 'Export as HTML…', 'export-html'),
  kit(FILE, 'Export as PDF…', 'export-pdf'),
  kit(FILE, 'Print…', 'print'),
  SEPARATOR(FILE),
  row(FILE, 'Line endings…', 'file.setLineEndings'),

  row(EDIT, 'Undo', 'edit.undo'),
  row(EDIT, 'Redo', 'edit.redo'),
  SEPARATOR(EDIT),
  kit(EDIT, 'Find', 'find', 'Mod+F'),
  row(EDIT, 'Find and replace…', 'text.findReplace'),
  kit(EDIT, 'Go to line…', 'goto-line', 'Mod+Alt+L'),
  row(EDIT, 'Select section', 'edit.selectSection'),
  SEPARATOR(EDIT),
  row(EDIT, 'Move line up', 'edit.moveLineUp'),
  row(EDIT, 'Move line down', 'edit.moveLineDown'),
  row(EDIT, 'Delete paragraph', 'edit.deleteParagraph'),
  row(EDIT, 'Sort lines', 'edit.sortLines'),
  row(EDIT, 'Trim trailing spaces', 'edit.trimWhitespace'),

  row(FORMAT, 'Bold', 'format.bold'),
  row(FORMAT, 'Italic', 'format.italic'),
  row(FORMAT, 'Strikethrough', 'format.strike'),
  row(FORMAT, 'Inline code', 'format.code'),
  row(FORMAT, 'Highlight', 'format.highlight'),
  row(FORMAT, 'Comment', 'format.comment'),
  SEPARATOR(FORMAT),
  row(FORMAT, 'Bullet list', 'list.bullet'),
  row(FORMAT, 'Numbered list', 'list.numbered'),
  row(FORMAT, 'Task list', 'list.task'),
  row(FORMAT, 'Toggle checkbox', 'list.toggleCheckbox'),
  row(FORMAT, 'Indent', 'edit.indent'),
  row(FORMAT, 'Outdent', 'edit.outdent'),
  row(FORMAT, 'Quote', 'format.quote'),
  row(HEADING, 'Body text', 'format.paragraph'),
  row(HEADING, 'Heading 1', 'format.heading1'),
  row(HEADING, 'Heading 2', 'format.heading2'),
  row(HEADING, 'Heading 3', 'format.heading3'),
  row(HEADING, 'Heading 4', 'format.heading4'),
  row(HEADING, 'Heading 5', 'format.heading5'),
  row(HEADING, 'Heading 6', 'format.heading6'),
  row(HEADING, 'Promote heading', 'format.headingUp'),
  row(HEADING, 'Demote heading', 'format.headingDown'),

  row(INSERT, 'Link', 'format.link'),
  row(INSERT, 'Wikilink', 'insert.wikilink'),
  row(INSERT, 'Embed', 'insert.embed'),
  row(INSERT, 'Image', 'insert.image'),
  row(INSERT, 'Tag', 'insert.tag'),
  SEPARATOR(INSERT),
  row(INSERT, 'Table', 'insert.table'),
  row(INSERT, 'Code block', 'insert.codeBlock'),
  row(INSERT, 'Math block', 'insert.mathBlock'),
  row(INSERT, 'Callout', 'insert.callout'),
  row(INSERT, 'Footnote', 'insert.footnote'),
  row(INSERT, 'Divider', 'insert.rule'),
  SEPARATOR(INSERT),
  row(INSERT, 'Date', 'insert.date'),
  row(INSERT, 'Date and time', 'insert.timestamp'),

  kit(VIEW, 'Outline', 'outline'),
  kit(VIEW, 'Comments', 'comments'),
  SEPARATOR(VIEW),
  row(VIEW, 'Fold heading', 'view.fold'),
  row(VIEW, 'Fold all headings', 'view.foldAll'),
  row(VIEW, 'Unfold all', 'view.unfoldAll'),
  row(VIEW, 'Fold more', 'view.foldMore'),
  row(VIEW, 'Fold less', 'view.foldLess'),

  kit(AI, 'Improve selection', 'ai-improve'),
  kit(AI, 'Shorten selection', 'ai-shorten'),
  kit(AI, 'Expand selection', 'ai-expand'),
  SEPARATOR(AI),
  kit(AI, 'Ask the agent…', 'ai-agent'),
  kit(AI, 'Make a design from this note…', 'ai-design'),
]

/** The tab the note opens on in Single line and Classic: its ribbon is the Markdown toolbar (`FormatRibbon`). */
export const FORMAT_TAB = 'Format'

/**
 * The rows each menu-built tab shows in Single line's row (kit-editor's `primary`); the rest wait in
 * its group's popover. Undo and Redo are left out: the top bar has them.
 */
export const PRIMARY_ROWS: Readonly<Record<string, readonly string[]>> = {
  File: ['kit:export-html', 'kit:export-pdf', 'kit:print'],
  Edit: ['kit:find', 'text.findReplace', 'kit:goto-line', 'edit.selectSection', 'edit.moveLineUp', 'edit.moveLineDown', 'edit.deleteParagraph'],
  Insert: ['format.link', 'insert.wikilink', 'insert.image', 'insert.table', 'insert.codeBlock', 'insert.mathBlock', 'insert.callout', 'insert.footnote', 'insert.rule'],
  View: ['kit:outline', 'kit:comments', 'view.fold', 'view.foldAll', 'view.unfoldAll'],
  AI: ['kit:ai-improve', 'kit:ai-shorten', 'kit:ai-expand', 'kit:ai-agent', 'kit:ai-design'],
}

/** Menus kept in a read-only document (File keeps its exports). */
export const VIEW_MENUS = ['File', 'View']

/** Rows that only look (kept in a read-only note). */
const LOOKING = new Set(['kit:outline', 'kit:comments', 'kit:find', 'kit:goto-line', 'kit:export-html', 'kit:export-pdf', 'kit:print', 'edit.selectSection', 'view.fold', 'view.foldAll', 'view.unfoldAll', 'view.foldMore', 'view.foldLess'])

/** The rows that change the note: hidden in a read-only note (the frame keeps only View there). */
export function isEditRow(command: string) {
  return !LOOKING.has(command)
}
