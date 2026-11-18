//! `kition-note-kit`: the Note kit's engine (`kit.note`) on the Kition SDK, engine contract 2.
//!
//! A note is a Markdown file held as a KDM replica (`kition-note-kdm`): the file's text is the
//! `Note` object's `body`, so saving writes back exactly what was read plus the edits made.
//! Commands are transactions of the replica (`kition_kit::session::KitSession`); the same
//! document runs in the page as WebAssembly and headless as `kition-note`.

use kition_kit::KitCommand;
/// The document contract used by this SDK, for native hosts without a separate SDK pin.
pub use kition_kit::KitDocument;
use kition_kit::diagnostics::Diagnostic;
pub use kition_kit::kdm::Origin;
use kition_kit::kdm::{ActorId, UndoScope};
pub use kition_kit::session::KitSessionDocument;
use kition_kit::session::{Damage, KitSession, MemoryBlobs, history_scope};
use kition_note_kdm::file::{Eol, InvalidUtf8, LineStyle, decode};
use kition_note_kdm::{NoteModel, registers};
use serde_json::{Value, json};

mod clock;
mod collab;
pub mod commands;
mod continuity;
mod editor;
pub mod export;
mod files;
pub mod page;
pub mod positions;
pub mod premise;
pub mod projection;
mod run;

pub use collab::encode_blobs;

/// The editor state `inspect()` reports (the kit's editor-state list, kition-headless's volatile
/// keys): `view`. The selection, folds, the view's width and the heading under the pointer are the
/// editor's, and `inspect()` answers none of them; no command an agent is offered reads them.
pub const EDITOR_STATE: &[&str] = &["view"];

/// The file extensions a note opens and saves.
pub const EXTENSIONS: &[&str] = &["md", "markdown"];

/// The most headings `inspect()` lists (`outline.read` pages through all of them).
const INSPECT_HEADINGS: usize = 100;

/// The person a command from the in-page editor runs for when the host names none.
fn local_person() -> Origin {
    Origin::person("person:local")
}

/// One open note.
/// What the view shows besides the text (see `NoteDocument::painted`): the lines with the
/// caret, folds and pictures, the width, and the heading under the pointer.
type PaintedKey = (usize, usize, u64, u32, Option<usize>);

/// What the view was last laid out for: the revision, the caret's lines, the selection, and the
/// pictures and folds.
type ViewKey = (u64, usize, usize, usize, usize, u64);

pub struct NoteDocument {
    session: KitSession<NoteModel>,
    /// The pages, laid out when first asked for after a change (`height`, `render`, `inspect`).
    pages: std::cell::OnceCell<page::Pages>,
    revision: u64,
    saved: u64,
    /// The replica's version when the note last took in a change (`changed`): a model batch that
    /// moved it past this is a change the note has not taken in yet (`batch_applied`).
    accounted: kition_kit::kdm::Version,
    /// The text the file held when it was last read or written: the base of an external change
    /// merge ([`NoteDocument::merge_file`]).
    base: String,
    /// A file that is not valid UTF-8: its bytes, saved unchanged; the note is read-only.
    invalid: Option<(Vec<u8>, InvalidUtf8)>,
    /// The file's name, for an export's title when the note has no heading.
    name: String,
    /// The editing view, laid out again when first asked for after a change.
    view: std::cell::RefCell<kition_note_view::View>,
    /// The revision and the lines with the caret (shown with their syntax) `view` was laid out for.
    view_revision: std::cell::Cell<Option<ViewKey>>,
    /// Moves when a picture arrives, so the view lays out again.
    pictures: std::cell::Cell<u64>,
    /// What the pixels show besides the text (the lines with the caret, folds, pictures, the
    /// width, the heading under the pointer) when the revision was last asked for, and how often that changed: the reported
    /// revision moves with it, so the canvas repaints when the caret moves to another line and
    /// its syntax shows, though the document did not change.
    painted: std::cell::Cell<Option<PaintedKey>>,
    /// `inspect`'s answer for a revision of the document: indexing a large note costs, and the
    /// editor asks after every click.
    inspected: std::cell::RefCell<Option<(u64, kition_kit::kdm::Version, Value)>>,
    /// The printed page count last measured for a large note, with the length it had then.
    measured_pages: std::cell::Cell<Option<(usize, u32)>>,
    /// The note's text for the editor's commands, shared until the next revision (a large note's
    /// copied for each of the many commands a keystroke brings was a cost of its own).
    shared_text: std::cell::RefCell<Option<(u64, std::sync::Arc<str>)>>,
    paint_epoch: std::cell::Cell<u64>,
    /// The selection and the gesture in progress.
    editor: editor::Editor,
    /// The selection (anchor, focus) another person's change left, until the person moves it: the
    /// editor keeps such a caret where it is on screen instead of scrolling to it.
    remote_selection: Option<(usize, usize)>,
    /// The projection and its ids (`projection`), followed to the text when next asked.
    tracked: std::cell::RefCell<projection::Tracked>,
    /// The texts this note held when it answered an editor state, newest last: what a state
    /// taken on an earlier text is moved along from (`continuity`).
    stated: std::cell::RefCell<continuity::Stated>,
    /// The threads whose quoted text was gone when the last change was told (`carried`).
    told_orphans: std::cell::RefCell<Vec<String>>,
    /// A copy the guard plans a run's steps on (`fork_for_premises`): the text the note held when
    /// it was copied, before the run's earlier steps ran on the copy.
    planning: Option<String>,
}

