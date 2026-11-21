/**
 * The Note kit's editor: kit-editor's frame around the engine's own canvas (Word's model). The engine
 * lays out and draws the note and edits it through its `text` tool (caret, selection, typing,
 * keys), so this page holds no text editor: it adds the menus, the panels (outline, comments), the
 * find bar, the AI review, exports, autosave, links the person opens, and the clipboard (the
 * engine's selection goes to the system clipboard on copy and cut; paste arrives as typed text).
 */
import { useCallback, useEffect, useMemo, useRef, useState, type DragEvent, type KeyboardEvent, type MouseEvent as ReactMouseEvent } from 'react'
import { MessageSquareIcon as MessageSquare } from '@kition/tessera/icons/react'
import {
  Button,
  Dialog,
  EditorFrame,
  KitDocumentCanvas,
  KitEditorProvider,
  KitElement,
  catalogTabs,
  toast,
  createKitStrings,
  useEngineTools,
  useKitEditorState,
  useKitSession,
  type HostConnection,
  type KitCanvasHandle,
  type KitCanvasView,
  type KitView,
  type MenuRow,
  type RasterView,
} from '@kition/sdk/editor'
import { DESIGN_ACTION, MAX_SELECTION_CHARS, canAct, canGenerate, generateSuggestion, type AssistKind } from './ai/assist'
import { ReviewBar, SelectionBar, type Review } from './ai/ReviewBar'
import { describe, engineManifest } from './engine'
import { ExportDialog, canPrint, type ExportKind } from './ExportDialog'
import { FindBar, type FindMode } from './FindBar'
import { TableControls, type TableGeometry } from './TableControls'
import { SlashMenu, caretPlace, type Caret } from './SlashMenu'
import { TagMenu } from './TagMenu'
import { NoteScrollbar, scrollBounds, wheelPixels } from './NoteScroll'
import { CodeFlairs, type CodeBlock } from './CodeFlair'
import { EditorMenu } from './EditorMenu'
import { Properties, type PropertiesBlock } from './Properties'
import { Diagrams, type DiagramsView } from './Diagrams'
import { pastedMarkdown } from './pasteHtml'
import { canFetchImages, imageMarkdown, keepFetchedImage, keepImage, loadWantedImages } from './workspaceImages'
import { keepRemoteImages } from './remoteImages'
import { canOpenFiles, canOpenUrls, isNotFound, markdownTarget, openLink, openUrl, wikilinkRef, wikilinkView, type LinkRef } from './links'
import { headingNamed, revealView, showPlace } from './reveal'
import { FirstView } from './firstView'
import { FocusEscape } from './focusEscape'
import { FormatRibbon } from './FormatRibbon'
import { FORMAT_TAB, NOTE_MENUS, PRIMARY_ROWS, VIEW_MENUS, isEditRow } from './menus'
import { NoteComments } from './panels/CommentsPanel'
import { OutlinePanel } from './panels/OutlinePanel'
import { CATALOGS, phrasesFor } from './strings'
import { NoteSessionBridge, message, useNote } from './useNote'
import { NoteAutosave } from './autosave'
import { PeerPresence, useNotePresence } from './Presence'
import { HostCommentMarks, canCreateComment, useHostComments } from './HostComments'
import { useReadOnlySelect } from './readOnlySelect'
import { useColorScheme } from './colorScheme'

type Strings = ReturnType<typeof createKitStrings>

/** The engine's `selection.get`: Unicode character positions. */
type Selection = { from: number; to: number; text: string }

/** What the engine's `openLink` request carries (a Cmd+click on a link). */
type LinkRequest = { kind: string; destination: string; file: string; heading: string | null; blockId: string | null; external: boolean }

/** Room kept above and below the caret when it is scrolled into view, in document pixels: 5, as
 * Obsidian's editor (CodeMirror's scroll margin) keeps it the nearest way (measured). */
const CARET_MARGIN = 5

const MAC = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform)
/** The Control keys that move the caret, and those that edit the note (measured in Obsidian). */
const CONTROL_MOVES = new Set(['a', 'e', 'b', 'f', 'p', 'n', 'v'])
const CONTROL_EDITS = new Set(['d', 'h', 'k', 'o', 't'])

