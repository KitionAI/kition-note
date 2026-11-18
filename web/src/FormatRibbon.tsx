/**
 * The Format tab's ribbon in Single line and Classic (DM-note): a Markdown editor's toolbar, in
 * groups. Single line's row is the toolbar itself (headings, bold, italic, strikethrough, inline
 * code, lists, task list, quote, link, table, code block); Classic shows every Format row, plus the
 * Insert rows a Markdown toolbar keeps at hand, and gives way in steps where it is narrow. Menus
 * keep the Format menu (`menus.ts`); the rows run the same engine commands, with the menus' labels.
 */
import { CommandButton, RibbonGroup, RibbonRow, RibbonStack, useKitEditor } from '@kition/kit-editor'
import {
  BoldIcon,
  CodeIcon,
  CodeXmlIcon,
  HeadingIcon,
  HighlighterIcon,
  ImageIcon,
  ItalicIcon,
  LinkIcon,
  ListIcon,
  ListIndentDecreaseIcon,
  ListIndentIncreaseIcon,
  ListOrderedIcon,
  ListTodoIcon,
  MessageSquareDashedIcon,
  MinusIcon,
  PilcrowIcon,
  QuoteIcon,
  SquareCheckIcon,
  StrikethroughIcon,
  TableIcon,
  ChevronUpIcon,
  ChevronDownIcon,
} from '@kition/tessera-icons/react'

/** A heading level's button: "H1" in the row, its menu label as its name and tooltip. */
function HeadingButton({ level, primary }: { level: number; primary?: boolean }) {
  const { phrase } = useKitEditor()
  const name = phrase(`Heading ${level}`)
  return <CommandButton command={`format.heading${level}`} primary={primary} label={`H${level}`} aria-label={name} title={name} />
}

export function FormatRibbon() {
  const { phrase: p } = useKitEditor()
  return (
    <>
      <RibbonGroup label={p('Heading')} icon={HeadingIcon}>
        <RibbonStack>
          <RibbonRow>
            <HeadingButton level={1} primary />
            <HeadingButton level={2} primary />
            <HeadingButton level={3} primary />
          </RibbonRow>
          <RibbonRow>
            <HeadingButton level={4} />
            <HeadingButton level={5} />
            <HeadingButton level={6} />
          </RibbonRow>
          <CommandButton command="format.paragraph" icon={PilcrowIcon} label={p('Body text')} />
        </RibbonStack>
        <RibbonStack>
          <CommandButton command="format.headingUp" icon={ChevronUpIcon} label={p('Promote heading')} />
          <CommandButton command="format.headingDown" icon={ChevronDownIcon} label={p('Demote heading')} />
        </RibbonStack>
      </RibbonGroup>
      <RibbonGroup label={p('Text')} icon={BoldIcon}>
        <RibbonStack>
          <RibbonRow>
            <CommandButton command="format.bold" primary size="icon" icon={BoldIcon} label={p('Bold')} />
            <CommandButton command="format.italic" primary size="icon" icon={ItalicIcon} label={p('Italic')} />
            <CommandButton command="format.strike" primary size="icon" icon={StrikethroughIcon} label={p('Strikethrough')} />
            <CommandButton command="format.code" primary size="icon" icon={CodeIcon} label={p('Inline code')} />
          </RibbonRow>
          <CommandButton command="format.highlight" icon={HighlighterIcon} label={p('Highlight')} />
          <CommandButton command="format.comment" icon={MessageSquareDashedIcon} label={p('Comment')} />
        </RibbonStack>
      </RibbonGroup>
      <RibbonGroup label={p('Lists')} icon={ListIcon}>
        <RibbonStack>
          <CommandButton command="list.bullet" primary icon={ListIcon} label={p('Bullet list')} />
          <CommandButton command="list.numbered" primary icon={ListOrderedIcon} label={p('Numbered list')} />
          <CommandButton command="list.task" primary icon={ListTodoIcon} label={p('Task list')} />
        </RibbonStack>
        <RibbonStack>
          <CommandButton command="edit.indent" icon={ListIndentIncreaseIcon} label={p('Indent')} />
          <CommandButton command="edit.outdent" icon={ListIndentDecreaseIcon} label={p('Outdent')} />
          <CommandButton command="list.toggleCheckbox" icon={SquareCheckIcon} label={p('Toggle checkbox')} />
        </RibbonStack>
      </RibbonGroup>
      <RibbonGroup label={p('Blocks')} icon={QuoteIcon}>
        <RibbonStack>
          <CommandButton command="format.quote" primary icon={QuoteIcon} label={p('Quote')} />
          <CommandButton command="format.link" primary icon={LinkIcon} label={p('Link')} />
          <CommandButton command="insert.image" icon={ImageIcon} label={p('Image')} />
        </RibbonStack>
        <RibbonStack>
          <CommandButton command="insert.table" primary icon={TableIcon} label={p('Table')} />
          <CommandButton command="insert.codeBlock" primary icon={CodeXmlIcon} label={p('Code block')} />
          <CommandButton command="insert.rule" icon={MinusIcon} label={p('Divider')} />
        </RibbonStack>
      </RibbonGroup>
    </>
  )
}