fn extension(name: &str) -> String {
    std::path::Path::new(name).extension().map(|ext| ext.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

fn check_name(name: &str) -> Result<(), String> {
    let ext = extension(name);
    if EXTENSIONS.contains(&ext.as_str()) { Ok(()) } else { Err(format!("a note is a .md or .markdown file, not {name:?}")) }
}

impl NoteDocument {
    fn from_session(session: KitSession<NoteModel>) -> Self {
        let base = session.model().body().to_owned();
        let mut editor = editor::Editor::default();
        editor.follow(&base);
        let tracked = std::cell::RefCell::new(projection::Tracked::open(&base));
        let accounted = session.version();
        Self {
            session,
            pages: std::cell::OnceCell::new(),
            revision: 0,
            saved: 0,
            accounted,
            base,
            invalid: None,
            name: "Untitled note.md".into(),
            view: std::cell::RefCell::new(kition_note_view::View::default()),
            view_revision: std::cell::Cell::new(None),
            painted: std::cell::Cell::new(None),
            inspected: std::cell::RefCell::new(None),
            measured_pages: std::cell::Cell::new(None),
            shared_text: std::cell::RefCell::new(None),
            paint_epoch: std::cell::Cell::new(0),
            pictures: std::cell::Cell::new(0),
            editor,
            remote_selection: None,
            tracked,
            stated: std::cell::RefCell::default(),
            told_orphans: std::cell::RefCell::new(Vec::new()),
            planning: None,
        }
    }

    /// The projection, followed to the text now.
    fn ids(&self) -> std::cell::Ref<'_, projection::Tracked> {
        let text = self.session.model().body();
        if self.tracked.borrow().text() != text {
            self.tracked.borrow_mut().follow(text, false);
        }
        self.tracked.borrow()
    }

    /// A file that is not valid UTF-8, opened read-only: the replica holds the text with each
    /// invalid sequence shown as U+FFFD, and saving writes the original bytes back.
    fn read_only(bytes: &[u8], invalid: InvalidUtf8) -> Result<Self, String> {
        let replica = kition_note_kdm::create_replica(&invalid.lossy, LineStyle::default(), ActorId::random(), &Origin::system("import"))?;
        let mut document = Self::from_session(KitSession::from_replica(replica, Box::new(MemoryBlobs::default()))?);
        document.invalid = Some((bytes.to_vec(), invalid));
        Ok(document)
    }

    /// Whether the note refuses edits (its file is not valid UTF-8).
    pub fn is_read_only(&self) -> bool {
        self.invalid.is_some()
    }

    /// The Markdown text as the replica holds it.
    pub fn text(&self) -> &str {
        self.session.model().body()
    }

    /// The same, under a name the `text` tool's method does not shadow.
    pub fn body(&self) -> &str {
        self.session.model().body()
    }

    /// The note's pages, laid out now if a change made them stale.
    fn pages(&self) -> &page::Pages {
        self.pages.get_or_init(|| page::Pages::layout(self.session.model().body()))
    }

    /// Records that the document changed (`damage` other than none): a new revision and pages,
    /// and the selection follows the change.
    fn changed(&mut self, damage: &Damage) {
        self.remote_selection = None;
        if *damage != Damage::None {
            self.revision += 1;
            self.pages = std::cell::OnceCell::new();
            let text = self.session.model().body().to_owned();
            self.editor.follow(&text);
        }
        self.accounted = self.session.version();
    }

    /// What the view shows besides the text now (`painted`).
    fn painted_key(&self) -> PaintedKey {
        let text = self.session.model().body();
        let active = self.editor.active_lines(text);
        let folds = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            self.editor.folded.hash(&mut hasher);
            hasher.finish()
        };
        let view = self.view();
        // What the caret's lines hide: a move that shows or hides a link's or emphasis's
        // syntax repaints; one that changes nothing drawn does not.
        let shown = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            let hide = kition_note_view::style::flag::HIDE;
            if let Some(bytes) = view.styles().bytes.get(active.start.min(text.len())..active.end.min(text.len())) {
                for value in bytes {
                    (value & hide != 0).hash(&mut hasher);
                }
            }
            hasher.finish()
        };
        (active.start, active.end, self.pictures.get() ^ folds ^ shown, view.theme().width.to_bits(), view.hover())
    }

    /// The editing view, laid out for the current text.
    fn view(&self) -> std::cell::Ref<'_, kition_note_view::View> {
        let text = self.session.model().body();
        let active = self.editor.active_lines(text);
        let folds = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            self.editor.folded.hash(&mut hasher);
            hasher.finish()
        };
        let selection = self.editor.selection_bytes(text);
        let key = (self.revision, active.start, active.end, selection.start, selection.end, self.pictures.get() ^ folds);
        if self.view_revision.get() != Some(key) {
            let mut view = self.view.borrow_mut();
            view.update_selected(text, active, selection, &self.editor.folded);
            view.set_marks(run::open_anchors(text, self.session.replica()));
            drop(view);
            self.view_revision.set(Some(key));
        }
        self.view.borrow()
    }

    /// The view's pixels at scale 1, `width` × `height` from (`x`, `y`), straight alpha.
    fn full(&self, x: i32, y: i32, width: u32, height: u32) -> Vec<u8> {
        let (w, h) = (u16::try_from(width).unwrap_or(u16::MAX), u16::try_from(height).unwrap_or(u16::MAX));
        self.realized(f64::from(y), f64::from(y) + f64::from(height));
        self.view().frame(f64::from(x), f64::from(y), 1.0, w, h)
    }

    /// Lays out exactly the lines a large note only guessed between document heights `top` and
    /// `bottom` (what is about to be drawn).
    fn realized(&self, top: f64, bottom: f64) {
        let _ = self.view();
        self.view.borrow_mut().realize(top as f32, bottom as f32);
    }
}

