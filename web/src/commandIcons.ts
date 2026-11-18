/**
 * Tessera icons for the Note engine's commands that kit-editor's rules cannot read (they draw
 * the neutral command mark otherwise): its own vocabulary, by command id. Registered once, when the
 * editor loads (`main.tsx`); ribbons and menus built from engine commands draw them. The app's own
 * rows (`kit:` in `menus.ts`) are keyed by their name. Single line draws a menu tab's rows as icons
 * alone, so rows side by side never share one (Find and Find and replace, Link and Wikilink).
 */
import { registerCommandIcons } from '@kition/kit-editor'
import {
  ArrowDownIcon,
  ArrowUpIcon,
  ChevronsDownUpIcon,
  ExpandIcon,
  FileCodeIcon,
  FileSymlinkIcon,
  FileTextIcon,
  HighlighterIcon,
  ListIcon,
  ListIndentDecreaseIcon,
  ListIndentIncreaseIcon,
  ListTodoIcon,
  ListTreeIcon,
  MessageSquareTextIcon,
  MinusIcon,
  NotebookTextIcon,
  PrinterIcon,
  QuoteIcon,
  ReplaceIcon,
  ShrinkIcon,
  SigmaIcon,
  SuperscriptIcon,
} from '@kition/tessera-icons/react'

registerCommandIcons({
  "edit.indent": ListIndentIncreaseIcon,
  "edit.moveLineDown": ArrowDownIcon,
  "edit.moveLineUp": ArrowUpIcon,
  "edit.outdent": ListIndentDecreaseIcon,
  "export-html": FileCodeIcon,
  "export-pdf": FileTextIcon,
  "format.highlight": HighlighterIcon,
  "format.quote": QuoteIcon,
  "insert.callout": MessageSquareTextIcon,
  "insert.embed": FileSymlinkIcon,
  "insert.footnote": SuperscriptIcon,
  "insert.mathBlock": SigmaIcon,
  "insert.rule": MinusIcon,
  "insert.wikilink": NotebookTextIcon,
  "list.bullet": ListIcon,
  "list.task": ListTodoIcon,
  outline: ListTreeIcon,
  print: PrinterIcon,
  "text.findReplace": ReplaceIcon,
  "view.fold": ChevronsDownUpIcon,
  "view.foldAll": ShrinkIcon,
  "view.unfoldAll": ExpandIcon,
})