export function App({ host }: { host: HostConnection }) {
  const t = useMemo(() => createKitStrings(CATALOGS, host.init.locale), [host])
  const phrase = useMemo(() => phrasesFor(host.init.locale), [host])
  const manifest = useMemo(engineManifest, [])
  const bridge = useMemo(() => new NoteSessionBridge(), [])
  const session = useKitSession(host, manifest, describe, bridge.options)
  const note = useNote(session, host, bridge, t)
  const editor = useKitEditorState(session, host, t, 'note', phrase)
  const name = host.init.document.name
  const { doc, change, state } = session
  const canvas = useRef<KitCanvasHandle>(null)
  // The page opens at its natural size from the top, never fitted whole: a remembered view is one
  // kit-editor's canvas starts from (the first placement then centers it).
  const viewMemory = useRef<RasterView | null>({ zoom: 1, x: 0, y: 0 })
  const [outlineOpen, setOutlineOpen] = useState(false)
  const [commentsOpen, setCommentsOpen] = useState(false)
  const [find, setFind] = useState<FindMode | null>(null)
  const openThreads = state?.openThreads ?? 0
  const readOnly = host.readOnly === true || state?.readOnly === true
  const [selected, setSelected] = useState(false)
  /** The engine's selected text after its last answer (for copy and cut). */
  const selectedText = useRef('')
  const [findText, setFindText] = useState('')
  /** Bumped by Mod+; to ask the properties block for a new property. */
  const [addProperty, setAddProperty] = useState(0)
  /** A note with no front matter: the empty properties block Mod+; shows at its top until a key is
   * named (then the front matter is written, as Obsidian writes it). */
  const [pendingProperties, setPendingProperties] = useState<PropertiesBlock | null>(null)
  /** Opens the find bar, the selected text (on one line) to find, as Obsidian's. */
  const openFind = useCallback((mode: FindMode) => {
    const text = selectedText.current
    setFindText(text && !text.includes('\n') ? text : '')
    setFind(mode)
  }, [])
  const [review, setReview] = useState<Review | null>(null)
  const [exporting, setExporting] = useState<ExportKind | null>(null)
  /** The suggestion asked last: an answer to an earlier one (cancelled, asked again) is dropped. */
  const asked = useRef(0)
  const generates = canGenerate(host) && !readOnly

  // Links (KitLinks 1.0 section 5): other files and web addresses open in the host, from the
  // person's Cmd+click; a heading of this note shows here. A missing note is offered to create.
  const opensFiles = canOpenFiles(host)
  const opensUrls = canOpenUrls(host)
  const [missing, setMissing] = useState<{ link: LinkRef; view?: KitView; beside?: boolean } | null>(null)
  const open = useCallback(
    async (link: LinkRef, options: { view?: KitView; beside?: boolean; create?: boolean } = {}) => {
      try {
        await openLink(host, link, options)
      } catch (error) {
        if (isNotFound(error) && !options.create) setMissing({ link, ...(options.view ? { view: options.view } : {}), ...(options.beside ? { beside: true } : {}) })
        else toast('error', t('linkFailed'), message(error))
      }
    },
    [host, t],
  )
  const followLink = useCallback(
    async (request: LinkRequest) => {
      const wiki = request.kind === 'wikilink' || request.kind === 'embed'
      if (wiki) {
        const shown = wikilinkView({ heading: request.heading, blockId: request.blockId })
        if (!request.file.trim()) {
          // `[[#Heading]]`, `[[#^block]]`: a place of this note.
          if (shown) await revealView(shown, note.run)
          return
        }
        if (opensFiles) await open(wikilinkRef(request.file), shown ? { view: shown } : {})
        return
      }
      const target = markdownTarget(request.destination)
      if (target.kind === 'web' && opensUrls) await openUrl(host, target.url).catch((error: unknown) => toast('error', t('linkFailed'), message(error)))
      else if (target.kind === 'here') await showPlace(await headingNamed(target.section, note.run), note.run)
      else if (target.kind === 'file' && opensFiles) await open({ path: target.path }, target.section ? { view: { heading: [target.section] } } : {})
    },
    [host, note.run, open, opensFiles, opensUrls, t],
  )

  const tools = useEngineTools(session.tool, doc != null, change, {
    initial: 'text',
    hostTools: {},
    onDialog: (dialog, params) => {
      if (dialog === 'openLink') void followLink(params as LinkRequest)
    },
  })

  /** The caret in document pixels, after the text tool's last answer (for the input method). */
  const [caretAt, setCaretAt] = useState<{ x: number; y: number; height: number } | null>(null)
  const [slashCaret, setSlashCaret] = useState<Caret | null>(null)
  /** The note's tags, for the tag menu (a read: it does not refresh the canvas). */
  const noteTags = useCallback(async () => {
    if (!doc) return []
    const answer = JSON.parse(await doc.execute('links.list', { kinds: ['tag'] })) as { links: Array<{ name: string }> }
    return answer.links.map((link) => link.name)
  }, [doc])
  /** The caret the last follow saw (in document pixels). */
  const followed = useRef<{ x: number; y: number; height: number } | null>(null)
  /** Brings the caret into view when it left it (typing at the bottom, a search hit, a heading). */
  const followCaret = useCallback(async () => {
    const box = canvas.current
    const view = box?.view()
    if (!doc || !box || !view) return
    let caret: { x: number; y: number; height: number; remote?: boolean }
    try {
      caret = JSON.parse(await doc.invoke<string>('caretRect')) as typeof caret
    } catch {
      return
    }
    setCaretAt(caret)
    // The line before the caret, for the slash menu.
    void doc
      .execute('selection.get', {})
      .then((answer) => {
        const got = JSON.parse(answer) as { focus: number; to: number; from: number; before: string }
        setSlashCaret(got.from === got.to ? { focus: got.focus, before: got.before } : null)
      })
      .catch(() => setSlashCaret(null))
    // Only a caret that moved is brought back: an answer that leaves it where it was (a hover, a
    // repaint) never pulls the note back to it while the person scrolls elsewhere.
    const last = followed.current
    followed.current = caret
    if (last && last.x === caret.x && last.y === caret.y && last.height === caret.height) return
    const height = view.height / view.zoom
    // Another person's change moved the caret (text came in above it): a caret in view stays where
    // it is on screen, the note moving under the person's eyes no more than the change needs; a
    // caret out of view is left there (the person reads elsewhere).
    if (caret.remote) {
      const shown = last !== null && last.y + last.height > view.y && last.y < view.y + height
      if (shown && caret.y !== last.y) box.panTo(view.x, Math.max(0, view.y + caret.y - last.y))
      return
    }
    if (caret.y < view.y + CARET_MARGIN) box.panTo(view.x, Math.max(0, caret.y - CARET_MARGIN))
    else if (caret.y + caret.height > view.y + height - CARET_MARGIN) box.panTo(view.x, caret.y + caret.height + CARET_MARGIN - height)
  }, [doc])

  // After every answer of the text tool (its overlays move with the caret), the caret stays in view
  // and the selection bar follows whether text is selected.
  const overlays = tools.decorate
  useEffect(() => {
    if (!doc || !overlays) return
    void followCaret()
    void doc
      .invoke<string>('selectedText')
      .then((text) => {
        selectedText.current = text
        setSelected(Boolean(text))
      })
      .catch(() => setSelected(false))
  }, [doc, followCaret, overlays])

  // Presence (Kit UI protocol section 7e): this person's caret and selection shared after every
  // change and every answer of the text tool, and the other people's drawn over the canvas.
  const peers = useNotePresence(host, doc, change.revision, overlays)
  // A host that keeps comment threads itself (Kit UI protocol section 7k): its threads marked on
  // the text, and a new one on the selection is the host's. Otherwise the note's own (the panel).
  const hostComments = useHostComments(host, doc, change.revision, overlays, t)
  const commentable = canCreateComment(host)
  const createComment = hostComments.create
  const commentOnSelection = useCallback(() => void createComment(), [createComment])

  const [shownView, setShownView] = useState<KitCanvasView | null>(null)
  // The document heights whose tables the controls need: the screen in view and one around it,
  // moved a screen at a time so a scroll asks again only when it leaves them.
  const screen = shownView ? shownView.height / shownView.zoom : 0
  const band = shownView && screen > 0 ? Math.floor(shownView.y / screen) : 0
  const tableRange = useMemo<[number, number] | null>(() => (screen > 0 ? [(band - 1) * screen, (band + 3) * screen] : null), [band, screen])

  // The tables' grids, for the table controls: after every change and every answer of the text
  // tool (the caret's row shows its syntax, which can widen a column). Read straight from the
  // engine, so reading is no change.
  const [tables, setTables] = useState<TableGeometry[]>([])
  // The note's text for assistive technology (see below); its size picks how soon it is read.
  const [mirror, setMirror] = useState<{ text: string; pictures: string[] }>({ text: '', pictures: [] })
  const mirrorSize = useRef(0)
  mirrorSize.current = mirror.text.length
  useEffect(() => {
    // Asked once the band in view is known: the whole of a large note's is large.
    if (!doc || !tableRange) return undefined
    let current = true
    // A note that is not large has its accessible text read with the grids and shown with them,
    // so a table's row count and the text it is drawn from never disagree (a host's test reads
    // one, then the other).
    const small = mirrorSize.current <= 200_000
    void Promise.all([
      // The tables in view and a screen around it (a large note has thousands).
      doc.execute('table.geometry', { top: tableRange[0], bottom: tableRange[1] }),
      small ? doc.execute('text.read', {}) : Promise.resolve(null),
    ])
      .then(([answer, read]) => {
        if (!current) return
        setTables((JSON.parse(answer) as { tables: TableGeometry[] }).tables)
        if (read !== null) {
          const text = (JSON.parse(read) as { text: string }).text
          setMirror((shown) => (shown.text === text ? shown : { ...shown, text }))
        }
      })
      .catch(() => undefined)
    return () => {
      current = false
    }
  }, [doc, change.revision, overlays, tableRange])
  // The code blocks in view, for their language labels and copy buttons.
  const [codeBlocks, setCodeBlocks] = useState<CodeBlock[]>([])
  useEffect(() => {
    // Asked once the band in view is known: the whole of a large note's is large.
    if (!doc || !tableRange) return undefined
    let current = true
    void doc
      .execute('code.blocks', { top: tableRange[0], bottom: tableRange[1] })
      .then((answer) => {
        if (current) setCodeBlocks((JSON.parse(answer) as { blocks: CodeBlock[] }).blocks)
      })
      .catch(() => undefined)
    return () => {
      current = false
    }
  }, [doc, change.revision, tableRange])
  // The front matter as a properties block (null while it is edited as its source).
  const [properties, setProperties] = useState<PropertiesBlock | null>(null)
  /** Mod+; (Obsidian's Add file property): a new property's key asked in the properties block. */
  const askProperty = useCallback(async () => {
    if (!properties) {
      const at = await note.run<{ x: number; y: number }>('selection.where', { at: 0 })
      const column = await note.run<{ column?: number }>('diagrams.view', { top: 0, bottom: 0 }).then((answer) => answer.column ?? 700)
      setPendingProperties({ top: at.y - 8, bottom: at.y + 92, left: at.x, width: column, end: 0, rows: [] })
    }
    setAddProperty((count) => count + 1)
  }, [note, properties])
  useEffect(() => {
    if (!doc) return undefined
    let current = true
    void doc
      .execute('properties.view', {})
      .then((answer) => {
        if (!current) return
        const block = JSON.parse(answer) as PropertiesBlock | null
        setProperties(block)
        if (block) setPendingProperties(null)
      })
      .catch(() => undefined)
    return () => {
      current = false
    }
  }, [doc, change.revision])
  // The host's color scheme: the engine paints the note in its palette (nothing moves), at open
  // and whenever the host changes it.
  // (The session is read through a ref: it is a new object on each render, and running the
  // command on each render would never settle.)
  // An engine opens in the light palette, so the canvas shows a document only once the engine has
  // its scheme: its first frame is already in the host's (no light paper for a frame in a dark
  // host). That is one command in the worker after the open; a later change repaints in place.
  const scheme = useColorScheme()
  const schemeSession = useRef(session)
  schemeSession.current = session
  const [schemedDoc, setSchemedDoc] = useState<typeof doc>(null)
  useEffect(() => {
    if (!doc) return undefined
    let current = true
    void schemeSession.current
      .run('view.scheme', { scheme })
      .catch(() => undefined)
      .then(() => {
        if (current) setSchemedDoc(doc)
      })
    return () => {
      current = false
    }
  }, [doc, scheme])
  const schemeSet = doc !== null && schemedDoc === doc
  // Mermaid diagrams: those to draw, and where the drawn ones are (in view and a screen around).
  const [diagrams, setDiagrams] = useState<DiagramsView | null>(null)
  useEffect(() => {
    // Asked once the band in view is known: the whole of a large note's is large.
    if (!doc || !tableRange) return undefined
    let current = true
    void doc
      .execute('diagrams.view', { top: tableRange[0], bottom: tableRange[1] })
      .then((answer) => {
        if (current) setDiagrams(JSON.parse(answer) as DiagramsView)
      })
      .catch(() => undefined)
    return () => {
      current = false
    }
  }, [doc, change.revision, tableRange])
  const sizeDiagram = useCallback((source: string, width: number, height: number) => session.run('diagram.set', { source, width, height }), [session])
  const readText = useCallback(
    async (from: number, to: number) => (doc ? (JSON.parse(await doc.execute('text.read', { from, to })) as { text: string }).text : ''),
    [doc],
  )
  // Pictures: the engine asks for the image-only lines' pictures it does not have; the host reads them.
  const triedImages = useRef(new Set<string>())
  useEffect(() => {
    if (!doc) return undefined
    let current = true
    void doc
      .execute('image.wanted', {})
      .then((answer) => {
        const wanted = (JSON.parse(answer) as { images: string[] }).images.filter((name) => !triedImages.current.has(name))
        if (current && wanted.length) void loadWantedImages(host, wanted, (command, params) => session.run(command, params), triedImages.current)
      })
      .catch(() => undefined)
    return () => {
      current = false
    }
  }, [doc, change.revision, host, session])

  /** Keeps image files as workspace files and links them at the caret, one undo step each. */
  const insertImages = useCallback(
    async (files: File[]) => {
      for (const [index, file] of files.entries()) {
        try {
          const kept = await keepImage(host, file, index + 1)
          await note.run('selection.replace', { text: imageMarkdown(host, kept, file) })
        } catch (error) {
          toast('error', t('imageFailed'), message(error))
        }
      }
    },
    [host, note, t],
  )
  /**
   * Pasted Markdown goes in with its pictures given by web address kept as workspace files first
   * (when the host downloads them), so the note never depends on an address that expires.
   */
  const pasteMarkdown = useCallback(
    async (markdown: string) => {
      let text = markdown
      if (canFetchImages(host)) {
        const kept = await keepRemoteImages(markdown, (url, index) => keepFetchedImage(host, url, index))
        text = kept.text
        if (kept.failed.length) toast('error', t('imagesNotKept', { count: kept.failed.length }), message(kept.failed[0]?.error))
      }
      await note.run('selection.replace', { text })
    },
    [host, note, t],
  )
  // A pasted image (from the clipboard's files) is kept and linked; text paste stays the text field's.
  useEffect(() => {
    if (!doc || readOnly) return undefined
    const paste = (event: ClipboardEvent) => {
      // HTML from a web page goes in as Markdown, as Obsidian converts it.
      const markdown = pastedMarkdown(event.clipboardData)
      if (markdown !== null) {
        event.preventDefault()
        event.stopPropagation()
        void pasteMarkdown(markdown).catch((error: unknown) => toast('error', t('commandFailed'), message(error)))
        return
      }
      const files = [...(event.clipboardData?.files ?? [])].filter((file) => file.type.startsWith('image/'))
      if (!files.length) {
        // Plain text pasted in the note goes in as a paste, an undo step of its own that the
        // typing after it joins (as in Obsidian), not as typed text.
        const plain = event.clipboardData?.getData('text/plain') ?? ''
        const target = event.target
        if (!plain || !(target instanceof HTMLTextAreaElement) || !target.classList.contains('kui-text-sink')) return
        event.preventDefault()
        event.stopPropagation()
        void note.run('selection.replace', { text: plain, paste: true }).catch((error: unknown) => toast('error', t('commandFailed'), message(error)))
        return
      }
      event.preventDefault()
      event.stopPropagation()
      void insertImages(files)
    }
    document.addEventListener('paste', paste, true)
    return () => document.removeEventListener('paste', paste, true)
  }, [doc, insertImages, note, pasteMarkdown, readOnly, t])
  /** A dropped image is kept and linked where it was dropped. */
  const onDrop = useCallback(
    (event: DragEvent<HTMLDivElement>) => {
      const files = [...event.dataTransfer.files].filter((file) => file.type.startsWith('image/'))
      const view = canvas.current?.view()
      if (!files.length || !view || readOnly) return
      event.preventDefault()
      const surface = (event.currentTarget.querySelector('[data-testid="kit-canvas-surface"]') ?? event.currentTarget).getBoundingClientRect()
      const x = view.x + (event.clientX - surface.left) / view.zoom
      const y = view.y + (event.clientY - surface.top) / view.zoom
      void note.run('selection.set', { x, y }).then(() => insertImages(files))
    },
    [insertImages, note, readOnly],
  )

  // The note's text for assistive technology (the canvas has none): a visually hidden document,
  // and the pictures drawn, named by their alt text's source. Updated a moment after each change
  // (a large note's text is read once the typing pauses, and the same text renders nothing).
  // A large note's text is read after a longer pause (reading a megabyte is not free).
  const mirrorDelay = mirror.text.length > 200_000 ? 1000 : 200
  useEffect(() => {
    if (!doc) return undefined
    let current = true
    const timer = window.setTimeout(() => {
      void Promise.all([doc.execute('text.read', {}), doc.execute('image.wanted', {})])
        .then(([text, images]) => {
          if (!current) return
          const next = { text: (JSON.parse(text) as { text: string }).text, pictures: (JSON.parse(images) as { drawn: string[] }).drawn }
          setMirror((shown) => (shown.text === next.text && shown.pictures.join('\n') === next.pictures.join('\n') ? shown : next))
        })
        .catch(() => undefined)
    }, mirrorDelay)
    return () => {
      current = false
      window.clearTimeout(timer)
    }
  }, [doc, change.revision, mirrorDelay])

  const [tableMenu, setTableMenu] = useState<{ table: number; column: number; row: number; x: number; y: number } | null>(null)
  /** A right click on a table cell opens its column's menu. */
  const onContextMenu = useCallback(
    (event: ReactMouseEvent<HTMLDivElement>) => {
      const view = canvas.current?.view()
      const surface = (event.currentTarget.querySelector('[data-testid="kit-canvas-surface"]') ?? event.currentTarget).getBoundingClientRect()
      if (!view) return
      const px = event.clientX - surface.left
      const py = event.clientY - surface.top
      const x = view.x + px / view.zoom
      const y = view.y + py / view.zoom
      for (const table of tables) {
        const top = table.rows[0]?.top ?? 0
        const bottom = table.rows.at(-1)?.bottom ?? top
        if (y < top || y > bottom) continue
        const column = table.columns.findIndex((edge, index) => index + 1 < table.columns.length && x >= edge && x < (table.columns[index + 1] ?? edge))
        if (column < 0) continue
        // The row under the point: 0 is the header (the column's menu only).
        const row = Math.max(0, table.rows.findIndex((candidate) => y >= candidate.top && y < candidate.bottom))
        event.preventDefault()
        setTableMenu({ table: table.table, column, row, x: px, y: py })
        return
      }
      // Elsewhere, the editor's menu; a click away from a selection puts the caret there first.
      event.preventDefault()
      const at = { x: event.clientX, y: event.clientY }
      void (async () => {
        // A callout drawn as one (the caret outside it) opens no menu, as in Obsidian.
        const block = await note.run<{ callout?: boolean; active?: boolean }>('view.blockAt', { x, y }).catch(() => ({ callout: false, active: false }))
        if (block.callout && !block.active) return
        const selected = doc ? await doc.invoke<string>('selectedText').catch(() => '') : ''
        if (!selected) await note.run('selection.set', { x, y }).catch(() => undefined)
        setEditorMenu(at)
      })()
    },
    [doc, note, tables],
  )
  const [editorMenu, setEditorMenu] = useState<{ x: number; y: number } | null>(null)
  const menuSelectedText = useCallback(async () => (doc ? doc.invoke<string>('selectedText').catch(() => '') : ''), [doc])

  // On a Mac, Ctrl+Backspace deletes a character back and Ctrl+Delete nothing (as in Obsidian);
  // the frame would take Control for Command, so they are caught before it.
  useEffect(() => {
    if (!MAC || !doc) return undefined
    const keydown = (event: globalThis.KeyboardEvent) => {
      if (!event.ctrlKey || event.metaKey || event.altKey || (event.key !== 'Backspace' && event.key !== 'Delete')) return
      const target = event.target
      if (!(target instanceof HTMLTextAreaElement) || !target.classList.contains('kui-text-sink')) return
      event.preventDefault()
      event.stopPropagation()
      if (event.key === 'Backspace' && !readOnly) {
        void note
          .run('selection.controlKey', { key: 'h' })
          .then(() => tools.refresh())
          .catch(() => undefined)
      }
    }
    window.addEventListener('keydown', keydown, true)
    return () => window.removeEventListener('keydown', keydown, true)
  }, [doc, note, readOnly, tools])

  /** After a command moved the selection without changing the note: new overlays, and the caret shown. */
  const shown = useCallback(() => {
    tools.refresh()
    window.setTimeout(() => void followCaret(), 0)
  }, [followCaret, tools])

  // A person who may not edit selects with the mouse all the same (to copy, or to comment on).
  // The engine's tool draws no selection where it takes no gestures, so the selection is drawn here.
  const [readOnlyRects, setReadOnlyRects] = useState<Array<{ x: number; y: number; width: number; height: number }>>([])
  const readOnlySelected = useCallback(() => {
    shown()
    if (!doc) return
    void doc
      .invoke<string>('selectedText')
      .then((text) => {
        selectedText.current = text
        setSelected(Boolean(text))
      })
      .catch(() => setSelected(false))
    void (async () => {
      const { from, to } = JSON.parse(await doc.execute('selection.get', {})) as Selection
      const at = (offset: number) => ({ object: 'note', field: 'body', offset })
      const placed = JSON.parse(await doc.invoke<string>('presenceLocate', JSON.stringify([{ caret: at(to), anchor: at(from) }]))) as Array<{ rects?: Array<{ x: number; y: number; width: number; height: number }> }>
      setReadOnlyRects(placed[0]?.rects ?? [])
    })().catch(() => setReadOnlyRects([]))
  }, [doc, shown])
  const readOnlySelect = useReadOnlySelect(host.readOnly === true && doc !== null, () => canvas.current?.view() ?? null, note.run, readOnlySelected)

  // The first view: the column at its natural size, centered, from the top.
  const placed = useRef(false)
  const firstView = useRef(new FirstView())
  // The document is as wide as the canvas, and the engine keeps its readable column centered in it.
  const fitted = useRef(0)
  const fittedHeight = useRef(0)
  /** The document's width as the engine took it (the column is centered in it), for the comment markers. */
  const [docWidth, setDocWidth] = useState<number | null>(null)
  const fit = useCallback(
    (view: KitCanvasView) => {
      if (!doc || !view.width) return
      const wanted = Math.round(view.width / view.zoom)
      // The visible height sets the room below the note (half of it, as Obsidian's).
      const tall = Math.round(view.height / view.zoom)
      if (Math.abs(wanted - fitted.current) < 2 && Math.abs(tall - fittedHeight.current) < 2) return
      fitted.current = wanted
      fittedHeight.current = tall
      void session
        .run('view.width', { width: wanted, height: tall })
        .then((answer) => {
          // The page stays centered in the canvas.
          const width = (JSON.parse(String(answer)) as { width?: number }).width ?? wanted
          setDocWidth(width)
          const box = canvas.current
          const now = box?.view()
          if (box && now) box.panTo(width / 2 - now.width / now.zoom / 2, now.y)
        })
        .catch(() => undefined)
    },
    [doc, session],
  )
  /**
   * Places the first view of a document. Marked placed only once it is: the canvas's handle or the
   * document can come a frame after the first view, so it tries again then.
   */
  const latestView = useRef<KitCanvasView | null>(null)
  const place = useCallback(() => {
    const view = latestView.current
    if (placed.current || !view?.width) return
    const box = canvas.current
    if (!doc || !box) {
      requestAnimationFrame(() => place())
      return
    }
    placed.current = true
    box.zoomTo(1)
    box.panTo(doc.info.width / 2 - view.width / 2, 0)
    firstView.current.placed()
  }, [doc])
  /** Scrolls to document height `y`, kept in the note's bounds (see NoteScroll). */
  const scrollTo = useCallback(
    (y: number) => {
      const box = canvas.current
      const view = box?.view()
      if (!doc || !box || !view) return
      const to = scrollBounds(view, doc, y)
      box.panTo(to.x, to.y)
    },
    [doc],
  )
  const onView = useCallback(
    (view: KitCanvasView) => {
      latestView.current = view
      setShownView(view)
      fit(view)
      place()
      // A view out of the note's bounds (a zoom about the pointer, a pan the canvas made itself) comes back in.
      if (doc && placed.current) {
        const to = scrollBounds(view, doc)
        if (Math.abs(to.x - view.x) > 0.5 || Math.abs(to.y - view.y) > 0.5) canvas.current?.panTo(to.x, to.y)
      }
    },
    [doc, fit, place],
  )
  // The wheel scrolls the note up and down, never sideways, and a pinch does not zoom it (as in
  // Obsidian). Taken before kit-editor's canvas, which would pan and zoom it as a drawing.
  const scrollBox = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const element = scrollBox.current
    if (!element) return undefined
    const wheel = (event: WheelEvent) => {
      if (!(event.target instanceof HTMLCanvasElement)) return
      event.preventDefault()
      event.stopPropagation()
      if (event.ctrlKey || event.metaKey) return
      const view = canvas.current?.view()
      if (!view) return
      scrollTo(view.y + wheelPixels(event, view.height / view.zoom) / view.zoom)
    }
    element.addEventListener('wheel', wheel, { capture: true, passive: false })
    return () => element.removeEventListener('wheel', wheel, { capture: true })
  }, [scrollTo])
  // Another document in this editor opens at the top again.
  useEffect(() => {
    followed.current = null
    placed.current = false
    fitted.current = 0
    viewMemory.current = { zoom: 1, x: 0, y: 0 }
  }, [doc])

  // The clipboard: copy and cut take the engine's selection (kept after every answer of the text
  // tool, so the event can carry it at once); paste reaches the engine as typed text through
  // kit-editor's text field, so it needs nothing here.
  useEffect(() => {
    if (!doc) return undefined
    const copy = (event: ClipboardEvent, cut: boolean) => {
      const target = event.target
      if (target instanceof HTMLInputElement || (target instanceof HTMLTextAreaElement && !target.classList.contains('kui-text-sink'))) return
      const text = selectedText.current
      if (!text || !event.clipboardData) return
      event.preventDefault()
      event.clipboardData.setData('text/plain', text)
      if (cut && !readOnly) void session.run('edit.cut', {}).catch((error: unknown) => toast('error', t('commandFailed'), message(error)))
    }
    const onCopy = (event: ClipboardEvent) => copy(event, false)
    const onCut = (event: ClipboardEvent) => copy(event, true)
    document.addEventListener('copy', onCopy)
    document.addEventListener('cut', onCut)
    return () => {
      document.removeEventListener('copy', onCopy)
      document.removeEventListener('cut', onCut)
    }
  }, [doc, readOnly, session, t])

  /** The engine's selection, or `null` without one. */
  const selection = useCallback(async () => {
    try {
      const answer = await note.run<Selection>('selection.get', {})
      return answer.to > answer.from ? answer : null
    } catch {
      return null
    }
  }, [note])

  /** Asks the host's model to rewrite the selection; the note changes only on Accept. */
  const assist = useCallback(
    async (kind: AssistKind, range?: { from: number; to: number }) => {
      const current = range ? null : await selection()
      const from = range?.from ?? current?.from
      const to = range?.to ?? current?.to
      if (from === undefined || to === undefined || from === to) {
        toast('info', t('aiSelectFirst'))
        return
      }
      if (to - from > MAX_SELECTION_CHARS) {
        toast('info', t('aiTooLong'))
        return
      }
      const read = async (a: number, b: number) => (await note.run<{ text: string }>('text.read', { from: a, to: b }).catch(() => ({ text: '' }))).text ?? ''
      const original = current?.text ?? (await read(from, to))
      const ticket = ++asked.current
      setReview({ kind, from, to, original, status: 'running' })
      const [before, after] = await Promise.all([read(Math.max(0, from - 1200), from), read(to, to + 1200)])
      const outcome = await generateSuggestion(host, { kind, text: original, before, after, title: name })
      if (ticket !== asked.current) return
      if ('text' in outcome) setReview({ kind, from, to, original, status: 'ready', suggestion: outcome.text, model: outcome.model })
      else setReview({ kind, from, to, original, status: 'failed', reason: outcome.reason, message: outcome.message })
    },
    [host, name, note, selection, t],
  )

  /** Accept: one engine command over the reviewed text; refused when the text there changed. */
  const accept = useCallback(async () => {
    const shownReview = review
    if (!shownReview || shownReview.suggestion === undefined) return
    try {
      await note.run('text.replace', { from: shownReview.from, to: shownReview.to, text: shownReview.suggestion, expected: shownReview.original })
      asked.current += 1
      setReview(null)
    } catch (error) {
      toast('error', String(error).includes('stale') ? t('aiStale') : t('commandFailed'), message(error))
    }
  }, [note, review, t])
  const reject = useCallback(() => {
    asked.current += 1
    setReview(null)
  }, [])

  /** Ask the agent (capability `agent`), about the selection when there is one. */
  const askAgent = useCallback(async () => {
    const current = await selection()
    void host.openAgent(current ? `${t('aiAgentPrompt', { name })}\n\n${current.text}` : undefined)
  }, [host, name, selection, t])

  /** The host action `design.fromRecord`: a design from the note's title and properties, from this click only. */
  const makeDesign = useCallback(async () => {
    let fields: Record<string, unknown> = {}
    try {
      const answer = await note.run<{ properties: Array<{ key: string; value: unknown }> }>('property.list', {})
      fields = Object.fromEntries(answer.properties.map((property) => [property.key, property.value]))
    } catch {
      fields = {}
    }
    try {
      const answer = (await host.peer.request('host/action', { id: DESIGN_ACTION, params: { title: state?.heading || name.replace(/\.(md|markdown)$/i, ''), fields } })) as { done: boolean; message?: string }
      if (answer.done) toast('success', answer.message || t('aiDesignDone'))
      else toast('error', t('aiActionFailed'), answer.message)
    } catch (error) {
      toast('error', t('aiActionFailed'), message(error))
    }
  }, [host, name, note, state?.heading, t])

  // Where the host shows the note (Kit UI protocol section 7i, OP3): the view it opened the note
  // at, once the note is open, then each `document/reveal` (a search hit, a diagnostic, a link).
  const openingView = useRef(host.view)
  useEffect(() => {
    if (!doc) return undefined
    const first = openingView.current
    openingView.current = null
    if (first) firstView.current.whenPlaced(() => void revealView(first, note.run).then((done) => done && shown()))
    return host.onReveal((view) => void revealView(view, note.run).then((done) => done && shown()))
  }, [doc, host, note.run, shown])

  const onKitCommand = useCallback(
    (command: string) => {
      const kitName = command.slice('kit:'.length)
      if (kitName === 'outline') setOutlineOpen((value) => !value)
      else if (kitName === 'comments') setCommentsOpen((value) => !value)
      else if (kitName === 'find') openFind('find')
      else if (kitName === 'goto-line') setFind('line')
      else if (kitName === 'ai-improve' || kitName === 'ai-shorten' || kitName === 'ai-expand') void assist(kitName.slice('ai-'.length) as AssistKind)
      else if (kitName === 'export-html') setExporting('html')
      else if (kitName === 'export-pdf') setExporting('pdf')
      else if (kitName === 'print') setExporting('print')
      else if (kitName === 'ai-agent') void askAgent()
      else if (kitName === 'ai-design') void makeDesign()
    },
    [askAgent, assist, makeDesign, openFind],
  )
  const agent = host.init.capabilities.includes('agent')
  const designs = canAct(host, DESIGN_ACTION) && !readOnly
  const prints = canPrint(host)
  const hidden = useCallback(
    (row: MenuRow) => {
      if (readOnly && isEditRow(row.command)) return true
      if (row.command === 'kit:ai-improve' || row.command === 'kit:ai-shorten' || row.command === 'kit:ai-expand') return !generates
      if (row.command === 'kit:ai-agent') return !agent
      if (row.command === 'kit:ai-design') return !designs
      if (row.command === 'kit:print') return !prints
      // The host lists and opens its own threads.
      if (row.command === 'kit:comments') return hostComments.on
      return false
    },
    [agent, designs, generates, hostComments.on, prints, readOnly],
  )
  const checked = useCallback((command: string) => (command === 'kit:outline' ? outlineOpen : command === 'kit:comments' ? commentsOpen : undefined), [commentsOpen, outlineOpen])
  // Format, the tab the note opens on, is a Markdown editor's toolbar in Single line and Classic
  // (FormatRibbon); in Menus it stays the Format menu. The other tabs are their menus' rows.
  const tabs = useMemo(
    () => catalogTabs(NOTE_MENUS, { phrase, hidden, onKitCommand, checked, view: VIEW_MENUS, primary: PRIMARY_ROWS }).map((tab) => (tab.id === FORMAT_TAB ? { ...tab, ribbon: <FormatRibbon /> } : tab)),
    [checked, hidden, onKitCommand, phrase],
  )
  const hasSelection = useCallback(() => selectedText.current !== '', [])

  const keyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    // macOS's Control editing keys (Ctrl+A, E, B, F, P, N, V, D, H, K, O, T) edit as in Obsidian;
    // the frame's text field hands Control keys to the shortcuts, so they come here.
    if (MAC && event.ctrlKey && !event.metaKey && !event.altKey && doc) {
      const letter = event.code.startsWith('Key') ? event.code.slice(3).toLowerCase() : event.key.toLowerCase()
      if (CONTROL_MOVES.has(letter) || (!readOnly && CONTROL_EDITS.has(letter))) {
        void note
          .run('selection.controlKey', { key: letter })
          .then(shown)
          .catch((error: unknown) => toast('error', t('commandFailed'), message(error)))
        return true
      }
    }
    // Page Up and Page Down move the caret a page (to the note's start or end past it), as in
    // Obsidian; the frame does not hand them to the engine.
    if ((event.key === 'PageUp' || event.key === 'PageDown') && !event.metaKey && !event.ctrlKey && !event.altKey && doc) {
      void note
        .run('selection.controlKey', { key: event.key === 'PageUp' ? 'pageUp' : 'pageDown', shift: event.shiftKey })
        .then(shown)
        .catch((error: unknown) => toast('error', t('commandFailed'), message(error)))
      return true
    }
    // Cmd+U and Cmd+Shift+U undo and redo the selection (CodeMirror's, kept by Obsidian); on a
    // Mac, Ctrl+Alt+H deletes a group back.
    const undoSelection = (event.metaKey || (!MAC && event.ctrlKey)) && !event.altKey && event.code === 'KeyU'
    const groupBack = MAC && event.ctrlKey && event.altKey && !event.metaKey && event.code === 'KeyH'
    if ((undoSelection || groupBack) && doc && !(groupBack && readOnly)) {
      const key = groupBack ? 'groupH' : event.shiftKey ? 'redoSelection' : 'undoSelection'
      void note
        .run('selection.controlKey', { key })
        .then(shown)
        .catch((error: unknown) => toast('error', t('commandFailed'), message(error)))
      return true
    }
    // Mod+; adds a property (Obsidian's Add file property), when the note has a properties block.
    if ((event.metaKey || (!MAC && event.ctrlKey)) && !event.altKey && !event.shiftKey && event.key === ';' && doc && !readOnly) {
      void askProperty().catch(() => undefined)
      return true
    }
    // Mod+Alt+M comments on the selection (the key Google Docs and Word use), in a host that keeps threads.
    if ((event.metaKey || (!MAC && event.ctrlKey)) && event.altKey && !event.shiftKey && event.code === 'KeyM' && doc && commentable) {
      commentOnSelection()
      return true
    }
    const command = event.metaKey || event.ctrlKey
    // Actual size, not the whole note fitted in the window (kit-editor's canvas default).
    if (command && !event.altKey && event.key === '0') {
      canvas.current?.zoomTo(1)
      return true
    }
    // Mod+F finds, Mod+Alt+F finds and replaces (Obsidian's keys), the selected text to find.
    if (command && (event.key === 'f' || event.key === 'F' || event.code === 'KeyF')) {
      openFind(event.altKey ? 'replace' : 'find')
      return true
    }
    if (command && event.altKey && (event.key === 'l' || event.key === 'L' || event.code === 'KeyL')) {
      setFind('line')
      return true
    }
    return false
  }

  const panels =
    doc && (outlineOpen || (commentsOpen && !hostComments.on)) ? (
      <>
        {outlineOpen ? (
          <KitElement id="panel:outline">
            <OutlinePanel session={session} run={note.run} onShown={shown} title={t('outline')} empty={t('outlineEmpty')} />
          </KitElement>
        ) : null}
        {commentsOpen && !hostComments.on ? (
          <KitElement id="panel:comments">
            <NoteComments host={host} session={session} run={note.run} t={t} />
          </KitElement>
        ) : null}
      </>
    ) : null

  return (
    <KitEditorProvider editor={editor} tools={tools}>
      <NoteAutosave waiting={note.diskChanged} t={t} />
      <EditorFrame
        tabs={tabs}
        defaultTab={FORMAT_TAB}
        canvas={canvas}
        // The note asks about a file changed on disk itself, offering a merge (note-disk-changed).
        fileChangedBar={false}
        onKeyDown={keyDown}
        title={
          <span className="note-kit-title" data-testid="note-title">
            {name}
          </span>
        }
        status={
          state ? (
            <>
              <span data-testid="note-words">{t('words', { count: state.words })}</span>
              <span>{t('summary', { characters: state.characters, lines: state.lines })}</span>
            </>
          ) : null
        }
        actions={
          hostComments.on ? null : (
          <KitElement id="control:comments">
            <Button variant="ghost" aria-pressed={commentsOpen} title={t('comments')} data-testid="note-comments-toggle" onClick={() => setCommentsOpen((value) => !value)}>
              <MessageSquare size={16} />
              {openThreads ? <span data-testid="note-comments-count">{openThreads}</span> : null}
            </Button>
          </KitElement>
          )
        }
        panels={panels}
      >
        <FocusEscape selected={hasSelection} />
        <div className="note-kit-body" data-testid="note-body">
          {state?.readOnly ? (
            <div className="kui-message kui-muted" data-testid="note-read-only">
              {t('readOnly')}
            </div>
          ) : null}
          {note.diskChanged ? (
            <div className="note-disk-changed" role="alert" data-testid="note-disk-changed">
              <span>{t('diskChanged')}</span>
              <span className="note-ai-review-spacer" />
              <Button variant="ghost" onClick={note.keepMine} data-testid="note-disk-keep">
                {t('diskKeep')}
              </Button>
              <Button variant="primary" onClick={() => void note.mergeFromDisk()} data-testid="note-disk-merge">
                {t('diskMerge')}
              </Button>
            </div>
          ) : null}
          {find ? (
            <FindBar
              mode={find}
              initial={findText}
              run={note.run}
              t={t}
              onShown={shown}
              onClose={() => {
                setFind(null)
                // The keyboard goes back to the note.
                window.requestAnimationFrame(() => (document.querySelector('[data-testid="kit-text-sink"]') as HTMLElement | null)?.focus({ preventScroll: true }))
              }}
            />
          ) : null}
          <div className="note-a11y" role="document" aria-label={name} aria-readonly={readOnly} data-testid="note-text" data-pictures={mirror.pictures.join('\n')}>
            {mirror.text}
          </div>
          {review ? <ReviewBar review={review} t={t} onAccept={() => void accept()} onReject={reject} onRetry={() => void assist(review.kind, { from: review.from, to: review.to })} /> : null}
          <div
            onContextMenuCapture={onContextMenu}
            onDragOver={(event) => {
              if ([...event.dataTransfer.items].some((item) => item.kind === 'file')) event.preventDefault()
            }}
            onDrop={onDrop}
            // The pointer gone, no line shows its fold chevron (as Obsidian hides it).
            onPointerLeave={() => void session.run('view.hoverOut', {}, null, { refresh: 'repaint' }).catch(() => undefined)}
            ref={scrollBox}
            {...readOnlySelect}
            className="note-kit-canvas"
            data-testid="note-editor"
            data-read-only={readOnly ? 'true' : undefined}
            // The document area in view (document pixels), for tests and for the host's look at the editor.
            data-view={shownView ? `${Math.round(shownView.x)},${Math.round(shownView.y)},${Math.round(shownView.width / shownView.zoom)},${Math.round(shownView.height / shownView.zoom)},${shownView.zoom.toFixed(3)}` : undefined}
          >
            {schemeSet ? (
              <KitDocumentCanvas canvas={canvas} viewMemory={viewMemory} framed={false} onView={onView} testId="note-canvas">
                <TableControls tables={tables} view={shownView} run={note.run} readOnly={readOnly} t={t} menuAt={tableMenu} />
                <Composition caret={caretAt} view={shownView} />
                <HostCommentMarks marks={hostComments.marks} view={shownView} right={docWidth !== null && diagrams ? (docWidth + diagrams.column) / 2 : null} onActivate={hostComments.activate} t={t} />
                <PeerPresence marks={peers} view={shownView} />
                {host.readOnly === true && shownView && readOnlyRects.length ? (
                  <div className="note-readonly-selection" data-testid="note-readonly-selection">
                    {readOnlyRects.map((rect, index) => (
                      <div key={index} style={{ left: (rect.x - shownView.x) * shownView.zoom, top: (rect.y - shownView.y) * shownView.zoom, width: rect.width * shownView.zoom, height: rect.height * shownView.zoom }} />
                    ))}
                  </div>
              ) : null}
              <Caret caret={caretAt} view={shownView} readOnly={readOnly} />
              <CodeFlairs blocks={codeBlocks} view={shownView} read={readText} t={t} />
              <Properties block={properties ?? pendingProperties} view={shownView} run={note.run} readOnly={readOnly} t={t} addRequest={addProperty} onCancel={() => setPendingProperties(null)} />
              <Diagrams view={shownView} diagrams={diagrams} sizes={sizeDiagram} run={note.run} readOnly={readOnly} scheme={scheme} t={t} />
              <SlashMenu caret={slashCaret} place={caretPlace(caretAt, shownView)} run={note.run} t={t} readOnly={readOnly} />
              <TagMenu caret={slashCaret} place={caretPlace(caretAt, shownView)} run={note.run} tags={noteTags} t={t} readOnly={readOnly} />
            </KitDocumentCanvas>
            ) : null}
            <NoteScrollbar view={shownView} docHeight={doc?.height ?? 0} onScroll={scrollTo} />
          </div>
          <EditorMenu at={editorMenu} onClose={() => setEditorMenu(null)} run={note.run} selectedText={menuSelectedText} readOnly={readOnly} {...(commentable ? { onComment: commentOnSelection } : {})} phrase={phrase} t={t} />
          {(generates || commentable) && selected && !review ? <SelectionBar t={t} {...(generates ? { onAssist: (kind: AssistKind) => void assist(kind) } : {})} {...(commentable ? { onComment: commentOnSelection } : {})} /> : null}
        </div>
        {missing ? (
          <Dialog
            title={t('linkMissingTitle')}
            testId="note-link-missing"
            onClose={() => setMissing(null)}
            actions={
              <>
                <Button variant="ghost" data-testid="note-link-cancel" onClick={() => setMissing(null)}>
                  {t('cancel')}
                </Button>
                <Button
                  variant="primary"
                  data-testid="note-link-create"
                  onClick={() => {
                    const wanted = missing
                    setMissing(null)
                    void open(wanted.link, { ...wanted, create: true })
                  }}
                >
                  {t('linkCreate')}
                </Button>
              </>
            }
          >
            <p data-testid="note-link-missing-message">{t('linkMissing', { name: missing.link.path ?? missing.link.name ?? '' })}</p>
          </Dialog>
        ) : null}
        {exporting ? <ExportDialog kind={exporting} host={host} session={session} t={t} onClose={() => setExporting(null)} /> : null}
      </EditorFrame>
    </KitEditorProvider>
  )
}