/// Notes from this length on have their printed pages guessed (from their height on screen, then
/// scaled by their length), not laid out for print.
const MEASURE_PAGES_BELOW: usize = 200_000;

/// The screen height (document pixels) of one printed letter page's text, for that guess.
const ESTIMATED_PAGE_HEIGHT: f64 = 864.0;

impl NoteDocument {
    /// The note's page count when printed. Exact for a shorter note; a large one's (laying out its
    /// pages takes long) is guessed from its height on screen and scaled by its length after.
    fn page_count(&self) -> u32 {
        let length = self.text().len();
        if length < MEASURE_PAGES_BELOW {
            return self.pages().pages();
        }
        if let Some((measured, pages)) = self.measured_pages.get() {
            let ratio = length as f64 / measured.max(1) as f64;
            return ((f64::from(pages) * ratio).round() as u32).max(1);
        }
        // Not measured: a large note's printed pages are guessed from its height on screen (a
        // letter page holds about 864 px of the column), so opening it does not lay it out for print.
        let pages = (self.view().height() as f64 / ESTIMATED_PAGE_HEIGHT).ceil().max(1.0) as u32;
        self.measured_pages.set(Some((length, pages)));
        pages
    }

    /// `inspect`'s answer, computed (see `inspected`).
    fn inspect_now(&self) -> Result<Value, String> {
        let text = self.text();
        // From the view's chunks (memoised by their text), not the whole note indexed again.
        let view = self.view();
        let summary = view.summary();
        let line_of = |at: usize| view.styles().line_of(at) + 1;
        let replica = self.session.replica();
        let threads = kition_kit::kdm::comment::threads(replica).unwrap_or_default();
        let mut stack: Vec<(u8, String)> = Vec::new();
        let outline: Vec<Value> = summary
            .headings
            .iter()
            .take(INSPECT_HEADINGS)
            .map(|(level, heading, at)| {
                while stack.last().is_some_and(|(above, _)| *above >= *level) {
                    stack.pop();
                }
                stack.push((*level, heading.clone()));
                let path: Vec<&String> = stack.iter().map(|(_, text)| text).collect();
                json!({ "path": path, "level": level, "line": line_of(*at) })
            })
            .collect();
        let properties: Vec<Value> = summary
            .frontmatter
            .iter()
            .flat_map(|fm| fm.entries.iter())
            .map(|entry| json!({ "key": entry.key, "type": entry.typed.as_ref().map_or("unknown", kition_note_markdown::Typed::kind), "complex": matches!(entry.style, kition_note_markdown::Style::Complex(_)) }))
            .collect();
        let links = view.links();
        let count = |kind: kition_note_markdown::LinkKind| links.iter().filter(|link| link.kind == kind).count();
        Ok(json!({
            "canUndo": self.session.can_undo(),
            "canRedo": self.session.can_redo(),
            "heading": summary.headings.first().map(|(_, heading, _)| heading.clone()),
            "outline": outline,
            "headings": summary.headings.len(),
            "properties": properties,
            "links": {
                "wikilinks": count(kition_note_markdown::LinkKind::Wikilink),
                "markdown": count(kition_note_markdown::LinkKind::Markdown) + count(kition_note_markdown::LinkKind::Reference) + count(kition_note_markdown::LinkKind::Autolink),
                "embeds": count(kition_note_markdown::LinkKind::Embed) + count(kition_note_markdown::LinkKind::Image),
                "urls": count(kition_note_markdown::LinkKind::BareUrl),
                "tags": summary.tags,
                "blockIds": summary.block_ids,
            },
            "tables": view.index_tables().iter().enumerate().map(|(number, table)| json!({ "table": number, "line": line_of(table.range.start), "rows": table.rows.len(), "columns": table.columns() })).collect::<Vec<_>>(),
            "tasks": { "open": summary.tasks_open, "done": summary.tasks_done },
            "callouts": summary.callouts,
            "words": summary.words,
            // (Counted by the bytes that start a character: the same count, a large note's faster.)
            "characters": text.bytes().filter(|byte| (byte & 0xC0) != 0x80).count(),
            "lines": if text.is_empty() { 0 } else { text.split('\n').count() },
            "pages": self.page_count(),
            "registers": registers(replica),
            "comments": { "open": threads.iter().filter(|thread| !thread.resolved).count(), "resolved": threads.iter().filter(|thread| thread.resolved).count() },
            "openThreads": threads.iter().filter(|thread| !thread.resolved).count(),
            "readOnly": self.is_read_only(),
        }))
    }
}

