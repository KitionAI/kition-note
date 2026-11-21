/**
 * The front matter as Obsidian's properties block, drawn over the canvas where the engine keeps
 * its room (`properties.view`, document pixels): a Properties heading, a row a key with its
 * type's icon, its name, and its value (tags and lists as pills, a checkbox, a date), and an
 * Add property row. Everything is edited in the block, as Obsidian's (the caret never enters the
 * front matter): a key renames its property (emptied, deletes it), a value is set where it is
 * shown (`property.set`), a list takes a new item typed after its pills, and Add property asks
 * for the new key in its place. The icon (or a right click on the row) opens the property's menu:
 * Remove (a type is the vault's in Obsidian, not the note's, so there is none to pick).
 */
import { useEffect, useState, type KeyboardEvent } from 'react'
import type { KitCanvasView } from '@kition/sdk/editor'
import { ContextMenu, MenuItem, createKitStrings, toast, type ContextMenuPoint } from '@kition/sdk/editor'
import { CalendarIcon, ChevronDownIcon, ClockIcon, CornerUpRightIcon, ExternalLinkIcon, HashIcon, LinkIcon, PencilIcon, ListIcon, PlusIcon, SquareCheckIcon, TagIcon, TextAlignStartIcon, type IconComponent } from '@kition/tessera/icons/react'
import { message, type NoteRun } from './useNote'
import { useWheelToCanvas } from './wheel'

type Strings = ReturnType<typeof createKitStrings>

export type PropertyRow = { key: string; kind: string; values: string[]; y: number; height: number; value: number }
export type PropertiesBlock = { top: number; bottom: number; left: number; width: number; end: number; rows: PropertyRow[]; collapsed?: boolean }

/** Tessera icons for the property types, a mark per kind as Obsidian has. */
const ICONS: Record<string, IconComponent> = {
  text: TextAlignStartIcon,
  tags: TagIcon,
  aliases: CornerUpRightIcon,
  list: ListIcon,
  number: HashIcon,
  checkbox: SquareCheckIcon,
  date: CalendarIcon,
  datetime: ClockIcon,
  add: PlusIcon,
}

function Icon({ name }: { name: string }) {
  const Glyph = ICONS[name] ?? ICONS.text
  return <Glyph className="note-property-icon" size={16} aria-hidden="true" />
}

/** A date as Obsidian's date field shows it (the locale's short date). */
function shownDate(value: string, time: boolean) {
  const date = new Date(time ? value : `${value}T00:00:00`)
  if (Number.isNaN(date.getTime())) return value
  return time ? date.toLocaleString(undefined, { dateStyle: 'short', timeStyle: 'short' }) : date.toLocaleDateString(undefined, { year: 'numeric', month: '2-digit', day: '2-digit' })
}

/** A value, as Obsidian draws it: a wikilink as its text in the link color. */
function Text({ value }: { value: string }) {
  const link = /^\[\[([^\]|]+)(?:\|([^\]]+))?\]\]$/.exec(value.trim())
  if (link) return <span className="note-property-link">{link[2] ?? link[1]}</span>
  // A web address is an external link, underlined with its icon after it.
  if (/^https?:\/\/\S+$/.test(value.trim()))
    return (
      <span className="note-property-url">
        {value}
        <ExternalLinkIcon className="note-property-url-icon" size={12} aria-hidden="true" />
      </span>
    )
  return <span>{value}</span>
}

/** Enter commits a field (by leaving it), Escape puts its text back; the editor's keys stay out. */
function fieldKeys(event: KeyboardEvent<HTMLElement>, original: string) {
  event.stopPropagation()
  if (event.key === 'Enter') {
    event.preventDefault()
    event.currentTarget.blur()
  } else if (event.key === 'Escape') {
    event.preventDefault()
    if (event.currentTarget instanceof HTMLInputElement) event.currentTarget.value = original
    else event.currentTarget.textContent = original
    event.currentTarget.blur()
  }
}

/** A value as it is typed: a number for a number property, else its text. */
function typed(kind: string, text: string): string | number {
  return kind === 'number' && /^-?\d+(\.\d+)?$/.test(text.trim()) ? Number(text.trim()) : text
}