/**
 * The input method at the caret: kit-editor's text field (where typing goes) moves to the caret, so the
 * system's candidate window opens there, and the text being composed is drawn at the caret,
 * underlined, until the input method commits it (the engine gets the committed text).
 */
/**
 * The caret, blinking as Obsidian's does: drawn over the canvas at the engine's caret, shown while
 * the editor has the keyboard, and steady for a moment after every move (the blink restarts).
 */
function Caret({ caret, view, readOnly }: { caret: { x: number; y: number; height: number } | null; view: KitCanvasView | null; readOnly: boolean }) {
  const [focused, setFocused] = useState(false)
  useEffect(() => {
    const sink = document.querySelector('[data-testid="kit-text-sink"]') as HTMLTextAreaElement | null
    if (!sink) return undefined
    const update = () => setFocused(document.activeElement === sink && document.hasFocus())
    update()
    sink.addEventListener('focus', update)
    sink.addEventListener('blur', update)
    window.addEventListener('focus', update)
    window.addEventListener('blur', update)
    return () => {
      sink.removeEventListener('focus', update)
      sink.removeEventListener('blur', update)
      window.removeEventListener('focus', update)
      window.removeEventListener('blur', update)
    }
  }, [view !== null])
  if (!caret || !view || !focused || readOnly) return null
  const left = (caret.x - view.x) * view.zoom
  const top = (caret.y - view.y + 2) * view.zoom
  const height = Math.max(1, (caret.height - 4) * view.zoom)
  // Keyed by place, so each move starts the blink again from shown.
  return <div key={`${left},${top}`} className="note-caret" data-testid="note-caret" style={{ left, top, height }} />
}

