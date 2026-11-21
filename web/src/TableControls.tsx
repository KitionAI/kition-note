/**
 * The table controls over the engine's canvas (the GFM table widget's chrome; the engine draws the
 * grid and edits the cells): an append button at each table's bottom edge and right edge, shown
 * while its edge rail is hovered, and the column menu a right click on a header cell opens. They
 * are placed from the engine's `table.geometry` (document pixels) and the canvas's view, and run
 * the engine's `table.*` commands, each one undo step. A column's and a body row's drag handles
 * (Obsidian 1.14's, above each column and left of each row, shown under the pointer) move it:
 * the dragged column or row is outlined, a bar marks where it goes, Escape cancels.
 */
import { useCallback, useEffect, useRef, useState, type MouseEvent as ReactMouseEvent, type PointerEvent as ReactPointerEvent } from 'react'
import { GripHorizontalIcon, GripVerticalIcon, PlusIcon as Plus } from '@kition/tessera/icons/react'
import { createKitStrings, toast, type KitCanvasView } from '@kition/sdk/editor'
import { message, type NoteRun } from './useNote'

type Strings = ReturnType<typeof createKitStrings>

/** A table's grid as the engine reports it. */
export type TableGeometry = { table: number; columns: number[]; rows: Array<{ top: number; bottom: number; line: number }> }

/** Width of an edge rail, in screen pixels. */
const RAIL = 18

/** A drag handle's thickness (Obsidian's 16px handle less the 2px selection border). */
const HANDLE = 14

/** A column or a body row being dragged: where the pointer went, and the line it would go to. */
type Drag = { kind: 'col' | 'row'; table: number; from: number; start: number; pointer: number; to: number }

/** The boundary (0 to `edges.length - 1`, in screen pixels) nearest `at`. */
function nearestBoundary(edges: number[], at: number) {
  let best = 0
  edges.forEach((edge, index) => {
    if (Math.abs(edge - at) < Math.abs((edges[best] ?? 0) - at)) best = index
  })
  return best
}

type MenuState = { table: number; column: number; row: number; x: number; y: number; selected: number }

type MenuItem = { label: string; warning?: boolean; run: (table: number, column: number, row: number) => [string, Record<string, unknown>] }

/** The menu's items: a body row's first (row 0 is the header), then the column's. */
function menuItems(t: Strings, row: number): MenuItem[] {
  const rows: MenuItem[] = row > 0
    ? [
        { label: t('table.insertAbove'), run: (table, _column, at) => ['table.insertRow', { table, at }] },
        { label: t('table.insertBelow'), run: (table, _column, at) => ['table.insertRow', { table, at: at + 1 }] },
        { label: t('table.moveUp'), run: (table, _column, at) => ['table.moveRow', { table, from: at, to: Math.max(1, at - 1) }] },
        { label: t('table.moveDown'), run: (table, _column, at) => ['table.moveRow', { table, from: at, to: at + 1 }] },
        { label: t('table.deleteRow'), warning: true, run: (table, _column, at) => ['table.deleteRow', { table, row: at }] },
      ]
    : []
  return [
    ...rows,
    { label: t('table.insertLeft'), run: (table, column) => ['table.insertColumn', { table, at: column }] },
    { label: t('table.insertRight'), run: (table, column) => ['table.insertColumn', { table, at: column + 1 }] },
    { label: t('table.moveLeft'), run: (table, column) => ['table.moveColumn', { table, from: column, to: Math.max(0, column - 1) }] },
    { label: t('table.moveRight'), run: (table, column) => ['table.moveColumn', { table, from: column, to: column + 1 }] },
    { label: t('table.alignLeft'), run: (table, column) => ['table.align', { table, column, align: 'left' }] },
    { label: t('table.alignCenter'), run: (table, column) => ['table.align', { table, column, align: 'center' }] },
    { label: t('table.alignRight'), run: (table, column) => ['table.align', { table, column, align: 'right' }] },
    { label: t('table.deleteColumn'), warning: true, run: (table, column) => ['table.deleteColumn', { table, column }] },
  ]
}