impl NoteDocument {
    /// Each command's parameter schema, as the catalogue offers it to agents; read once, since a
    /// command's parameters do not change with the note.
    fn parameter_schemas(&self) -> &'static std::collections::HashMap<String, Value> {
        static SCHEMAS: std::sync::OnceLock<std::collections::HashMap<String, Value>> = std::sync::OnceLock::new();
        SCHEMAS.get_or_init(|| self.commands().into_iter().filter_map(|command| Some((command.id, command.schema?))).collect())
    }
}

impl KitDocument for NoteDocument {
    /// The projection's sections, tasks, tables, links and properties as objects of the object
    /// model (`projection::objects`), by the ids `projection.read` gives.
    fn projected_objects(&self) -> Result<Vec<kition_kit::kdm::projection::ProjectedObject>, String> {
        Ok(projection::objects(self.ids().now(), premise::ROOT))
    }

    /// A committed run of the object model's operations (`model/batch`: extensions, workspace
    /// scripts, kit-env) is a change of the note as a command's is: the open typing run ends, the
    /// revision moves (the editor draws it, and the note has unsaved changes), the pages are laid
    /// out again, the selection follows the text and the projection's ids follow it. A command
    /// inside the batch has taken in its own change already (`changed`), so it is not counted twice.
    fn batch_applied(&mut self) {
        if self.session.version() == self.accounted {
            return;
        }
        self.stop_typing();
        self.changed(&Damage::Full);
        drop(self.ids());
    }

    /// Opens a Markdown file. Every byte string is a note: valid UTF-8 opens for editing and saves
    /// back byte for byte; anything else opens read-only and saves its bytes unchanged.
    fn open(name: &str, bytes: &[u8]) -> Result<Self, String> {
        check_name(name)?;
        // (Valid UTF-8 checked without decoding the note twice: the import decodes it.)
        let valid = std::str::from_utf8(bytes.strip_prefix(kition_note_kdm::file::BOM).unwrap_or(bytes)).is_ok();
        let checked = if valid { Ok(()) } else { decode(bytes).map(drop) };
        let mut document = match checked {
            Err(invalid) => Self::read_only(bytes, invalid)?,
            Ok(()) => Self::from_session(KitSession::<NoteModel>::import(name, bytes, Box::new(MemoryBlobs::default()), ActorId::random())?),
        };
        document.name = name.to_owned();
        // The inline title: the file's name without its folder and extension, as Obsidian's.
        let stem = name.rsplit(['/', '\\']).next().unwrap_or(name);
        let stem = stem.strip_suffix(".md").or_else(|| stem.strip_suffix(".markdown")).unwrap_or(stem);
        if !stem.is_empty() {
            document.view.borrow_mut().set_title(Some(stem.to_owned()));
        }
        Ok(document)
    }

    /// A new, empty note. `name` is only a title (a host may pass `untitled`); saving writes
    /// Markdown whatever it is.
    fn create(_name: &str, _width: u32, _height: u32) -> Result<Self, String> {
        let replica = kition_note_kdm::create_replica("", LineStyle::default(), ActorId::random(), &Origin::system("create"))?;
        Ok(Self::from_session(KitSession::from_replica(replica, Box::new(MemoryBlobs::default()))?))
    }

    fn width(&self) -> u32 {
        // The page's width does not depend on the text: no layout for it.
        self.view.borrow().width()
    }

    fn height(&self) -> u32 {
        self.view().height()
    }