function Composition({ caret, view }: { caret: { x: number; y: number; height: number } | null; view: KitCanvasView | null }) {
  const [composing, setComposing] = useState<string | null>(null)
  useEffect(() => {
    const sink = document.querySelector('[data-testid="kit-text-sink"]') as HTMLTextAreaElement | null
    if (!sink) return undefined
    const start = () => setComposing('')
    const update = (event: CompositionEvent) => setComposing(event.data ?? '')
    const end = () => setComposing(null)
    sink.addEventListener('compositionstart', start)
    sink.addEventListener('compositionupdate', update)
    sink.addEventListener('compositionend', end)
    return () => {
      sink.removeEventListener('compositionstart', start)
      sink.removeEventListener('compositionupdate', update)
      sink.removeEventListener('compositionend', end)
    }
  }, [view !== null])
  const place = caret && view ? { left: (caret.x - view.x) * view.zoom, top: (caret.y - view.y) * view.zoom, height: caret.height * view.zoom } : null
  // The text field follows the caret (inline style over kit-editor's), for the candidate window.
  useEffect(() => {
    const sink = document.querySelector('[data-testid="kit-text-sink"]') as HTMLTextAreaElement | null
    if (!sink || !place) return
    sink.style.left = `${place.left}px`
    sink.style.top = `${place.top + place.height}px`
  }, [place?.left, place?.top, place?.height])
  if (!place || !composing) return null
  return (
    <div className="note-composition" data-testid="note-composition" style={{ left: place.left, top: place.top, height: place.height, lineHeight: `${place.height}px`, fontSize: Math.round(place.height * 0.62) }}>
      {composing}
    </div>
  )
}

export type { Strings }