/** A text value edited where it is shown (Obsidian's long text field), its placeholder when empty. */
function TextField({ value, kind, empty, readOnly, commit }: { value: string; kind: string; empty: string; readOnly: boolean; commit: (value: string | number) => void }) {
  const [editing, setEditing] = useState(false)
  return (
    <span
      className="note-property-text"
      data-testid="note-property-text"
      contentEditable={!readOnly}
      suppressContentEditableWarning
      spellCheck
      data-placeholder={empty}
      onFocus={() => setEditing(true)}
      onKeyDown={(event) => fieldKeys(event, value)}
      onBlur={(event) => {
        setEditing(false)
        const next = event.currentTarget.textContent ?? ''
        if (next !== value) commit(typed(kind, next))
      }}
    >
      {/* A wikilink or a web address shows as a link until it is edited; anything else is its
          own text throughout. */}
      {editing || !value || !/^(\[\[|https?:\/\/)/.test(value.trim()) ? value : <Text value={value} />}
    </span>
  )
}

/** A list's new item, typed after its pills: Enter adds it, Escape drops it. */
function NewItem({ add }: { add: (item: string) => void }) {
  return (
    <span
      className="note-property-new"
      data-testid="note-property-new"
      contentEditable
      suppressContentEditableWarning
      onKeyDown={(event) => {
        event.stopPropagation()
        if (event.key === 'Enter') {
          event.preventDefault()
          const item = (event.currentTarget.textContent ?? '').trim()
          event.currentTarget.textContent = ''
          if (item) add(item)
        } else if (event.key === 'Escape') {
          event.currentTarget.textContent = ''
          event.currentTarget.blur()
        }
      }}
    />
  )
}

export function Properties({ block, view, run, readOnly, t, addRequest = 0, onCancel }: { block: PropertiesBlock | null; view: KitCanvasView | null; run: NoteRun; readOnly: boolean; t: Strings; addRequest?: number; onCancel?: () => void }) {
  const root = useWheelToCanvas()
  const [adding, setAdding] = useState(false)
  // Mod+; (Obsidian's Add file property) asks for a new property's key, as Add property does.
  useEffect(() => {
    if (addRequest > 0 && !readOnly) setAdding(true)
  }, [addRequest, readOnly])
  const [menu, setMenu] = useState<(ContextMenuPoint & { key: string }) | null>(null)
  if (!block || !view) return null
  const fail = (error: unknown) => toast('error', t('commandFailed'), message(error))
  const set = (key: string, value: unknown) => void run('property.set', { key, value }).catch(fail)
  const rename = (key: string, to: string) => {
    const name = to.trim()
    if (name === key) return
    void (name ? run('property.rename', { key, to: name }) : run('property.delete', { key })).catch(fail)
  }
  const style = { transform: `translate(${(block.left - 4 - view.x) * view.zoom}px, ${(block.top - view.y) * view.zoom}px) scale(${view.zoom})`, width: block.width + 4 }
  const addTop = block.bottom - block.top - 8 - 32 - 29
  return (
    <div ref={root} className="note-properties" data-testid="note-properties" style={style} onMouseDown={(event) => event.stopPropagation()}>
      {/* Obsidian's heading folds the block to it (Toggle fold properties; nothing is written):
          its chevron shows while the block is folded, and under the pointer. */}
      <div
        className={`note-properties-heading${block.collapsed ? ' is-collapsed' : ''}`}
        data-testid="note-properties-heading"
        onClick={() => void run('view.foldProperties', {}).catch(fail)}
      >
        <span className="note-properties-fold" aria-hidden="true">
          <ChevronDownIcon size={10} />
        </span>
        {t('properties.heading')}
      </div>
      {block.collapsed ? null : block.rows.map((row) => (
        <div
          key={`${row.key}|${row.values.join("|")}`}
          className="note-property"
          data-testid="note-property"
          data-key={row.key}
          // A row as tall as the engine keeps it (28 px for a field: a number, a checkbox, a date).
          style={{ top: row.y - block.top, height: row.height }}
          onContextMenu={(event) => {
            event.preventDefault()
            if (!readOnly) setMenu({ x: event.clientX, y: event.clientY, key: row.key })
          }}
        >
          <button
            type="button"
            className="note-property-icon-button"
            data-testid="note-property-icon"
            aria-label={row.key}
            disabled={readOnly}
            onClick={(event) => setMenu({ x: event.clientX, y: event.clientY, key: row.key })}
          >
            <Icon name={row.kind} />
          </button>
          <input
            className="note-property-key"
            data-testid="note-property-key"
            defaultValue={row.key}
            readOnly={readOnly}
            aria-label={row.key}
            spellCheck={false}
            onKeyDown={(event) => fieldKeys(event, row.key)}
            onBlur={(event) => rename(row.key, event.currentTarget.value)}
          />
          <span className="note-property-value" data-kind={row.kind}>
            {row.kind === 'checkbox' ? (
              <input
                type="checkbox"
                className="note-property-checkbox"
                checked={row.values[0] === 'true'}
                disabled={readOnly}
                onChange={(event) => set(row.key, event.target.checked)}
              />
            ) : row.kind === 'tags' || row.kind === 'aliases' || row.kind === 'list' ? (
              <>
                {row.values.map((value) => (
                  <span key={value} className={`note-property-pill${row.kind === 'tags' ? ' is-tag' : ''}`}>
                    <Text value={value} />
                    {readOnly ? null : (
                      <button
                        type="button"
                        className="note-property-remove"
                        aria-label={t('properties.remove')}
                        onClick={() => set(row.key, row.values.filter((other) => other !== value))}
                      >
                        ×
                      </button>
                    )}
                  </span>
                ))}
                {readOnly ? null : <NewItem add={(item) => !row.values.includes(item) && set(row.key, [...row.values, item])} />}
              </>
            ) : row.kind === 'date' || row.kind === 'datetime' ? (
              <span
                className="note-property-date"
                // The calendar icon opens the field's picker (the browser's own button is hidden).
                onClick={(event) => {
                  const field = event.currentTarget.querySelector('input')
                  if (field && event.target !== field) field.showPicker?.()
                }}
              >
                <Icon name="date" />
                {readOnly ? (
                  shownDate(row.values[0] ?? '', row.kind === 'datetime')
                ) : (
                  <input
                    className="note-property-date-input"
                    type={row.kind === 'datetime' ? 'datetime-local' : 'date'}
                    defaultValue={row.values[0] ?? ''}
                    onKeyDown={(event) => event.stopPropagation()}
                    onChange={(event) => event.currentTarget.value && set(row.key, event.currentTarget.value)}
                  />
                )}
                {/* Obsidian's Open daily note button after a date (Kition has no daily notes: its look only). */}
                {row.kind === 'date' ? (
                  <span className="note-property-date-link" aria-hidden="true">
                    <LinkIcon size={14} />
                  </span>
                ) : null}
              </span>
            ) : (
              <>
                <TextField value={row.values[0] ?? ''} kind={row.kind} empty={t('properties.empty')} readOnly={readOnly} commit={(value) => set(row.key, value)} />
                {/* A link value's edit button at the cell's end, under the pointer (Obsidian's flair). */}
                {!readOnly && /^(\[\[|https?:\/\/)/.test((row.values[0] ?? '').trim()) ? (
                  <button
                    type="button"
                    className="note-property-flair"
                    data-testid="note-property-flair"
                    aria-label={t('properties.edit')}
                    onMouseDown={(event) => event.preventDefault()}
                    onClick={(event) => (event.currentTarget.previousElementSibling as HTMLElement | null)?.focus()}
                  >
                    <PencilIcon size={14} />
                  </button>
                ) : null}
              </>
            )}
          </span>
        </div>
      ))}
      <ContextMenu at={menu} onClose={() => setMenu(null)} label={t('properties.heading')} testId="note-property-menu">
        <MenuItem
          label={t('properties.remove')}
          onSelect={() => {
            if (menu) void run('property.delete', { key: menu.key }).catch(fail)
          }}
          testId="note-property-remove"
        />
      </ContextMenu>
      {readOnly || block.collapsed ? null : adding ? (
        // The new property's key, in Add property's place: named, it is added (empty).
        <div className="note-property note-property-adding" style={{ top: addTop }}>
          <Icon name="text" />
          <input
            className="note-property-key"
            data-testid="note-property-new-key"
            autoFocus
            spellCheck={false}
            onKeyDown={(event) => fieldKeys(event, '')}
            onBlur={(event) => {
              const key = event.currentTarget.value.trim()
              setAdding(false)
              if (key) set(key, null)
              else onCancel?.()
            }}
          />
          <span className="note-property-value">
            <span className="note-property-text" data-placeholder={t('properties.empty')} />
          </span>
        </div>
      ) : (
        <button type="button" className="note-properties-add" data-testid="note-properties-add" style={{ top: addTop }} onClick={() => setAdding(true)}>
          <Icon name="add" />
          {t('properties.add')}
        </button>
      )}
    </div>
  )
}