    /// The document's revision, moved also by every change of what the view shows without a
    /// change of the document (`painted`). Unsaved changes still follow the document's own.
    fn revision(&self) -> u64 {
        let key = self.painted_key();
        if self.painted.get() != Some(key) {
            if self.painted.get().is_some() {
                self.paint_epoch.set(self.paint_epoch.get() + 1);
            }
            self.painted.set(Some(key));
        }
        self.revision + self.paint_epoch.get()
    }

    fn dirty(&self) -> bool {
        self.revision != self.saved
    }

    /// The editing view (Word's rule): below scale 1 the box mean of the full-size pixels, at 1 those
    /// pixels, above 1 drawn again at the scale so text stays sharp.
    fn render(&self, x: f64, y: f64, scale: f64, width: u32, height: u32) -> Result<Vec<u8>, String> {
        let (width, height) = (width.clamp(1, kition_kit::MAX_RENDER_SIDE), height.clamp(1, kition_kit::MAX_RENDER_SIDE));
        let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
        if scale < 1.0 {
            let (x0, y0) = (x.floor() as i32, y.floor() as i32);
            let span_x = ((f64::from(width) / scale).ceil() as i32).max(1);
            let span_y = ((f64::from(height) / scale).ceil() as i32).max(1);
            return self.render_area(x0, y0, x0.saturating_add(span_x), y0.saturating_add(span_y), width, height);
        }
        if scale == 1.0 && x.fract() == 0.0 && y.fract() == 0.0 {
            return Ok(self.full(x as i32, y as i32, width, height));
        }
        let (w, h) = (u16::try_from(width).unwrap_or(u16::MAX), u16::try_from(height).unwrap_or(u16::MAX));
        self.realized(y, y + f64::from(height) / scale);
        Ok(self.view().frame(x, y, scale, w, h))
    }

    fn render_area(&self, x0: i32, y0: i32, x1: i32, y1: i32, width: u32, height: u32) -> Result<Vec<u8>, String> {
        let (width, height) = (width.clamp(1, kition_kit::MAX_RENDER_SIDE), height.clamp(1, kition_kit::MAX_RENDER_SIDE));
        let w = u32::try_from(x1.saturating_sub(x0).max(1)).unwrap_or(1).min(kition_kit::MAX_RENDER_SIDE);
        let h = u32::try_from(y1.saturating_sub(y0).max(1)).unwrap_or(1).min(kition_kit::MAX_RENDER_SIDE);
        let full = self.full(x0, y0, w, h);
        Ok(kition_kit::raster::render_area(kition_kit::raster::Frame { rgba: &full, width: w, height: h }, 0, 0, w as i32, h as i32, width, height))
    }

    fn execute(&mut self, command: &str, params: Value) -> Result<Value, String> {
        self.execute_as(command, params, &local_person())
    }

    /// What `note_inspect` shows: the outline, properties, links, tables, tasks and comments in
    /// short, the size, and the line style.
    fn inspect(&self) -> Result<Value, String> {
        // Comments change the replica's version without a new revision.
        let version = self.session.version();
        if let Some((revision, seen, value)) = self.inspected.borrow().as_ref()
            && *revision == self.revision
            && *seen == version
        {
            let mut value = value.clone();
            if let Some(object) = value.as_object_mut() {
                object.insert("canUndo".into(), json!(self.session.can_undo()));
                object.insert("canRedo".into(), json!(self.session.can_redo()));
            }
            return Ok(value);
        }
        let value = self.inspect_now()?;
        drop(self.ids());
        *self.inspected.borrow_mut() = Some((self.revision, version, value.clone()));
        Ok(value)
    }

    fn commands(&self) -> Vec<KitCommand> {
        let mut commands = commands::list(self.session.can_undo(), self.session.can_redo(), self.is_read_only());
        kition_kit::files::declare(&mut commands, files::FILES);
        commands
    }

    fn save(&self, name: Option<&str>) -> Result<Vec<u8>, String> {
        if let Some(name) = name {
            check_name(name)?;
        }
        if let Some((bytes, _)) = &self.invalid {
            return Ok(bytes.clone());
        }
        Ok(self.session.export(name.unwrap_or("note.md"))?)
    }

    /// The note as HTML or PDF ([`export::EXPORTS`]), as `file.export` writes it with `options`
    /// (its parameters but `format`); the note does not change.
    fn export(&self, format: &str, options: &Value) -> Result<Vec<u8>, String> {
        export::write(self.text(), &self.name, format, options).map(|written| written.bytes)
    }