/** Document point → the canvas's CSS pixels. */
const screen = (view: KitCanvasView, x: number, y: number) => ({ left: (x - view.x) * view.zoom, top: (y - view.y) * view.zoom })

export function TableControls({ tables, view, run, readOnly, t, menuAt }: { tables: TableGeometry[]; view: KitCanvasView | null; run: NoteRun; readOnly: boolean; t: Strings; menuAt: { table: number; column: number; row: number; x: number; y: number } | null }) {
  const [menu, setMenu] = useState<MenuState | null>(null)
  const [drag, setDrag] = useState<Drag | null>(null)
  // The drag as the last pointer event left it, ahead of React's render: a release that follows the
  // last move before React draws it drops where that move went, not where the drawn drag was.
  const dragNow = useRef<Drag | null>(null)
  const track = useCallback((next: Drag | null) => {
    dragNow.current = next
    setDrag(next)
  }, [])
  const menuRef = useRef<HTMLDivElement>(null)
  const items = menuItems(t, menu?.row ?? 0)

  const act = useCallback(
    (command: string, params: Record<string, unknown>) => {
      void run(command, params).catch((error: unknown) => toast('error', t('commandFailed'), message(error)))
    },
    [run, t],
  )

  // A right click on a header cell (found by the editor) opens the menu there.
  useEffect(() => {
    if (menuAt && !readOnly) setMenu({ ...menuAt, selected: 0 })
  }, [menuAt, readOnly])

  // The menu takes the keyboard; a click elsewhere, or the frame losing focus, closes it. A pointer
  // press, not a mouse one: the canvas cancels its pointer presses, so no mousedown follows them.
  const open = menu !== null
  useEffect(() => {
    if (!open) return undefined
    menuRef.current?.focus()
    const close = () => setMenu(null)
    const outside = (event: PointerEvent) => {
      if (!menuRef.current?.contains(event.target as Node)) close()
    }
    // The keys are the menu's while it is open, wherever the focus went (kit-editor's text field takes
    // it back after a click on the canvas).
    const keys = (event: KeyboardEvent) => {
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        const step = event.key === 'ArrowDown' ? 1 : items.length - 1
        setMenu((open) => (open ? { ...open, selected: (open.selected + step) % items.length } : open))
      } else if (event.key === 'Enter') {
        chosen.current?.()
      } else if (event.key === 'Escape') {
        close()
      } else {
        return
      }
      event.preventDefault()
      event.stopPropagation()
    }
    window.addEventListener('blur', close)
    window.addEventListener('pointerdown', outside, true)
    window.addEventListener('keydown', keys, true)
    return () => {
      window.removeEventListener('blur', close)
      window.removeEventListener('pointerdown', outside, true)
      window.removeEventListener('keydown', keys, true)
    }
  }, [open, items.length])

  // Escape cancels a drag (the column or row stays where it was).
  const dragging = drag !== null
  useEffect(() => {
    if (!dragging) return undefined
    const keys = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      event.stopPropagation()
      track(null)
    }
    window.addEventListener('keydown', keys, true)
    return () => window.removeEventListener('keydown', keys, true)
  }, [dragging, track])

  /** Runs the selected item (for Enter, from the window's key listener). */
  const chosen = useRef<(() => void) | null>(null)
  chosen.current = menu ? () => choose(menu.selected) : null

  const choose = (index: number) => {
    const current = menu
    const item = items[index]
    if (!current || !item) return
    setMenu(null)
    const [command, params] = item.run(current.table, current.column, current.row)
    act(command, params)
  }

  if (!view) return null
  return (
    <div className="note-tables" data-testid="note-tables">
      {readOnly
        ? null
        : tables.map((table) => {
            const left = table.columns[0] ?? 0
            const right = table.columns.at(-1) ?? left
            const top = table.rows[0]?.top ?? 0
            const bottom = table.rows.at(-1)?.bottom ?? top
            const a = screen(view, left, top)
            const b = screen(view, right, bottom)
            const columns = Math.max(1, table.columns.length - 1)
            const rows = table.rows.length
            return (
              <div
                key={table.table}
                className="note-table"
                data-testid="note-table"
                data-table={table.table}
                data-columns={columns}
                data-rows={rows}
                // The grid's lines in CSS pixels from the table's top-left corner (the header row first),
                // for tools that act on a cell (a host's tests).
                data-column-edges={table.columns.map((x) => Math.round((x - left) * view.zoom)).join(',')}
                data-row-edges={[top, ...table.rows.map((row) => row.bottom)].map((y) => Math.round((y - top) * view.zoom)).join(',')}
                style={{ left: a.left, top: a.top, width: b.left - a.left, height: b.top - a.top }}
              >
                <TableHandles
                  table={table}
                  view={view}
                  drag={drag?.table === table.table ? drag : null}
                  current={() => (dragNow.current?.table === table.table ? dragNow.current : null)}
                  start={track}
                  move={track}
                  drop={(done) => {
                    track(null)
                    // A column ends up at its boundary's place (less one when it moves right); a body row likewise.
                    const to = done.to > done.from ? done.to - 1 : done.to
                    if (done.kind === 'col' && to !== done.from) act('table.moveColumn', { table: done.table, from: done.from, to })
                    if (done.kind === 'row' && to !== done.from) act('table.moveRow', { table: done.table, from: done.from, to })
                  }}
                  t={t}
                />
                <div className="note-table-rail note-table-rail-bottom" data-testid="note-table-row-rail" style={{ top: b.top - a.top, height: RAIL }}>
                  <button type="button" className="note-table-add" data-testid="note-table-row-add" title={t('table.addRow')} aria-label={t('table.addRow')} onMouseDown={(event: ReactMouseEvent) => event.preventDefault()} onClick={() => act('table.insertRow', { table: table.table, at: rows })}>
                    <Plus size={12} />
                  </button>
                </div>
                <div className="note-table-rail note-table-rail-right" data-testid="note-table-col-rail" style={{ left: b.left - a.left, width: RAIL }}>
                  <button type="button" className="note-table-add" data-testid="note-table-col-add" title={t('table.addColumn')} aria-label={t('table.addColumn')} onMouseDown={(event: ReactMouseEvent) => event.preventDefault()} onClick={() => act('table.insertColumn', { table: table.table, at: columns })}>
                    <Plus size={12} />
                  </button>
                </div>
              </div>
            )
          })}
      {menu ? (
        <div
          ref={menuRef}
          className="note-table-menu"
          role="menu"
          tabIndex={-1}
          data-testid="note-table-menu"
          style={{ left: menu.x, top: menu.y }}
        >
          {items.map((item, index) => (
            <button
              key={item.label}
              type="button"
              role="menuitem"
              className={`note-table-menu-item${item.warning ? ' is-warning' : ''}${index === menu.selected ? ' is-selected' : ''}`}
              data-testid="note-table-menu-item"
              onMouseEnter={() => setMenu({ ...menu, selected: index })}
              onMouseDown={(event) => {
                event.preventDefault()
                choose(index)
              }}
            >
              {item.label}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  )
}

/**
 * A table's drag handles: one above each column, one left of each body row (the header row stays
 * the header), each drawn under the pointer. A press outlines its column or row; dragging moves
 * the handle along and a bar marks the boundary it goes to; release drops it there.
 */
function TableHandles({ table, view, drag, current, start, move, drop, t }: { table: TableGeometry; view: KitCanvasView; drag: Drag | null; current: () => Drag | null; start: (drag: Drag) => void; move: (drag: Drag) => void; drop: (drag: Drag) => void; t: Strings }) {
  const left = table.columns[0] ?? 0
  const top = table.rows[0]?.top ?? 0
  // Boundaries in screen pixels from the table's top-left: between columns, and between body rows.
  const columnEdges = table.columns.map((x) => (x - left) * view.zoom)
  const rowEdges = table.rows.slice(1).map((row) => (row.top - top) * view.zoom)
  rowEdges.push(((table.rows.at(-1)?.bottom ?? top) - top) * view.zoom)
  const width = (columnEdges.at(-1) ?? 0) - (columnEdges[0] ?? 0)
  const press = (kind: 'col' | 'row', index: number) => (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return
    event.preventDefault()
    event.stopPropagation()
    event.currentTarget.setPointerCapture(event.pointerId)
    const at = kind === 'col' ? event.clientX : event.clientY
    // Body rows count from 1 (row 0 is the header); their boundaries from 0 are rows 1 and on.
    start({ kind, table: table.table, from: kind === 'col' ? index : index + 1, start: at, pointer: at, to: kind === 'col' ? index : index + 1 })
  }
  // Moves and the release read the drag as the last event left it (`current`), not as last drawn.
  const motion = (event: ReactPointerEvent<HTMLDivElement>) => {
    const now = current()
    if (!now) return
    event.stopPropagation()
    const at = now.kind === 'col' ? event.clientX : event.clientY
    const box = event.currentTarget.parentElement?.getBoundingClientRect()
    if (!box) return
    const offset = at - (now.kind === 'col' ? box.left : box.top)
    const boundary = now.kind === 'col' ? nearestBoundary(columnEdges, offset) : nearestBoundary(rowEdges, offset) + 1
    move({ ...now, pointer: at, to: boundary })
  }
  const release = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.stopPropagation()
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId)
    const now = current()
    if (now) drop(now)
  }
  const cancel = () => {
    const now = current()
    if (now) move({ ...now, to: now.from })
  }
  const shift = drag ? drag.pointer - drag.start : 0
  return (
    <>
      {columnEdges.slice(0, -1).map((edge, index) => {
        const active = drag?.kind === 'col' && drag.from === index
        const span = (columnEdges[index + 1] ?? edge) - edge
        return (
          <div key={`col-${index}`}>
            {active ? <div className="note-table-selection" data-testid="note-table-selection" style={{ left: edge, top: 0, width: span, height: (rowEdges.at(-1) ?? 0) }} /> : null}
            <div
              className={`note-table-handle note-table-handle-col${active ? ' is-active' : ''}`}
              data-testid="note-table-col-handle"
              role="button"
              aria-label={t('table.moveColumn')}
              style={{ left: edge + (active ? shift : 0), top: -HANDLE, width: span, height: HANDLE }}
              onPointerDown={press('col', index)}
              onPointerMove={motion}
              onPointerUp={release}
              onPointerCancel={cancel}
              onMouseDown={(event: ReactMouseEvent) => event.preventDefault()}
            >
              <GripHorizontalIcon size={16} aria-hidden="true" />
            </div>
          </div>
        )
      })}
      {rowEdges.slice(0, -1).map((edge, index) => {
        const active = drag?.kind === 'row' && drag.from === index + 1
        const span = (rowEdges[index + 1] ?? edge) - edge
        return (
          <div key={`row-${index}`}>
            {active ? <div className="note-table-selection" data-testid="note-table-selection" style={{ left: 0, top: edge, width, height: span }} /> : null}
            <div
              className={`note-table-handle note-table-handle-row${active ? ' is-active' : ''}`}
              data-testid="note-table-row-handle"
              role="button"
              aria-label={t('table.moveRow')}
              style={{ left: -HANDLE, top: edge + (active ? shift : 0), width: HANDLE, height: span }}
              onPointerDown={press('row', index)}
              onPointerMove={motion}
              onPointerUp={release}
              onPointerCancel={cancel}
              onMouseDown={(event: ReactMouseEvent) => event.preventDefault()}
            >
              <GripVerticalIcon size={16} aria-hidden="true" />
            </div>
          </div>
        )
      })}
      {drag && drag.to !== drag.from && drag.to !== drag.from + 1 ? (
        drag.kind === 'col' ? (
          <div className="note-table-drop note-table-drop-col" data-testid="note-table-drop" style={{ left: (columnEdges[drag.to] ?? 0) - 2, top: 0, height: rowEdges.at(-1) ?? 0 }} />
        ) : (
          <div className="note-table-drop note-table-drop-row" data-testid="note-table-drop" style={{ left: 0, top: (rowEdges[drag.to - 1] ?? 0) - 2, width }} />
        )
      ) : null}
    </>
  )
}