    fn exports() -> Vec<&'static str> {
        export::EXPORTS.to_vec()
    }

    fn mark_saved(&mut self) {
        self.saved = self.revision;
        self.base = self.text().to_owned();
    }

    /// The selection and the folds, with the text they are offsets of (`continuity`).
    fn editor_state(&self) -> Value {
        self.state_now()
    }

    /// Puts back what [`KitDocument::editor_state`] answered, moved along every change since
    /// (`continuity`); no edit.
    fn restore_editor_state(&mut self, saved: &Value) -> Result<(), String> {
        self.put_back_state(saved)
    }

    /// The comment threads, the projection's ids and the text the editor state refers to, carried
    /// from the note this one replaces (`continuity`).
    fn carry_replica_objects(&mut self, previous: &Self) -> Result<(), String> {
        self.carry_from(previous)
    }

    /// The file's difference taken into this session as an outside change (`continuity`).
    fn reconcile_reread(&mut self, fresh: &Self) -> Result<bool, String> {
        self.reconcile(fresh)
    }

    /// What a write rests on (`premise`): the root under a place in the body, held by `covers`.
    fn under(&self, command: &str, params: &Value) -> Result<Option<kition_kit::Under>, String> {
        if let Some(scope) = history_step(command, params)? {
            // What the step it takes back or applies again wrote (kition-sdk undo footprints).
            let touches = if command == "edit.undo" { self.session.undo_touches_in(scope) } else { self.session.redo_touches_in(scope) };
            return Ok(Some(premise::history_under(&touches)));
        }
        Ok(if commands::mutates(command) { premise::under(command, params) } else { None })
    }

    /// A copy for the guard to run a run's earlier steps on before it asks what a later one rests
    /// on: the same replica (object ids, threads), the projection's ids and the editor's state.
    fn fork_for_premises(&self) -> Result<Self, String> {
        let session = self.session.fork(ActorId::random(), Box::new(MemoryBlobs::default()))?;
        let mut copy = Self::from_session(session);
        copy.name = self.name.clone();
        copy.invalid = self.invalid.clone();
        copy.editor = self.editor.clone();
        *copy.tracked.borrow_mut() = self.ids().clone();
        *copy.told_orphans.borrow_mut() = self.told_orphans.borrow().clone();
        copy.planning = Some(self.text().to_owned());
        Ok(copy)
    }

    fn covered(&self, command: &str, params: &Value, objects: &[kition_kit::CoveredAsk]) -> Result<Option<kition_kit::CoveredAt>, String> {
        Ok(premise::covered(self.planning.as_deref(), command, params, objects, self.text(), &self.ids()))
    }

    /// What a change of the note carries (`model.carried`): the comment threads whose quoted text
    /// went since the last change told (a change set of a file read again holds no thread), on the
    /// root's change; a thread's own change says when the text it quotes is gone. What changed in
    /// the projection is told by its objects (`projected_objects`).
    fn carried(&self, ids: &[Value], _changes: &[Value]) -> Result<Vec<kition_kit::Carried>, String> {
        let mut out = Vec::new();
        let orphans = run::orphaned(self.text(), self.session.replica());
        if ids.iter().any(|id| id.as_str() == Some(premise::ROOT)) {
            let mut known = self.told_orphans.borrow_mut();
            let what: Vec<String> = orphans
                .iter()
                .filter(|(thread, _)| !known.contains(thread))
                .map(|(thread, quote)| format!("comment thread {thread} orphaned: the text it quotes, {quote:?}, is gone"))
                .collect();
            *known = orphans.iter().map(|(thread, _)| thread.clone()).collect();
            if !what.is_empty() {
                out.push(kition_kit::Carried {
                    id: json!(premise::ROOT),
                    carries: kition_kit::Carries { count: what.len() as u64, what: what.join("; ") },
                });
            }
        }
        for id in ids.iter().filter_map(Value::as_str) {
            if let Some(quote) = orphans.iter().find(|(thread, _)| thread == id).map(|(_, quote)| quote) {
                out.push(kition_kit::Carried {
                    id: json!(id),
                    carries: kition_kit::Carries { count: 1, what: format!("orphaned: the text it quotes, {quote:?}, is not in the note now") },
                });
            }
        }
        Ok(out)
    }

    fn unproven_identities(&self, objects: &[Value]) -> Result<Vec<Value>, String> {
        let replica = self.session.replica();
        Ok(premise::unproven(objects, |id| replica.exists(&kition_kit::kdm::ObjectId::from_string(id))))
    }

    fn content_at(&self, ask: &kition_kit::ContentAsk, _history: &dyn kition_kit::HistoricalBlobs) -> Result<Option<bool>, String> {
        Ok(premise::content_at(self.planning.as_deref(), ask, self.text(), &self.ids()))
    }

    /// `note_check`: the twelve codes of `kition-note-check` (`agent/checks.json`).
    fn check(&self) -> Vec<Diagnostic> {
        let text = self.text();
        let ix = kition_note_markdown::index(text);
        kition_note_check::check(&kition_note_check::Input {
            text,
            index: &ix,
            invalid_utf8: self.invalid.as_ref().map(|(_, invalid)| invalid.at),
            mixed_line_endings: self.session.model().style().eol == Eol::Mixed,
            files: &files::HostFiles,
        })
    }
}

/// The scope of an `edit.undo` or `edit.redo` an agent runs: its `scope`, else the agent's own.
fn history_step(command: &str, params: &Value) -> Result<Option<UndoScope>, String> {
    if !matches!(command, "edit.undo" | "edit.redo") {
        return Ok(None);
    }
    Ok(Some(history_scope(params, &Origin::agent("premise"))?))
}

impl KitSessionDocument for NoteDocument {
    type Model = NoteModel;

    fn session(&self) -> &KitSession<NoteModel> {
        &self.session
    }

    fn session_mut(&mut self) -> &mut KitSession<NoteModel> {
        &mut self.session
    }

    fn execute_as(&mut self, command: &str, params: Value, origin: &Origin) -> Result<Value, String> {
        if !commands::is_known(command) {
            return Err(format!("unknown command {command}"));
        }
        let params = if params.is_null() { json!({}) } else { params };
        if !params.is_object() {
            return Err("invalid-params: params must be an object".into());
        }
        // A key the command's schema does not name is refused, naming it (platform rule E7a).
        if let Some(schema) = self.parameter_schemas().get(command) {
            kition_kit::schema::refuse_unknown(command, schema, &params).map_err(|message| format!("invalid-params: {message}"))?;
        }
        if self.is_read_only() && commands::mutates(command) {
            return Err(format!("read-only: the file is not valid UTF-8, so {command} cannot change it"));
        }
        // A command from elsewhere that changes something (the note, the selection, the view) ends
        // the person's typing run, so it is an undo step of its own. Reads do not: the editor's
        // overlays ask after every keystroke (code blocks, diagrams, properties, tables), and a run
        // they broke would undo a character at a time.
        let reads = !commands::mutates(command) && (!command.starts_with("selection.") || matches!(command, "selection.get" | "selection.where"));
        let reads = reads && !matches!(command, "view.fold" | "view.foldAll" | "view.unfoldAll" | "view.width");
        // The editor's own commands take the undo step as Obsidian's do (a Cmd+B joins the word
        // typed before it).
        if !(self.editor.inside || reads || (commands::is_editor(command) && commands::mutates(command))) {
            self.stop_typing();
        }
        if commands::is_editor(command) {
            let answer = self.run_editor(command, &params);
            if command.starts_with("selection.") && command != "selection.get" && command != "selection.where" {
                self.unfold_at_caret();
            }
            return answer;
        }
        // An id of the projection stands for the line of what it names now.
        let params = if projection::named(&params).is_empty() { params } else { projection::resolve(&params, self.ids().now())? };
        match command {
            "file.export" | "page.layout" => export::run(self.text(), &self.name, command, &params),
            // In the scope its `scope` parameter names, else the origin's (E14a, M12a).
            "edit.undo" => {
                let (done, damage) = KitSessionDocument::undo_in(self, history_scope(&params, origin)?)?;
                Ok(json!({ "done": done, "damage": damage }))
            }
            "edit.redo" => {
                let (done, damage) = KitSessionDocument::redo_in(self, history_scope(&params, origin)?)?;
                Ok(json!({ "done": done, "damage": damage }))
            }
            _ if !commands::mutates(command) => {
                let ids = self.ids();
                Ok(run::read(self.session.model(), self.session.replica(), command, &params, ids.now())?)
            }
            _ => {
                let (value, damage) = self.session.execute(origin, |model, tx| run::write(model, tx, command, &params, origin))?;
                self.changed(&damage);
                // Every state an agent's write leaves is one its next write may rest on.
                drop(self.ids());
                Ok(value)
            }
        }
    }

    /// Another person's changes (`replica/import`), followed as every change by others is
    /// (`NoteDocument::changed_outside`).
    fn import_changes(&mut self, bytes: &[u8]) -> Result<Damage, String> {
        let damage = self.session.import_changes(bytes)?;
        self.changed_outside(&damage);
        Ok(damage)
    }

    fn undo(&mut self) -> Result<(bool, Damage), String> {
        self.undo_in(UndoScope::Person)
    }

    fn redo(&mut self) -> Result<(bool, Damage), String> {
        self.redo_in(UndoScope::Person)
    }

    // The agent's scope keeps the revision and pages current too, as `undo` and `redo` do.
    fn undo_in(&mut self, scope: UndoScope) -> Result<(bool, Damage), String> {
        // The open step ends first, so it is one step (as Obsidian takes back a run of typing).
        self.stop_typing();
        if scope == UndoScope::Person && self.skip_empty(true) {
            return Ok((true, Damage::None));
        }
        let record = self.next_record(true).filter(|_| scope == UndoScope::Person);
        let (done, damage) = self.session.undo_in(scope)?;
        self.changed(&damage);
        // The person's step puts back the selection it had before (as Obsidian's Undo does).
        if done && scope == UndoScope::Person {
            self.restore_selection(true, record);
        }
        Ok((done, damage))
    }

    fn redo_in(&mut self, scope: UndoScope) -> Result<(bool, Damage), String> {
        self.stop_typing();
        if scope == UndoScope::Person && self.skip_empty(false) {
            return Ok((true, Damage::None));
        }
        let record = self.next_record(false).filter(|_| scope == UndoScope::Person);
        let before = self.session.model().body().to_owned();
        let (done, damage) = self.session.redo_in(scope)?;
        self.changed(&damage);
        // The person's step puts back the selection it ended with; another's, the caret after it.
        if !(done && scope == UndoScope::Person && self.restore_selection(false, record)) {
            self.caret_after_redo(&before);
        }
        Ok((done, damage))
    }
}

impl NoteDocument {
    /// The text changed by others (an imported change, the file read again): the caret, the
    /// selection, folds and the undo records move with the change exactly
    /// (`Editor::follow_remote`). A change to the text ends the open typing run, as the replica's
    /// undo group ends there (an import that changes what the group changed closes it), so each
    /// Undo takes back what its record says; the replica's last local step is still the editor's
    /// last record.
    fn changed_outside(&mut self, damage: &Damage) {
        let text = self.session.model().body().to_owned();
        let selection = (self.editor.anchor, self.editor.focus);
        if self.editor.text() != text {
            self.base = editor::rebase(&self.base, self.editor.text(), &text);
            self.editor.follow_remote(&text);
            self.stop_typing();
        }
        let moved = (self.editor.anchor, self.editor.focus) != selection;
        let remote = self.remote_selection;
        let revision = self.revision;
        self.changed(damage);
        self.editor.keep_records(revision, self.revision);
        // A caret another change moved, or one it had moved before that the person has not moved since.
        let still = remote == Some(selection);
        self.remote_selection = (moved || still).then_some((self.editor.anchor, self.editor.focus));
    }
}

impl NoteDocument {
    /// After a Redo the caret goes to the end of what it brought back, as Obsidian's editor does.
    fn caret_after_redo(&mut self, before: &str) {
        let after = self.session.model().body();
        if before == after {
            return;
        }
        let prefix = before.bytes().zip(after.bytes()).take_while(|(a, b)| a == b).count();
        let suffix = before.bytes().rev().zip(after.bytes().rev()).take_while(|(a, b)| a == b).count().min(before.len().min(after.len()) - prefix);
        let mut end = after.len() - suffix;
        while end > 0 && !after.is_char_boundary(end) {
            end -= 1;
        }
        self.editor.anchor = end;
        self.editor.focus = end;
    }
}

kition_kit::export_kit_document!(NoteDocument as "NoteDocument");
kition_kit::export_kit_session!(NoteDocument as "NoteDocument");
kition_kit::export_kit_tools!(NoteDocument as "NoteDocument");

/// The editor's extras on the exported class (`KitEngine.invoke`): fonts before a note opens
/// (Kition platform E9), and the caret and selection the page needs for scrolling and the clipboard.
#[cfg(target_arch = "wasm32")]
mod web {
    use kition_kit::wasm_bindgen::prelude::*;

    use crate::__kition_kit_export::WebDocument;

    /// Registers a font file before a note opens (Kition platform E9; kit-editor calls it for each of the
    /// manifest's `fonts`); answers its family name.
    #[wasm_bindgen(js_name = addFont, wasm_bindgen = kition_kit::wasm_bindgen)]
    pub fn add_font(bytes: Vec<u8>) -> Result<String, JsError> {
        kition_note_fonts::add_font(bytes).map_err(|error| JsError::new(&error))
    }

    #[wasm_bindgen(js_class = "NoteDocument", wasm_bindgen = kition_kit::wasm_bindgen)]
    impl WebDocument {
        /// `{ x, y, height }` of the caret, in document pixels.
        #[wasm_bindgen(js_name = caretRect)]
        pub fn caret_rect(&self) -> String {
            self.0.caret_rect().to_string()
        }

        /// The selected text, for copy and cut.
        #[wasm_bindgen(js_name = selectedText)]
        pub fn selected_text(&self) -> String {
            self.0.selected_text()
        }

        /// What this person shares as presence: `{ selection, caret, anchor }` (JSON).
        #[wasm_bindgen(js_name = presenceCaret)]
        pub fn presence_caret(&self) -> String {
            self.0.presence_caret().to_string()
        }

        /// Where other people's carets and selections are in this view: `places` is JSON
        /// `[{ caret, anchor? }]`; the answer one `{ caret, rects }` per place (JSON).
        #[wasm_bindgen(js_name = presenceLocate)]
        pub fn presence_locate(&self, places: &str) -> String {
            let places = serde_json::from_str(places).unwrap_or(serde_json::Value::Null);
            self.0.presence_locate(&places).to_string()
        }
    }
}

#[cfg(test)]
mod coedit_tests;
#[cfg(test)]
mod command_tests;
#[cfg(test)]
mod round_trip_tests;
#[cfg(test)]
mod tests;
