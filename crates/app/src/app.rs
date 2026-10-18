mod settings;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use std::sync::Arc;

use gpui::{
    Animation, AnimationExt as _, AnyElement, App, Bounds, ClickEvent, Context, CursorStyle,
    ElementId, Entity, FocusHandle, Focusable, FontWeight, Hsla, ListAlignment, ListOffset,
    ListState, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PathBuilder, Pixels,
    Point, RenderImage, SharedString, Subscription, Window, actions, canvas, div, ease_out_quint,
    img, list, point, prelude::*, px, size,
};
use gyotaku_core::trash::{self, Trashed};
use gyotaku_core::{Config, Hit, Index, Line, Query, Rect, Shot, ThemeChoice};

use crate::grid::{self, Row};
use crate::images::{self, Images, Lookup};
use crate::input::{Changed, TextInput};
use crate::keys;
use crate::platform;
use crate::spring::Spring;
use crate::stats::FrameStats;
use crate::theme::Theme;
use settings::{Key, Onboarding, Settings};

actions!(
    gyotaku,
    [
        Back,
        Open,
        Up,
        Down,
        Left,
        Right,
        PageUp,
        PageDown,
        CopyText,
        CopyImage,
        OpenExternal,
        Reveal,
        Quit,
        OpenSettings,
        Toggle,
        Remove,
        MarkUp,
        MarkDown,
        MarkLeft,
        MarkRight,
        MarkAll,
        Trash,
        Undo,
        Similar,
        AddFolder
    ]
);

const PAD: f32 = 16.0;
const GAP: f32 = 8.0;
const ROW_HEIGHT: f32 = 172.0;
const RADIUS: f32 = 10.0;
// Tiles sit PAD in from the panel edge, so the panel's corner is the tile's
// radius plus that distance and the two curves run parallel.
const PANEL_RADIUS: f32 = RADIUS + PAD;
const HEADER: f32 = 64.0;

const DETAIL_TOP: f32 = 56.0;
const DETAIL_BOTTOM: f32 = 52.0;
const DETAIL_PAD: f32 = 20.0;

// Motion is kept short everywhere: this is summoned, used and dismissed in a
// few seconds, dozens of times a day. Springs are in seconds of response,
// critically damped, and closing is quicker than opening.
const OPEN_RESPONSE: f32 = 0.26;
const CLOSE_RESPONSE: f32 = 0.2;
const FADE_RESPONSE: f32 = 0.14;
const PRESS: Duration = Duration::from_millis(110);
const THUMBS_KEPT_HIDDEN: usize = 40;
const REPAINT_BATCH: Duration = Duration::from_millis(20);
const TOAST_SHOWN: Duration = Duration::from_millis(1400);
// Long enough to read it and reach for ctrl z.
const TOAST_UNDO: Duration = Duration::from_millis(5000);
const TOAST_FADE: Duration = Duration::from_millis(120);
// A marked tile's picture shrinks inside its frame, the way photo apps show
// a selection, so marked and unmarked read apart at a glance without colour.
const MARK_INSET: f32 = 7.0;
const MARK: Duration = Duration::from_millis(130);
// Trashed tiles shrink away (and restored ones grow back) over this long,
// then the grid closes the gap.
const TILE_FADE: Duration = Duration::from_millis(180);
const BAR_IN: Duration = Duration::from_millis(140);
// Leaving is quicker than arriving, it's the less interesting half.
const BAR_OUT: Duration = Duration::from_millis(110);
// The reading bar takes this long to catch up with each new count, which
// arrives every tick, so it moves steadily instead of in steps.
const READING_GLIDE: Duration = Duration::from_millis(600);
const BAR_SWAP: Duration = Duration::from_millis(110);

const BROWSE_LIMIT: usize = 20_000;
const SEARCH_LIMIT: usize = 2_000;

// A few screens of thumbnails either side of where you are. A 480 px wide
// thumbnail is ~0.5 MB decoded, so this tops out around 60 MB.
const THUMB_CACHE: usize = 120;
const THUMB_MAX_SIDE: u32 = 1440;
// The full view keeps the one on screen and the one it just left, and never
// decodes past 4K on a side.
const FULL_CACHE: usize = 2;
const FULL_MAX_SIDE: u32 = 3840;

pub struct Gyotaku {
    index: Index,
    input: Entity<TextInput>,
    query: String,
    /// `query` split into words and filters (`in:`, `date:` and so on).
    parsed: Query,
    /// What `hits` was cut off at, so the count can say "2,000+".
    limit: usize,
    /// The tiles, in order. A burst of near-identical shots shows as its
    /// first one alone, unless it's unfolded (see `gyotaku_core::burst`).
    hits: Vec<Hit>,
    /// Every burst with more than one result: the results after its first,
    /// whether they're on screen or folded away behind it.
    stacks: HashMap<i64, Vec<Hit>>,
    /// The bursts showing all their shots, until the next search.
    unfolded: HashSet<i64>,
    /// Stacking similar shots is on in settings.
    grouped: bool,
    /// How many shots the query found, folded ones included.
    found: usize,
    /// The lines each hit matched on, looked up the first time its tile is
    /// drawn rather than for every hit up front.
    matched: HashMap<i64, Rc<Vec<Line>>>,
    thumb_paths: Vec<PathBuf>,
    thumbs: Images,
    full: Images,
    searchable: usize,

    rows: Vec<Row>,
    located: Vec<(usize, usize)>,
    laid_out_for: f32,
    list: ListState,
    selected: usize,
    /// Bumped on every search so the ink press replays on the new results.
    generation: usize,
    /// Where each visible tile was painted last frame, for the open animation.
    tile_bounds: Rc<RefCell<HashMap<usize, Bounds<Pixels>>>>,

    detail: Option<Detail>,
    toast: Option<Toast>,
    toasts: usize,
    last_frame: Instant,
    /// Drawn as a floating panel (layer shell) rather than filling a window.
    floating: bool,
    appearance: Option<Subscription>,
    activation: Option<Subscription>,
    /// Scroll the selection back into view after the next layout, because
    /// relaying out the list loses its scroll position.
    reveal_selected: bool,
    /// The shot at the top of the view and how far into its row, so a
    /// rebuilt list can be put back where it was.
    keep_top: Option<(i64, Pixels)>,

    page: Page,
    /// Focus for settings and onboarding, so typing there doesn't land in
    /// the search field.
    panel_focus: FocusHandle,
    theme_choice: ThemeChoice,
    /// Whether a window is showing this right now.
    visible: bool,
    stats: FrameStats,
    /// Drawing on the cpu (no usable gpu), see `calm`.
    software: bool,
    repaint_pending: bool,
    /// What the reader is doing, checked every tick.
    reader: Option<ReaderState>,
    /// The header's progress bar glides from one reading to the next:
    /// (from, to, since).
    reading_bar: (f32, f32, Instant),
    /// What the header said while reading, kept so it can fade out once the
    /// reader is done, and since when it's been fading.
    reading_last: Option<(String, Option<f32>)>,
    reading_leaving: Option<Instant>,

    /// Shots marked for the trash, by id so they survive a refresh, with
    /// when they were marked so the change can animate.
    marked: HashMap<i64, Instant>,
    /// Recently unmarked ones, still animating back.
    unmarking: HashMap<i64, Instant>,
    /// Where a shift selection started, and what was marked before it, so
    /// moving back over the range unmarks it again like a file manager does.
    anchor: Option<(usize, HashSet<i64>)>,
    /// Asking "move these to the trash?" and waiting for enter or escape.
    confirming: bool,
    /// The last batch moved to the trash, for ctrl z.
    undo: Vec<Binned>,
    /// Bumped each time the mark bar appears, so its entrance replays.
    bar_shown: Option<usize>,
    bars: usize,
    /// What the bar said last, kept so it can fade out still saying it.
    bar_last: Option<Bar>,
    bar_leaving: Option<Instant>,
    /// Tiles on their way out to the trash, or back in from it.
    fading: HashMap<i64, Fade>,
    /// Bumped per trash, so a stale "close the gap" timer does nothing.
    leaves: usize,
}

#[derive(Clone, Copy)]
struct Fade {
    arriving: bool,
    since: Instant,
    /// Leaving tiles keep their marked look all the way out.
    marked: bool,
}

#[derive(Clone, Copy, PartialEq)]
struct Bar {
    count: usize,
    confirming: bool,
    can_mark_more: bool,
}

/// Everything needed to put a trashed shot back, index rows included, so
/// undoing doesn't make the watcher read it all over again.
struct Binned {
    trashed: Trashed,
    shot: Shot,
    lines: Vec<Line>,
}

enum Page {
    Search,
    Settings(Settings),
    Onboarding(Onboarding),
}

struct Toast {
    message: SharedString,
    /// A key hint shown after the message, like "ctrl z undo".
    keys: Option<(SharedString, &'static str)>,
    id: usize,
    leaving: bool,
}

struct Detail {
    hit: usize,
    lines: Vec<Line>,
    matched: Vec<bool>,
    /// 0 is the tile in the grid, 1 is the full view.
    open: Spring,
    /// Whether it moves out of the tile, or just fades in place.
    grow: bool,
    from: Bounds<Pixels>,
    image_rect: Bounds<Pixels>,
    preview_area: Bounds<Pixels>,
    hovered: Option<usize>,
    drag: Option<(Point<Pixels>, Point<Pixels>)>,
    picked: Vec<usize>,
    zoom: crate::zoom::Zoom,
}

impl Gyotaku {
    pub fn new(index: Index, floating: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let searchable = index.visible_len().unwrap_or(0);
        let input = cx.new(|cx| TextInput::new(placeholder(searchable), cx));
        // Only when the text really changed: backspace in an empty field (a
        // natural reflex with shots marked) would otherwise clear the marks,
        // close the open shot and jump back to the top.
        cx.subscribe(&input, |this, input, _: &Changed, cx| {
            if *input.read(cx).content != *this.query {
                this.search(cx);
            }
        })
        .detach();
        // Only a missing config means "never set up". One that doesn't parse
        // (a typo made by hand) falls back to defaults rather than sending
        // someone through onboarding, which would overwrite their file.
        let (config, set_up) = match Config::load() {
            Ok(Some(config)) => (config, true),
            Ok(None) => (Config::default(), false),
            Err(_) => (Config::default(), true),
        };

        // While a backfill runs, new shots keep landing in the index. Checking
        // the count now and then lets them show up without reopening.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(1500))
                    .await;
                if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();

        let mut this = Self {
            index,
            input,
            query: String::new(),
            parsed: Query::default(),
            limit: BROWSE_LIMIT,
            hits: Vec::new(),
            stacks: HashMap::new(),
            unfolded: HashSet::new(),
            grouped: config.group_similar,
            found: 0,
            matched: HashMap::new(),
            thumb_paths: Vec::new(),
            thumbs: Images::new(THUMB_CACHE, THUMB_MAX_SIDE),
            full: Images::new(FULL_CACHE, FULL_MAX_SIDE),
            searchable,
            rows: Vec::new(),
            located: Vec::new(),
            laid_out_for: 0.0,
            list: ListState::new(0, ListAlignment::Top, px(ROW_HEIGHT * 3.0)),
            selected: 0,
            generation: 0,
            tile_bounds: Rc::default(),
            detail: None,
            toast: None,
            toasts: 0,
            last_frame: Instant::now(),
            floating,
            appearance: None,
            activation: None,
            reveal_selected: false,
            keep_top: None,
            page: Page::Search,
            panel_focus: cx.focus_handle(),
            theme_choice: config.theme,
            visible: true,
            stats: FrameStats::new(),
            software: false,
            repaint_pending: false,
            reader: Some(reader_state(searchable == 0)),
            reading_bar: (0.0, 0.0, Instant::now()),
            reading_last: None,
            reading_leaving: None,
            marked: HashMap::new(),
            unmarking: HashMap::new(),
            anchor: None,
            confirming: false,
            undo: Vec::new(),
            bar_shown: None,
            bars: 0,
            bar_last: None,
            bar_leaving: None,
            fading: HashMap::new(),
            leaves: 0,
        };
        // No config means onboarding was never finished.
        if !set_up {
            this.start_onboarding(cx);
        }
        this.attach(window, cx);
        this.search(cx);
        this
    }

    /// Everything that belongs to one particular window. The view outlives
    /// its windows (every summon is a new one), so this runs for each.
    fn attach(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.set_global(Theme::resolve(self.theme_choice, window.appearance()));
        self.appearance = Some(cx.observe_window_appearance(window, |this, window, cx| {
            cx.set_global(Theme::resolve(this.theme_choice, window.appearance()));
            cx.notify();
        }));
        self.tile_bounds.borrow_mut().clear();
        self.last_frame = Instant::now();
        self.visible = true;
        self.software = window.gpu_specs().is_some_and(|g| g.is_software_emulated);
        // Where the launcher window stays on top of everything, clicking away
        // from it has to put it away, like a start menu does.
        self.activation = (self.floating && platform::hides_when_inactive()).then(|| {
            cx.observe_window_activation(window, |_, window, _| {
                if !window.is_window_active() {
                    window.remove_window();
                }
            })
        });
    }

    /// Thumbnails finish decoding one at a time, a page of them within a few
    /// dozen milliseconds. Repainting for each one is 30 frames where two
    /// would do, which costs nothing on a gpu and a lot without one. So
    /// finished images wait up to one repaint interval and go in together.
    fn repaint_soon(&mut self, cx: &mut Context<Self>) {
        if self.repaint_pending {
            return;
        }
        self.repaint_pending = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(REPAINT_BATCH).await;
            let _ = this.update(cx, |this, cx| {
                this.repaint_pending = false;
                cx.notify();
            });
        })
        .detach();
    }

    /// Whether to skip decorative motion: when the system asks for reduced
    /// motion, and when there's no gpu. Drawn in software every animated
    /// frame is real cpu work (a 1180x780 frame is ~27 ms of llvmpipe on a
    /// fast laptop, more on the kind that has no gpu), so a short fade
    /// instead of a flying tile is both smoother and kinder to the battery.
    fn calm(&self, cx: &App) -> bool {
        self.software || cx.reduce_motion()
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        if !self.visible || !matches!(self.page, Page::Search) {
            return;
        }
        // The header shows how far the reader has got, and the empty screen
        // what it's up to, so both keep up with it.
        let reader = Some(reader_state(self.searchable == 0));
        if reader != self.reader {
            let progress = |r: &Option<ReaderState>| r.as_ref().and_then(ReaderState::progress);
            if let Some((done, of)) = progress(&reader) {
                self.reading_bar = (
                    self.reading_bar_now(),
                    done as f32 / of.max(1) as f32,
                    Instant::now(),
                );
            }
            self.reader = reader;
            cx.notify();
        }
        let searchable = self.index.visible_len().unwrap_or(self.searchable);
        if searchable != self.searchable {
            self.searchable = searchable;
            self.input.update(cx, |input, cx| {
                input.placeholder = placeholder(searchable);
                cx.notify();
            });
            self.refresh(cx);
            cx.notify();
        }
    }

    /// Called when the same view is put into a new window, which is what
    /// makes it open exactly where it was left: same query, same selection,
    /// same scroll, even the same shot open. Only if screenshots arrived in
    /// the meantime are the results redone, keeping the selection.
    pub fn reopen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.attach(window, cx);
        let searchable = self.index.visible_len().unwrap_or(self.searchable);
        if searchable != self.searchable {
            self.searchable = searchable;
            self.input.update(cx, |input, cx| {
                input.placeholder = placeholder(searchable);
                cx.notify();
            });
            self.refresh(cx);
        }
        cx.notify();
    }

    /// Hidden, keep about a screen of thumbnails so reopening is instant,
    /// and let the rest go.
    pub fn hidden(&mut self, cx: &mut Context<Self>) {
        self.visible = false;
        // A question left hanging would greet the next summon with a
        // trash prompt nobody remembers asking for.
        self.confirming = false;
        // Listening for a new shortcut means every binding is lifted. Hidden
        // mid-listen, they'd stay lifted until settings came back.
        self.stop_recording(cx);
        self.stats.report();
        self.thumbs.shrink_to(THUMBS_KEPT_HIDDEN, cx);
        self.full.shrink_to(1, cx);
    }

    /// Runs the current query again without resetting where you are.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        // Rebuilding the list loses its scroll position, and this runs every
        // time a new screenshot is read. The shot at the top of the view is
        // remembered so the view stays put. At the very top it doesn't: new
        // screenshots arrive there, and that's where they should be seen.
        let top = self.list.logical_scroll_top();
        self.keep_top = (top.item_ix > 0 || top.offset_in_item > px(0.))
            .then(|| {
                let first = self.rows.get(top.item_ix)?.tiles.first()?.item;
                Some((self.hits.get(first)?.id, top.offset_in_item))
            })
            .flatten();
        let selected = self.hits.get(self.selected).map(|h| h.id);
        let open = self
            .detail
            .as_ref()
            .and_then(|d| self.hits.get(d.hit))
            .map(|h| h.id);

        self.load_hits(cx);
        let present: HashSet<i64> = self
            .hits
            .iter()
            .chain(self.stacks.values().flatten())
            .map(|h| h.id)
            .collect();
        self.marked.retain(|id, _| present.contains(id));
        self.anchor = None;
        let at = |id: Option<i64>| id.and_then(|id| self.hits.iter().position(|h| h.id == id));
        let kept = at(selected);
        self.selected = kept.unwrap_or(0);
        // A question about the selected shot can't stand if that shot is gone.
        if kept.is_none() && self.marked.is_empty() {
            self.confirming = false;
        }
        match (at(open), self.detail.as_mut()) {
            (Some(ix), Some(d)) => d.hit = ix,
            _ => self.detail = None,
        }
        self.laid_out_for = 0.0;
    }

    fn load_hits(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).content.to_string();
        let parsed = Query::parse(&query);
        // Only filters, like `date:today`, list everything they let through,
        // the way browsing does.
        let limit = if parsed.terms.is_empty() {
            BROWSE_LIMIT
        } else {
            SEARCH_LIMIT
        };
        let found = self.index.find(&query, limit).unwrap_or_default();
        self.found = found.len();
        self.stack(found);
        self.parsed = parsed;
        self.limit = limit;
        self.matched.clear();
        self.thumb_paths = self
            .hits
            .iter()
            .map(|h| gyotaku_core::thumb_path(&h.path).unwrap_or_default())
            .collect();
        self.query = query;
    }

    /// Lays results out as tiles: one per burst, led by its first result,
    /// or all of them for a burst that's unfolded.
    fn stack(&mut self, found: Vec<Hit>) {
        self.stacks.clear();
        if !self.grouped {
            self.hits = found;
            return;
        }
        let bursts: Vec<i64> = found.iter().map(|h| h.burst).collect();
        let mut found: Vec<Option<Hit>> = found.into_iter().map(Some).collect();
        let mut hits = Vec::with_capacity(found.len());
        for (lead, rest) in gyotaku_core::burst::stack(&bursts) {
            let Some(lead) = found[lead].take() else {
                continue;
            };
            let rest: Vec<Hit> = rest.into_iter().filter_map(|i| found[i].take()).collect();
            let burst = lead.burst;
            hits.push(lead);
            if !rest.is_empty() {
                if self.unfolded.contains(&burst) {
                    hits.extend(rest.iter().cloned());
                }
                self.stacks.insert(burst, rest);
            }
        }
        self.hits = hits;
    }

    /// The shots folded away behind a tile, which it stands for while
    /// they're hidden: marking it or moving it to the trash takes them too.
    fn folded(&self, item: usize) -> &[Hit] {
        self.hits
            .get(item)
            .filter(|h| !self.unfolded.contains(&h.burst))
            .and_then(|h| self.stacks.get(&h.burst))
            .map_or(&[], Vec::as_slice)
    }

    /// A tile's shot and those folded behind it.
    fn with_folded(&self, item: usize) -> Vec<Hit> {
        self.hits
            .get(item)
            .into_iter()
            .chain(self.folded(item))
            .cloned()
            .collect()
    }

    fn search(&mut self, cx: &mut Context<Self>) {
        // Marks belong to the results they were made on. Carrying them into
        // a new search would trash shots that aren't even on screen.
        self.marked.clear();
        self.unmarking.clear();
        self.anchor = None;
        self.confirming = false;
        self.fading.clear();
        self.leaves += 1;
        self.unfolded.clear();
        self.load_hits(cx);
        self.generation += 1;
        self.selected = 0;
        self.laid_out_for = 0.0;
        self.detail = None;
        self.list.scroll_to(ListOffset::default());
        cx.notify();
    }

    /// The decoded image for a path, or None while it decodes in the
    /// background (the view repaints when it lands).
    /// `original` is the screenshot a thumbnail was made from. If the
    /// thumbnail is missing (cleared in settings, say) it gets redrawn from
    /// that. None for full size images.
    fn image(
        &mut self,
        path: &Path,
        original: Option<&Path>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Arc<RenderImage>> {
        let full = original.is_none();
        let store = if full {
            &mut self.full
        } else {
            &mut self.thumbs
        };
        let max_side = match store.get(path, window, cx) {
            Lookup::Ready(image) => return Some(image),
            Lookup::Pending => return None,
            Lookup::Start(max_side) => max_side,
        };
        let path = path.to_owned();
        let original = original.map(Path::to_owned);
        cx.spawn(async move |this, cx| {
            let decoding = path.clone();
            let image = cx
                .background_executor()
                .spawn(async move {
                    if let Some(original) = original.filter(|_| !decoding.exists()) {
                        images::write_thumbnail(&original, &decoding);
                    }
                    images::decode(&decoding, max_side)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let store = if full {
                    &mut this.full
                } else {
                    &mut this.thumbs
                };
                store.finish(&path, image);
                this.repaint_soon(cx);
            });
        })
        .detach();
        None
    }

    fn matched_lines(&mut self, item: usize) -> Rc<Vec<Line>> {
        let Some(hit) = self.hits.get(item) else {
            return Rc::default();
        };
        if !self.searching() {
            return Rc::default();
        }
        let (index, query) = (&self.index, &self.query);
        self.matched
            .entry(hit.id)
            .or_insert_with(|| {
                Rc::new(
                    index
                        .matching_lines(hit.id, query, hit.near)
                        .unwrap_or_default(),
                )
            })
            .clone()
    }

    /// Whether the panel rounds and outlines itself: only a floating panel
    /// nothing else frames. Where the system frames the window, a corner of
    /// our own would sit inside its curve and leave a sliver between them.
    fn own_frame(&self) -> bool {
        self.floating && !platform::SYSTEM_FRAMES_WINDOW
    }

    fn corner(&self) -> Pixels {
        if self.own_frame() {
            px(PANEL_RADIUS)
        } else {
            px(0.)
        }
    }

    fn searching(&self) -> bool {
        !self.query.trim().is_empty()
    }

    fn relayout(&mut self, width: f32) {
        let aspects: Vec<f32> = self
            .hits
            .iter()
            .map(|h| gyotaku_core::tile_aspect(h.width, h.height))
            .collect();
        self.rows = grid::justify(&aspects, width, ROW_HEIGHT, GAP);
        self.located = grid::locate(&self.rows, self.hits.len());
        self.laid_out_for = width;
        self.list.reset(self.rows.len());
        if let Some((id, offset)) = self.keep_top.take()
            && let Some(item) = self.hits.iter().position(|h| h.id == id)
            && let Some(&(row, _)) = self.located.get(item)
        {
            self.list.scroll_to(ListOffset {
                item_ix: row,
                offset_in_item: offset,
            });
        }
        if std::mem::take(&mut self.reveal_selected)
            && let Some(&(row, _)) = self.located.get(self.selected)
        {
            self.list.scroll_to_reveal_item(row);
        }
    }

    fn select(&mut self, item: Option<usize>, cx: &mut Context<Self>) {
        self.anchor = None;
        self.move_to(item, cx);
    }

    fn move_to(&mut self, item: Option<usize>, cx: &mut Context<Self>) {
        let Some(item) = item.filter(|i| *i < self.hits.len()) else {
            return;
        };
        self.confirming = false;
        self.selected = item;
        if let Some(&(row, _)) = self.located.get(item) {
            self.list.scroll_to_reveal_item(row);
        }
        if self.detail.is_some() {
            self.show(item, cx);
        }
        cx.notify();
    }

    // Marking shots for the trash. Ctrl click marks one, shift click or
    // shift and the arrows mark a run, ctrl shift a marks everything found.

    fn set_mark(&mut self, id: i64, on: bool) {
        let now = Instant::now();
        if on {
            if let std::collections::hash_map::Entry::Vacant(e) = self.marked.entry(id) {
                e.insert(now);
                self.unmarking.remove(&id);
            }
        } else if self.marked.remove(&id).is_some() {
            self.unmarking.insert(id, now);
        }
    }

    fn clear_marks(&mut self) {
        for id in self.marked.keys().copied().collect::<Vec<_>>() {
            self.set_mark(id, false);
        }
        self.anchor = None;
        self.confirming = false;
    }

    fn toggle_mark(&mut self, item: usize, cx: &mut Context<Self>) {
        let Some(id) = self.hits.get(item).map(|h| h.id) else {
            return;
        };
        let on = !self.marked.contains_key(&id);
        for hit in self.with_folded(item) {
            self.set_mark(hit.id, on);
        }
        self.select(Some(item), cx);
    }

    /// Marks the run from the anchor to `to`, on top of whatever was marked
    /// before the run started.
    fn extend_to(&mut self, to: Option<usize>, cx: &mut Context<Self>) {
        let Some(to) = to.filter(|i| *i < self.hits.len()) else {
            return;
        };
        let (anchor, before) = match self.anchor.take() {
            Some(a) => a,
            None => (self.selected, self.marked.keys().copied().collect()),
        };
        let (lo, hi) = (anchor.min(to), anchor.max(to));
        let run: HashSet<i64> = (lo..=hi)
            .flat_map(|i| self.with_folded(i))
            .map(|h| h.id)
            .collect();
        let stale: Vec<i64> = self
            .marked
            .keys()
            .filter(|id| !run.contains(id) && !before.contains(id))
            .copied()
            .collect();
        for id in stale {
            self.set_mark(id, false);
        }
        for id in run {
            self.set_mark(id, true);
        }
        self.anchor = Some((anchor, before));
        self.move_to(Some(to), cx);
    }

    fn mark_up(&mut self, _: &MarkUp, _: &mut Window, cx: &mut Context<Self>) {
        if self.on_panel() || self.detail.is_some() {
            return;
        }
        let to = self
            .located
            .get(self.selected)
            .and_then(|&at| grid::vertical(&self.rows, at, false));
        self.extend_to(to, cx);
    }

    fn mark_down(&mut self, _: &MarkDown, _: &mut Window, cx: &mut Context<Self>) {
        if self.on_panel() || self.detail.is_some() {
            return;
        }
        let to = self
            .located
            .get(self.selected)
            .and_then(|&at| grid::vertical(&self.rows, at, true));
        self.extend_to(to, cx);
    }

    fn mark_left(&mut self, _: &MarkLeft, _: &mut Window, cx: &mut Context<Self>) {
        if !self.on_panel() && self.detail.is_none() {
            self.extend_to(self.selected.checked_sub(1), cx);
        }
    }

    fn mark_right(&mut self, _: &MarkRight, _: &mut Window, cx: &mut Context<Self>) {
        if !self.on_panel() && self.detail.is_none() {
            self.extend_to(Some(self.selected + 1), cx);
        }
    }

    fn mark_all(&mut self, _: &MarkAll, _: &mut Window, cx: &mut Context<Self>) {
        if self.on_panel() || self.detail.is_some() {
            return;
        }
        // With no query that would be the whole library, two keys away from
        // the trash. Narrowing it down first is the point anyway.
        if !self.searching() {
            self.flash("search first, then mark everything it finds", cx);
            return;
        }
        let all: Vec<i64> = (0..self.hits.len())
            .flat_map(|i| self.with_folded(i))
            .map(|h| h.id)
            .collect();
        for id in all {
            self.set_mark(id, true);
        }
        self.anchor = None;
        cx.notify();
    }

    /// What ctrl delete acts on: the open shot, else the marked ones, else
    /// the selected one with any folded behind it. In result order, so the
    /// toast and undo match the grid.
    fn trash_targets(&self) -> Vec<Hit> {
        if let Some(d) = self.detail.as_ref().filter(|d| d.open.target() == 1.0) {
            return self.hits.get(d.hit).cloned().into_iter().collect();
        }
        if self.marked.is_empty() {
            return self.with_folded(self.selected);
        }
        (0..self.hits.len())
            .flat_map(|i| self.with_folded(i))
            .filter(|h| self.marked.contains_key(&h.id))
            .collect()
    }

    /// Ctrl delete asks first, a second ctrl delete or enter goes ahead.
    fn trash(&mut self, _: &Trash, _: &mut Window, cx: &mut Context<Self>) {
        if self.on_panel() || self.trash_targets().is_empty() {
            return;
        }
        if self.confirming {
            self.trash_now(cx);
        } else {
            self.confirming = true;
            cx.notify();
        }
    }

    fn trash_now(&mut self, cx: &mut Context<Self>) {
        self.confirming = false;
        let targets = self.trash_targets();
        if targets.is_empty() {
            return;
        }
        // Where the selection lands once they're gone: the first of them
        // that has a tile.
        let first = targets
            .iter()
            .find_map(|t| self.hits.iter().position(|h| h.id == t.id))
            .unwrap_or(self.selected);
        let mut batch = Vec::new();
        let mut failed = 0;
        for hit in &targets {
            let lines = self.index.lines(hit.id).unwrap_or_default();
            match trash::trash(&hit.path) {
                Ok(trashed) => {
                    // The watcher would notice the file leaving too, but it
                    // may not be running, and the grid shouldn't wait for it.
                    let _ = self.index.remove(&hit.path);
                    batch.push(Binned {
                        trashed,
                        shot: Shot {
                            path: hit.path.clone(),
                            mtime: hit.mtime,
                            width: hit.width,
                            height: hit.height,
                            look: hit.look,
                        },
                        lines,
                    });
                }
                Err(e) => {
                    eprintln!("{e:#}");
                    failed += 1;
                }
            }
        }

        let moved = batch.len();
        // The files are already gone at this point. What's left is only the
        // grid letting go of them: they shrink and fade where they sit, and
        // only then does the grid close up around the gap.
        let now = Instant::now();
        for b in &batch {
            if let Some(hit) = self.hits.iter().find(|h| h.path == b.shot.path) {
                let marked = self.marked.contains_key(&hit.id);
                self.fading.insert(
                    hit.id,
                    Fade {
                        arriving: false,
                        since: now,
                        marked,
                    },
                );
            }
        }
        // Only what went to the trash loses its mark: trashing the one open
        // shot leaves the marks on the others for later.
        let gone: Vec<i64> = targets.iter().map(|h| h.id).collect();
        for id in &gone {
            self.marked.remove(id);
            self.unmarking.remove(id);
        }
        if moved > 0 {
            self.undo = batch;
        }
        self.anchor = None;
        if let Some(d) = self.detail.as_mut() {
            // The tile it would fly back to is leaving, so it fades instead.
            d.grow = false;
            d.open.set_response(FADE_RESPONSE, 1.0);
            d.open.set_target(0.0);
            self.last_frame = Instant::now();
        }
        self.leaves += 1;
        let token = self.leaves;
        let settle = move |this: &mut Self, cx: &mut Context<Self>| {
            if this.leaves != token {
                return;
            }
            this.fading.retain(|_, f| f.arriving);
            this.after_change(cx);
            this.selected = first.min(this.hits.len().saturating_sub(1));
            this.reveal_selected = true;
        };
        if self.calm(cx) || moved == 0 {
            settle(self, cx);
        } else {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(TILE_FADE).await;
                let _ = this.update(cx, |this, cx| settle(this, cx));
            })
            .detach();
        }
        cx.notify();

        let message = match (moved, failed) {
            (0, _) => "couldn't move it to the trash".to_string(),
            (1, 0) => "moved to the trash".to_string(),
            (n, 0) => format!("moved {} to the trash", thousands(n)),
            (n, f) => format!("moved {}, {} couldn't be moved", thousands(n), thousands(f)),
        };
        if moved > 0 {
            let undo = keys::shown("undo", cx);
            self.flash_with(message, Some((undo, "undo")), TOAST_UNDO, cx);
        } else {
            self.flash(message, cx);
        }
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if self.on_panel() || self.undo.is_empty() {
            return;
        }
        // An undo straight after the trash, before its tiles finished
        // leaving, takes over from that settle.
        self.leaves += 1;
        self.fading.clear();
        // A question asked before the undo was about a different set of
        // marks; enter mustn't answer it with these.
        self.confirming = false;
        let batch = std::mem::take(&mut self.undo);
        let trashed: Vec<Trashed> = batch.iter().map(|b| b.trashed.clone()).collect();
        let mut back = Vec::new();
        let mut failed = 0;
        for (b, restored) in batch.iter().zip(trash::restore_all(&trashed)) {
            match restored {
                Ok(()) => {
                    if let Ok(id) = self.index.insert(&b.shot, &b.lines) {
                        back.push(id);
                    }
                }
                Err(e) => {
                    eprintln!("{e:#}");
                    failed += 1;
                }
            }
        }
        self.after_change(cx);
        // What came back grows back in, and stays marked so it's plain which
        // ones they were. Only what this search shows, though: a mark on a
        // shot that isn't on screen can't be seen or undone.
        let now = Instant::now();
        let shown: Vec<i64> = back
            .iter()
            .copied()
            .filter(|id| self.hits.iter().any(|h| h.id == *id))
            .collect();
        for &id in &shown {
            self.set_mark(id, true);
            self.fading.insert(
                id,
                Fade {
                    arriving: true,
                    since: now,
                    marked: true,
                },
            );
        }
        // With a shot open, the selection stays on it: open, copy and reveal
        // act on the selection, and must act on what's on screen.
        if let Some(d) = &self.detail {
            self.selected = d.hit;
        } else if let Some(i) = self.hits.iter().position(|h| shown.contains(&h.id)) {
            self.selected = i;
            self.reveal_selected = true;
        }
        let message = match (back.len(), failed) {
            (0, _) => "couldn't put them back".to_string(),
            (1, 0) => "put back".to_string(),
            (n, 0) => format!("put back {}", thousands(n)),
            (n, f) => format!("put back {}, {} couldn't be", thousands(n), thousands(f)),
        };
        self.flash(message, cx);
    }

    fn similar(&mut self, _: &Similar, _: &mut Window, cx: &mut Context<Self>) {
        if self.on_panel() || self.detail.is_some() {
            return;
        }
        self.unfold(self.selected, cx);
    }

    /// Shows every shot in a tile's burst, right after its first, or folds
    /// them back behind it. The selection stays on the first either way.
    fn unfold(&mut self, item: usize, cx: &mut Context<Self>) {
        let Some(burst) = self.hits.get(item).map(|h| h.burst) else {
            return;
        };
        if !self.stacks.contains_key(&burst) {
            self.flash("nothing similar to this one", cx);
            return;
        }
        let lead = self.hits.iter().find(|h| h.burst == burst).map(|h| h.id);
        // A question asked about the tiles before is about different ones now.
        self.confirming = false;
        if !self.unfolded.remove(&burst) {
            self.unfolded.insert(burst);
        }
        self.refresh(cx);
        if let Some(i) = lead.and_then(|id| self.hits.iter().position(|h| h.id == id)) {
            self.selected = i;
            self.reveal_selected = true;
        }
        cx.notify();
    }

    /// After shots leave or come back: recount, rerun the query, relayout.
    fn after_change(&mut self, cx: &mut Context<Self>) {
        self.searchable = self.index.visible_len().unwrap_or(self.searchable);
        let searchable = self.searchable;
        self.input.update(cx, |input, cx| {
            input.placeholder = placeholder(searchable);
            cx.notify();
        });
        self.refresh(cx);
        cx.notify();
    }

    // Actions. They're bound at the root, so they arrive whether the search
    // field or anything else has focus.

    /// Settings and onboarding take over the arrows, enter and escape.
    fn on_panel(&self) -> bool {
        !matches!(self.page, Page::Search)
    }

    fn back(&mut self, _: &Back, window: &mut Window, cx: &mut Context<Self>) {
        if self.on_panel() {
            return self.panel_back(window, cx);
        }
        if self.confirming {
            self.confirming = false;
            cx.notify();
            return;
        }
        // A shot already on its way closed doesn't eat a second escape, that
        // one goes to the next step. Pressing escape twice quickly should do
        // two things, not one, whatever speed the machine animates at.
        let closing = self.detail.as_ref().is_some_and(|d| d.open.target() == 0.0);
        let tile = self
            .detail
            .as_ref()
            .and_then(|d| self.tile_bounds.borrow().get(&d.hit).copied());
        if let Some(d) = self.detail.as_mut().filter(|_| !closing) {
            if d.grow {
                d.open.set_response(CLOSE_RESPONSE, 1.0);
            }
            // Back into the tile of the shot on screen now, which after
            // stepping through with the arrows isn't the one it opened from.
            if let Some(tile) = tile {
                d.from = tile;
            }
            d.open.set_target(0.0);
            self.last_frame = Instant::now();
        } else if !self.marked.is_empty() {
            self.clear_marks();
        } else if self.searching() {
            self.input.update(cx, |input, cx| input.clear(cx));
        } else {
            // Resident, this just hides. With --once it's the last window
            // and the app exits with it.
            window.remove_window();
        }
        cx.notify();
    }

    /// Enter opens the full view, and from there, the file itself.
    fn open(&mut self, _: &Open, window: &mut Window, cx: &mut Context<Self>) {
        if self.on_panel() {
            return self.panel_key(Key::Enter, window, cx);
        }
        if self.confirming {
            return self.trash_now(cx);
        }
        if self.detail.as_ref().is_some_and(|d| d.open.target() == 1.0) {
            self.open_selected(cx);
        } else {
            self.show(self.selected, cx);
        }
    }

    fn toggle(&mut self, _: &Toggle, window: &mut Window, cx: &mut Context<Self>) {
        self.panel_key(Key::Space, window, cx);
    }

    fn remove(&mut self, _: &Remove, window: &mut Window, cx: &mut Context<Self>) {
        self.panel_key(Key::Remove, window, cx);
    }

    /// Its own key in onboarding and settings, so enter is always free to
    /// mean "continue" there, wherever the mouse happens to rest.
    fn add_folder(&mut self, _: &AddFolder, window: &mut Window, cx: &mut Context<Self>) {
        if self.on_panel() {
            self.add_folders(window, cx);
        }
    }

    fn open_settings_action(
        &mut self,
        _: &OpenSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.page, Page::Search) {
            self.open_settings(window, cx);
        }
    }

    fn up(&mut self, _: &Up, window: &mut Window, cx: &mut Context<Self>) {
        if self.on_panel() {
            return self.panel_key(Key::Up, window, cx);
        }
        if self.detail.is_none() {
            let to = self
                .located
                .get(self.selected)
                .and_then(|&at| grid::vertical(&self.rows, at, false));
            self.select(to, cx);
        }
    }

    fn down(&mut self, _: &Down, window: &mut Window, cx: &mut Context<Self>) {
        if self.on_panel() {
            return self.panel_key(Key::Down, window, cx);
        }
        if self.detail.is_none() {
            let to = self
                .located
                .get(self.selected)
                .and_then(|&at| grid::vertical(&self.rows, at, true));
            self.select(to, cx);
        }
    }

    fn left(&mut self, _: &Left, window: &mut Window, cx: &mut Context<Self>) {
        if self.on_panel() {
            return self.panel_key(Key::Left, window, cx);
        }
        self.select(self.selected.checked_sub(1), cx);
    }

    fn right(&mut self, _: &Right, window: &mut Window, cx: &mut Context<Self>) {
        if self.on_panel() {
            return self.panel_key(Key::Right, window, cx);
        }
        self.select(Some(self.selected + 1), cx);
    }

    fn jump(&mut self, down: bool, cx: &mut Context<Self>) {
        let mut at = self.selected;
        for _ in 0..4 {
            match self
                .located
                .get(at)
                .and_then(|&loc| grid::vertical(&self.rows, loc, down))
            {
                Some(next) => at = next,
                None => break,
            }
        }
        self.select(Some(at), cx);
    }

    fn page_up(&mut self, _: &PageUp, _: &mut Window, cx: &mut Context<Self>) {
        if !self.on_panel() {
            self.jump(false, cx);
        }
    }

    fn page_down(&mut self, _: &PageDown, _: &mut Window, cx: &mut Context<Self>) {
        if !self.on_panel() {
            self.jump(true, cx);
        }
    }

    fn copy_text(&mut self, _: &CopyText, _: &mut Window, cx: &mut Context<Self>) {
        if self.on_panel() {
            return;
        }
        let lines = match &self.detail {
            Some(d) if !d.picked.is_empty() => {
                d.picked.iter().map(|&i| d.lines[i].clone()).collect()
            }
            Some(d) => d.lines.clone(),
            None => match self.hits.get(self.selected) {
                Some(hit) => self.index.lines(hit.id).unwrap_or_default(),
                None => return,
            },
        };
        if lines.is_empty() {
            self.flash("no text in this one", cx);
            return;
        }
        let text = lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        platform::copy_text(&text, cx);
        let what = if lines.len() == 1 {
            "copied 1 line".into()
        } else {
            format!("copied {} lines", lines.len())
        };
        self.flash(what, cx);
    }

    fn copy_image(&mut self, _: &CopyImage, _: &mut Window, cx: &mut Context<Self>) {
        let Some(hit) = self.hits.get(self.selected).filter(|_| !self.on_panel()) else {
            return;
        };
        let path = hit.path.clone();
        // Already a file that's read, so the reader shouldn't save it again
        // when it sees it on the clipboard.
        gyotaku_core::clipboard::mark_own_copy(&path);
        let message = if platform::copy_image(&path, cx) {
            "copied the image"
        } else {
            platform::WORDS.copy_image_failed
        };
        self.flash(message, cx);
    }

    fn open_external(&mut self, _: &OpenExternal, _: &mut Window, cx: &mut Context<Self>) {
        self.open_selected(cx);
    }

    fn open_selected(&self, cx: &mut Context<Self>) {
        if let Some(hit) = self.hits.get(self.selected) {
            cx.open_with_system(&hit.path);
        }
    }

    fn reveal(&mut self, _: &Reveal, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(hit) = self.hits.get(self.selected) {
            cx.reveal_path(&hit.path);
        }
    }

    fn quit(&mut self, _: &Quit, _: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }

    /// Opens the full view of a shot, or switches to it if one is already open.
    fn show(&mut self, item: usize, cx: &mut Context<Self>) {
        let Some(id) = self.hits.get(item).map(|h| h.id) else {
            return;
        };
        let hits_here = self.matched_lines(item);
        let lines = self.index.lines(id).unwrap_or_default();
        let matched = lines.iter().map(|l| hits_here.contains(l)).collect();
        self.selected = item;

        if let Some(d) = &mut self.detail {
            d.hit = item;
            d.zoom = crate::zoom::Zoom::new();
            d.lines = lines;
            d.matched = matched;
            d.hovered = None;
            d.picked.clear();
            if d.grow {
                d.open.set_response(OPEN_RESPONSE, 1.0);
            }
            d.open.set_target(1.0);
        } else {
            let tile = self.tile_bounds.borrow().get(&item).copied();
            let from = tile.unwrap_or_else(|| {
                Bounds::centered_at(point(px(400.), px(300.)), size(px(80.), px(60.)))
            });
            // With reduced motion the shot doesn't fly out of its tile, it
            // fades in where it's going to be.
            let grow = !self.calm(cx);
            let mut open = Spring::new(0.0, if grow { OPEN_RESPONSE } else { FADE_RESPONSE }, 1.0);
            open.set_target(1.0);
            self.detail = Some(Detail {
                hit: item,
                lines,
                matched,
                open,
                grow,
                from,
                image_rect: from,
                preview_area: from,
                zoom: crate::zoom::Zoom::new(),
                hovered: None,
                drag: None,
                picked: Vec::new(),
            });
        }
        self.last_frame = Instant::now();
        cx.notify();
    }

    fn flash(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.flash_with(message, None, TOAST_SHOWN, cx);
    }

    fn flash_with(
        &mut self,
        message: impl Into<SharedString>,
        keys: Option<(SharedString, &'static str)>,
        shown: Duration,
        cx: &mut Context<Self>,
    ) {
        self.toasts += 1;
        let id = self.toasts;
        self.toast = Some(Toast {
            message: message.into(),
            keys,
            id,
            leaving: false,
        });
        cx.spawn(async move |this, cx| {
            // Shown, then faded out, then gone. A newer toast takes over and
            // this one's timers do nothing.
            for leaving in [true, false] {
                let wait = if leaving { shown } else { TOAST_FADE };
                cx.background_executor().timer(wait).await;
                let _ = this.update(cx, |this, cx| {
                    let Some(toast) = this.toast.as_mut().filter(|t| t.id == id) else {
                        return;
                    };
                    if leaving {
                        toast.leaving = true;
                    } else {
                        this.toast = None;
                    }
                    cx.notify();
                });
            }
        })
        .detach();
        cx.notify();
    }

    // The full view's mouse handling. Hovering shows which line is under the
    // pointer, clicking copies it, dragging copies every line the box touches.

    fn line_at(&self, p: Point<Pixels>) -> Option<usize> {
        let d = self.detail.as_ref()?;
        if !d.preview_area.contains(&p) {
            return None;
        }
        d.lines
            .iter()
            .position(|l| on_screen(l.rect, d.image_rect).contains(&p))
    }

    fn detail_mouse_move(&mut self, e: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let hovered = self.line_at(e.position);
        let Some(d) = &mut self.detail else { return };
        if let Some((start, _)) = d.drag {
            d.drag = Some((start, e.position));
            let area = Bounds::from_corners(start.min(&e.position), start.max(&e.position));
            d.picked = (0..d.lines.len())
                .filter(|&i| on_screen(d.lines[i].rect, d.image_rect).intersects(&area))
                .collect();
            cx.notify();
        } else if d.hovered != hovered {
            d.hovered = hovered;
            cx.notify();
        }
    }

    fn detail_mouse_down(&mut self, e: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(d) = &mut self.detail {
            if !d.preview_area.contains(&e.position) {
                return;
            }
            d.drag = Some((e.position, e.position));
            d.picked.clear();
            cx.notify();
        }
    }

    fn detail_mouse_up(&mut self, e: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        let clicked = self.line_at(e.position);
        let Some(d) = &mut self.detail else { return };
        let Some((start, _)) = d.drag.take() else {
            return;
        };
        let dragged =
            (e.position.x - start.x).abs() > px(4.) || (e.position.y - start.y).abs() > px(4.);
        if !dragged {
            d.picked = clicked.into_iter().collect();
        }
        if d.picked.is_empty() {
            cx.notify();
            return;
        }
        self.copy_text(&CopyText, window, cx);
    }

    fn render_row(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(row) = self.rows.get(ix) else {
            return div().into_any_element();
        };
        let height = row.height;
        let tiles: Vec<AnyElement> = row
            .tiles
            .clone()
            .into_iter()
            .map(|t| self.render_tile(t.item, t.width, height, window, cx))
            .collect();
        div()
            .flex()
            .gap(px(GAP))
            .px(px(PAD))
            .py(px(GAP / 2.0))
            .children(tiles)
            .into_any_element()
    }

    fn render_tile(
        &mut self,
        i: usize,
        w: f32,
        h: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Rows are laid out ahead of the results changing under them now and
        // then (a refresh between layout and paint); a row naming a shot
        // that's no longer there draws an empty slot for that frame.
        let (Some(path), Some(original)) = (
            self.thumb_paths.get(i).cloned(),
            self.hits.get(i).map(|h| h.path.clone()),
        ) else {
            return div().w(px(w)).h(px(h)).into_any_element();
        };
        let theme = *cx.global::<Theme>();
        let thumb = self.image(&path, Some(&original), window, cx);
        let lines = self.matched_lines(i);
        let hit = &self.hits[i];
        let (id, near) = (hit.id, hit.near);
        // How many similar shots this tile leads, and whether they're
        // folded behind it. Only the first of a burst says so.
        let similar = self
            .stacks
            .get(&hit.burst)
            .filter(|rest| rest.iter().all(|h| h.id != id))
            .map(|rest| (rest.len(), !self.unfolded.contains(&hit.burst)));
        let crop = gyotaku_core::tile_crop(hit.width, hit.height);
        let sink = self.tile_bounds.clone();
        let calm = self.calm(cx);

        let (mut m, moving) = self.mark_progress(id, calm);
        if moving {
            window.request_animation_frame();
        } else if !self.marked.contains_key(&id) {
            self.unmarking.remove(&id);
        }
        // 1 is fully here, 0 is gone to the trash (or not back from it yet).
        let (presence, fading) = self.presence(id, calm);
        if fading {
            window.request_animation_frame();
        } else if self.fading.get(&id).is_some_and(|f| f.arriving) {
            self.fading.remove(&id);
        }
        if self
            .fading
            .get(&id)
            .is_some_and(|f| !f.arriving && f.marked)
        {
            m = 1.0;
        }
        // The picture and everything drawn on it live in an inner frame that
        // shrinks when marked, so the highlight boxes shrink right along.
        let inset = MARK_INSET * m + (1.0 - presence) * w.min(h) * 0.16;
        let (iw, ih) = (w - inset * 2., h - inset * 2.);
        let radius = px(RADIUS - (RADIUS - 6.) * m);

        let mut inner = div()
            .absolute()
            .left(px(inset))
            .top(px(inset))
            .w(px(iw))
            .h(px(ih))
            .children(
                thumb
                    .clone()
                    .map(|t| img(t).absolute().size_full().rounded(radius)),
            );

        // The ink press: the whole print darkens and only the words that
        // matched stay lit, each one a window cut through the veil back to
        // the thumbnail underneath.
        if let Some(thumb) = thumb.filter(|_| !lines.is_empty()) {
            inner = inner.child(div().absolute().size_full().rounded(radius).bg(theme.veil));
            for (j, line) in lines.iter().enumerate() {
                let Some(b) = in_tile(line.rect, crop, iw, ih) else {
                    continue;
                };
                let lit = div()
                    .absolute()
                    .left(px(b.x))
                    .top(px(b.y))
                    .w(px(b.w))
                    .h(px(b.h))
                    .overflow_hidden()
                    .rounded(px(3.))
                    .border_1()
                    .border_color(theme.accent)
                    .child(
                        img(thumb.clone())
                            .absolute()
                            .left(px(-b.x - 1.))
                            .top(px(-b.y - 1.))
                            .w(px(iw))
                            .h(px(ih)),
                    );
                // This replays on every keystroke, so it stays short: typing
                // is the most frequent thing anyone does here.
                if calm {
                    inner = inner.child(lit);
                } else {
                    let id = ElementId::Name(format!("press-{}-{i}-{j}", self.generation).into());
                    inner = inner.child(lit.with_animation(
                        id,
                        Animation::new(PRESS).with_easing(ease_out_quint()),
                        |el, t| el.opacity(t),
                    ));
                }
            }
        }

        inner = inner.child(
            div()
                .absolute()
                .size_full()
                .rounded(radius)
                .border_1()
                .border_color(theme.image_edge),
        );

        // Found only with look-alike characters folded together, so the
        // word in the picture isn't quite what was typed. Said plainly, so
        // `E0425` turning up for `EO425` is never mistaken for an exact hit.
        // 5 px in with a 5 px radius, concentric with the tile's 10.
        if near && self.searching() {
            inner = inner.child(
                div()
                    .absolute()
                    .left(px(5.))
                    .bottom(px(5.))
                    .px(px(6.))
                    .py(px(1.))
                    .rounded(px(RADIUS - 5.))
                    .bg(theme.panel)
                    .border_1()
                    .border_color(theme.hairline)
                    .text_xs()
                    .text_color(theme.muted)
                    .whitespace_nowrap()
                    .child("near match"),
            );
        }

        // Near-identical shots stacked behind this one. The same chip as
        // near match, in the other corner, and it's the way in with a mouse:
        // a click unfolds them (or folds them back) without opening the shot.
        if let Some((n, folded)) = similar {
            let label = if folded {
                format!("+{n} similar")
            } else {
                format!("hide {n}")
            };
            inner = inner.child(
                div()
                    .id(("similar", i))
                    .absolute()
                    .right(px(5.))
                    .bottom(px(5.))
                    .px(px(6.))
                    .py(px(1.))
                    .rounded(px(RADIUS - 5.))
                    .bg(theme.panel)
                    .border_1()
                    .border_color(theme.hairline)
                    .text_xs()
                    .text_color(theme.muted)
                    .whitespace_nowrap()
                    .cursor(CursorStyle::PointingHand)
                    .hover(move |s| s.text_color(theme.text))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        this.unfold(i, cx);
                    }))
                    .child(label),
            );
        }

        let mut tile = div()
            .id(("tile", i))
            .group(SharedString::from(format!("tile-{i}")))
            .relative()
            .flex_none()
            .w(px(w))
            .h(px(h))
            .rounded(px(RADIUS))
            .bg(theme.tile)
            .cursor(CursorStyle::PointingHand)
            .opacity(presence)
            .child(inner);

        if m > 0.0 {
            // An ink seal with a check, ringed in the panel colour so it
            // reads on a white page and a black terminal alike.
            tile = tile.child(
                div()
                    .absolute()
                    .top(px(3.))
                    .left(px(3.))
                    .size(px(20.))
                    .rounded_full()
                    .bg(theme.text)
                    .border_2()
                    .border_color(theme.panel)
                    .opacity(m)
                    .child(check(theme.panel)),
            );
        }

        if i == self.selected && presence == 1.0 {
            // Offset outward by 3 px, so the radius grows by 3 too, the ring
            // stays concentric with the tile's corners.
            tile = tile.child(
                div()
                    .absolute()
                    .top(px(-3.))
                    .left(px(-3.))
                    .w(px(w + 6.))
                    .h(px(h + 6.))
                    .rounded(px(RADIUS + 3.))
                    .border_2()
                    .border_color(theme.text),
            );
        } else if presence == 1.0 {
            // The same ring, faint, on the tile under the mouse: it says what
            // a click would open without taking the selection from the keys.
            tile = tile.child(
                div()
                    .absolute()
                    .top(px(-3.))
                    .left(px(-3.))
                    .w(px(w + 6.))
                    .h(px(h + 6.))
                    .rounded(px(RADIUS + 3.))
                    .border_2()
                    .border_color(theme.faint)
                    .opacity(0.)
                    .group_hover(format!("tile-{i}"), |s| s.opacity(1.)),
            );
        }

        tile.child(
            canvas(
                move |bounds, _, _| sink.borrow_mut().insert(i, bounds),
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        )
        .on_click(cx.listener(move |this, e: &ClickEvent, _, cx| {
            let mods = e.modifiers();
            if mods.secondary() {
                this.toggle_mark(i, cx);
            } else if mods.shift {
                this.extend_to(Some(i), cx);
            } else {
                this.confirming = false;
                this.anchor = None;
                this.selected = i;
                this.show(i, cx);
            }
        }))
        .into_any_element()
    }

    /// How present a tile is, 1 normally, heading to 0 on its way to the
    /// trash or up from 0 on its way back, and whether that's still moving.
    fn presence(&self, id: i64, calm: bool) -> (f32, bool) {
        let Some(f) = self.fading.get(&id) else {
            return (1.0, false);
        };
        let t = if calm {
            1.0
        } else {
            (f.since.elapsed().as_secs_f32() / TILE_FADE.as_secs_f32()).min(1.0)
        };
        let eased = ease_out_quint()(t);
        (if f.arriving { eased } else { 1.0 - eased }, t < 1.0)
    }

    /// How far into its marked look a tile is, 0 to 1, and whether that's
    /// still moving.
    fn mark_progress(&self, id: i64, calm: bool) -> (f32, bool) {
        let (on, since) = match (self.marked.get(&id), self.unmarking.get(&id)) {
            (Some(t), _) => (true, *t),
            (None, Some(t)) => (false, *t),
            (None, None) => return (0.0, false),
        };
        let t = if calm {
            1.0
        } else {
            (since.elapsed().as_secs_f32() / MARK.as_secs_f32()).min(1.0)
        };
        let eased = ease_out_quint()(t);
        (if on { eased } else { 1.0 - eased }, t < 1.0)
    }

    /// Where the header's progress bar is drawn right now, partway through
    /// gliding to the latest reading.
    fn reading_bar_now(&self) -> f32 {
        let (from, to, since) = self.reading_bar;
        let t = (since.elapsed().as_secs_f32() / READING_GLIDE.as_secs_f32()).min(1.0);
        from + (to - from) * ease_out_quint()(t)
    }

    /// While the reader works through screenshots: how far it's got, and a
    /// thin bar that fills as it goes. It fades in when reading starts and
    /// out when it's done, and only ever moves because reading moved on.
    fn render_reading(
        &mut self,
        theme: Theme,
        window: &mut Window,
        cx: &App,
    ) -> Option<AnyElement> {
        let calm = self.calm(cx);
        let now = self.reader.as_ref().filter(|r| r.running).and_then(|r| {
            if let Some((done, of)) = r.progress() {
                let label = format!("reading {} of {}", thousands(done), thousands(of));
                Some((label, Some(self.reading_bar_now())))
            } else {
                r.getting_ready()
                    .then(|| ("getting ready".to_string(), None))
            }
        });
        let leaving = match now {
            Some(shown) => {
                self.reading_last = Some(shown);
                self.reading_leaving = None;
                None
            }
            None => {
                self.reading_last.as_ref()?;
                let since = *self.reading_leaving.get_or_insert_with(Instant::now);
                let t = since.elapsed().as_secs_f32() / BAR_OUT.as_secs_f32();
                if calm || t >= 1.0 {
                    self.reading_last = None;
                    self.reading_leaving = None;
                    return None;
                }
                Some(ease_out_quint()(t))
            }
        };
        let (label, fill) = self.reading_last.clone()?;
        if fill.is_some_and(|f| (f - self.reading_bar.1).abs() > 0.001) || leaving.is_some() {
            window.request_animation_frame();
        }

        let width = 64.0;
        let row = div()
            .flex()
            .items_center()
            .gap_2()
            .text_xs()
            .text_color(theme.muted)
            .child(label)
            .children(fill.map(|f| {
                div()
                    .w(px(width))
                    .h(px(3.))
                    .rounded_full()
                    .bg(theme.track)
                    .child(
                        div()
                            .h_full()
                            .w(px(width * f.clamp(0.0, 1.0)))
                            .rounded_full()
                            .bg(theme.muted),
                    )
            }));
        Some(match leaving {
            Some(t) => row.opacity(1.0 - t).into_any_element(),
            None if calm => row.into_any_element(),
            None => row
                .with_animation(
                    "reading",
                    Animation::new(BAR_IN).with_easing(ease_out_quint()),
                    |el, t| el.opacity(t),
                )
                .into_any_element(),
        })
    }

    fn render_header(
        &mut self,
        theme: Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let reading = self.render_reading(theme, window, cx);
        // While searching, how many matched. While browsing, when the selected
        // one was taken, which is the one thing the grid itself can't show.
        let count = if self.searching() {
            // Every shot found, those folded behind a tile included.
            let n = self.found;
            match n {
                0 => String::new(),
                n if n >= self.limit => format!("{}+", thousands(n)),
                _ => thousands(n),
            }
        } else {
            self.hits
                .get(self.selected)
                .map(|h| taken_at(h.mtime))
                .unwrap_or_default()
        };
        div()
            .flex_none()
            .h(px(HEADER))
            .flex()
            .items_center()
            .gap_4()
            .px(px(PAD + 8.))
            .border_b_1()
            .border_color(theme.hairline)
            .child(
                div()
                    .flex_1()
                    .text_size(px(21.))
                    .line_height(px(30.))
                    .child(self.input.clone()),
            )
            .children(reading)
            .child(div().text_sm().text_color(theme.muted).child(count))
            .child(button(
                "settings-hint",
                keys::shown("settings", cx),
                "settings",
                theme,
                cx.listener(|this, _: &ClickEvent, window, cx| this.open_settings(window, cx)),
            ))
            .into_any_element()
    }

    fn render_empty(&self, theme: Theme, cx: &App) -> impl IntoElement + use<> {
        let settings = keys::shown("settings", cx);
        let (title, body): (SharedString, SharedString) = if self.searchable == 0 {
            match &self.reader {
                Some(ReaderState {
                    running: true,
                    line,
                }) if line == "up to date" => (
                    "no screenshots here yet".into(),
                    format!(
                        "the folders you picked have none. take one, or add a folder in settings ({settings})"
                    )
                    .into(),
                ),
                Some(ReaderState {
                    running: true,
                    line,
                }) => ("reading your screenshots".into(), line.clone().into()),
                Some(ReaderState {
                    running: false,
                    line,
                }) if line.starts_with("couldn't") || line.starts_with("stopped") => {
                    ("the reader stopped".into(), line.clone().into())
                }
                _ => (
                    "nothing read yet".into(),
                    format!(
                        "nothing is reading your screenshots right now. turn it on in settings ({settings})"
                    )
                    .into(),
                ),
            }
        } else if self.parsed.terms.is_empty() {
            (
                "nothing taken there or then".into(),
                "filters: in:folder, date:today, date:week, date:aug, before: and after:".into(),
            )
        } else if !self.parsed.filters.is_empty() {
            (
                format!(
                    "nothing says \u{201c}{}\u{201d}",
                    self.parsed.terms.join(" ")
                )
                .into(),
                "not with those filters, at least. try without them".into(),
            )
        } else {
            (
                format!("nothing says \u{201c}{}\u{201d}", self.query.trim()).into(),
                "fewer letters usually finds it, the middle of a word works too".into(),
            )
        };
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_1()
            .pb(px(HEADER))
            .child(div().text_lg().font_weight(FontWeight::MEDIUM).child(title))
            .child(div().text_sm().text_color(theme.muted).child(body))
    }

    fn render_scrollbar(&self, theme: Theme) -> Option<impl IntoElement + use<>> {
        let max = self.list.max_offset_for_scrollbar().y.as_f32();
        let viewport = self.list.viewport_bounds();
        let height = viewport.size.height.as_f32();
        if max <= 1.0 || height <= 0.0 {
            return None;
        }
        let offset = (-self.list.scroll_px_offset_for_scrollbar().y.as_f32()).clamp(0.0, max);
        let thumb = (height * height / (height + max)).max(28.0);
        let top = (height - thumb) * offset / max;
        Some(
            div()
                .absolute()
                .right(px(4.))
                .top(px(HEADER + top))
                .w(px(4.))
                .h(px(thumb))
                .rounded_full()
                .bg(theme.faint)
                .opacity(0.5),
        )
    }

    fn render_detail(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let theme = *cx.global::<Theme>();
        let viewport = window.viewport_size();
        let corner = self.corner();
        let shown = self.detail.as_ref()?.hit;
        let thumb_path = self.thumb_paths.get(shown)?.clone();
        let full_path = self.hits.get(shown)?.path.clone();
        let thumb = self.image(&thumb_path, Some(&full_path), window, cx);
        let full = self.image(&full_path, None, window, cx);
        let d = self.detail.as_mut()?;
        let hit = self.hits.get(d.hit)?;
        let p = d.open.value;

        let area = Bounds::new(
            point(px(DETAIL_PAD), px(DETAIL_TOP)),
            size(
                viewport.width - px(DETAIL_PAD * 2.),
                viewport.height - px(DETAIL_TOP + DETAIL_BOTTOM),
            ),
        );
        d.preview_area = area;
        let clip = if d.zoom.scale > 1. {
            area
        } else {
            Bounds::new(point(px(0.), px(0.)), viewport)
        };
        let base = fit(hit.width as f32 / hit.height.max(1) as f32, area);
        let target = d.zoom.rect(base, area);
        // The tile only shows the crop, so the full image starts out as the
        // rectangle that would put the crop exactly where the tile was.
        let crop = gyotaku_core::tile_crop(hit.width, hit.height);
        let from = Bounds::new(
            point(
                d.from.origin.x - d.from.size.width * (crop.x / crop.w),
                d.from.origin.y - d.from.size.height * (crop.y / crop.h),
            ),
            size(d.from.size.width / crop.w, d.from.size.height / crop.h),
        );
        let (rect, fade) = if d.grow {
            (lerp_bounds(from, target, p), 1.0)
        } else {
            (target, p.clamp(0.0, 1.0))
        };
        d.image_rect = rect;
        let chrome = ((p - 0.4) / 0.6).clamp(0.0, 1.0);

        let thumb_rect = Bounds::new(
            point(
                rect.origin.x + rect.size.width * crop.x,
                rect.origin.y + rect.size.height * crop.y,
            ),
            size(rect.size.width * crop.w, rect.size.height * crop.h),
        );
        let radius = px(RADIUS + (6.0 - RADIUS) * p);

        let mut boxes = Vec::new();
        for (i, line) in d.lines.iter().enumerate() {
            let b = on_screen(line.rect, rect).dilate(px(3.));
            // Same rule as the grid: what the search found is shu, what you
            // selected is ink.
            let (bg, border) = if d.picked.contains(&i) {
                (theme.hover_wash, Some(theme.text))
            } else if d.matched.get(i).copied().unwrap_or(false) {
                (theme.accent_wash, Some(theme.accent))
            } else if d.hovered == Some(i) {
                (theme.hover_wash, None)
            } else {
                continue;
            };
            let mut el = div()
                .absolute()
                .left(b.origin.x - clip.origin.x)
                .top(b.origin.y - clip.origin.y)
                .w(b.size.width)
                .h(b.size.height)
                .rounded(px(4.))
                .bg(bg);
            if let Some(border) = border {
                el = el.border_1().border_color(border);
            }
            boxes.push(el.opacity(chrome));
        }

        let drag = d
            .drag
            .filter(|(a, b)| (a.x - b.x).abs() > px(4.) || (a.y - b.y).abs() > px(4.))
            .map(|(a, b)| {
                let r = Bounds::from_corners(a.min(&b), a.max(&b));
                div()
                    .absolute()
                    .left(r.origin.x - clip.origin.x)
                    .top(r.origin.y - clip.origin.y)
                    .w(r.size.width)
                    .h(r.size.height)
                    .rounded(px(3.))
                    .border_1()
                    .border_color(theme.accent)
                    .bg(theme.accent_wash)
                    .opacity(0.6)
            });

        let name = hit
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let when = taken_at(hit.mtime);

        Some(
            div()
                .id("detail")
                .absolute()
                .size_full()
                .occlude()
                .cursor(if d.hovered.is_some() {
                    CursorStyle::PointingHand
                } else {
                    CursorStyle::Arrow
                })
                .on_pinch(cx.listener(move |this, event: &gpui::PinchEvent, _, cx| {
                    let Some(detail) = this.detail.as_mut().filter(|d| d.open.target() == 1.)
                    else {
                        return;
                    };
                    cx.stop_propagation();
                    if event.phase == gpui::TouchPhase::Started {
                        detail.zoom.begin();
                    }
                    detail.zoom.pinch(event.delta, event.position, base, area);
                    detail.hovered = None;
                    detail.drag = None;
                    cx.notify();
                }))
                .on_scroll_wheel(
                    cx.listener(move |this, event: &gpui::ScrollWheelEvent, _, cx| {
                        let Some(detail) = this.detail.as_mut().filter(|d| d.open.target() == 1.)
                        else {
                            return;
                        };
                        let delta = event.delta.pixel_delta(px(24.));
                        if event.modifiers.control || event.modifiers.platform {
                            let scale = detail.zoom.scale * (delta.y.as_f32() * 0.0015).exp();
                            detail.zoom.set(scale, event.position, base, area);
                        } else if detail.zoom.scale > 1. {
                            detail.zoom.move_by(delta, base, area);
                        } else {
                            return;
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
                .on_mouse_move(cx.listener(Self::detail_mouse_move))
                .on_mouse_down(MouseButton::Left, cx.listener(Self::detail_mouse_down))
                .on_mouse_up(MouseButton::Left, cx.listener(Self::detail_mouse_up))
                .child(
                    div()
                        .absolute()
                        .size_full()
                        .rounded(corner)
                        .bg(theme.panel)
                        .opacity(p.clamp(0.0, 1.0)),
                )
                // The thumbnail is already decoded, so it carries the move
                // until the full size image lands on top of it.
                .child(
                    div()
                        .absolute()
                        .left(clip.origin.x)
                        .top(clip.origin.y)
                        .w(clip.size.width)
                        .h(clip.size.height)
                        .overflow_hidden()
                        .children(thumb.map(|t| {
                            img(t)
                                .absolute()
                                .left(thumb_rect.origin.x - clip.origin.x)
                                .top(thumb_rect.origin.y - clip.origin.y)
                                .w(thumb_rect.size.width)
                                .h(thumb_rect.size.height)
                                .rounded(radius)
                                .opacity(fade)
                        }))
                        .children(full.map(|f| {
                            img(f)
                                .absolute()
                                .left(rect.origin.x - clip.origin.x)
                                .top(rect.origin.y - clip.origin.y)
                                .w(rect.size.width)
                                .h(rect.size.height)
                                .rounded(radius)
                                .opacity(fade)
                        }))
                        .children(boxes)
                        .children(drag),
                )
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .h(px(DETAIL_TOP))
                        .px(px(DETAIL_PAD + 4.))
                        .flex()
                        .items_center()
                        .justify_between()
                        .opacity(chrome)
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .text_ellipsis()
                                .child(name),
                        )
                        .child(div().text_sm().text_color(theme.muted).child(when)),
                )
                .child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(px(DETAIL_BOTTOM))
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap_1()
                        .opacity(chrome)
                        // Pressing a button here mustn't also start a line
                        // selection on the shot behind, which would forget
                        // the lines just picked before "copy text" sees them.
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(button(
                            "detail-back",
                            "esc",
                            "back",
                            theme,
                            cx.listener(|this, _, window, cx| this.back(&Back, window, cx)),
                        ))
                        .child(button(
                            "detail-copy-text",
                            keys::shown("copy_text", cx),
                            "copy text",
                            theme,
                            cx.listener(|this, _, window, cx| {
                                this.copy_text(&CopyText, window, cx)
                            }),
                        ))
                        .child(button(
                            "detail-copy-image",
                            keys::shown("copy_image", cx),
                            "copy image",
                            theme,
                            cx.listener(|this, _, window, cx| {
                                this.copy_image(&CopyImage, window, cx)
                            }),
                        ))
                        .child(button(
                            "detail-open",
                            "enter",
                            "open",
                            theme,
                            cx.listener(|this, _, window, cx| this.open(&Open, window, cx)),
                        ))
                        .child(button(
                            "detail-trash",
                            keys::shown("trash", cx),
                            "trash",
                            theme,
                            cx.listener(|this, _, window, cx| this.trash(&Trash, window, cx)),
                        ))
                        .child(button(
                            "detail-previous",
                            "\u{2190}",
                            "previous",
                            theme,
                            cx.listener(|this, _, window, cx| this.left(&Left, window, cx)),
                        ))
                        .child(button(
                            "detail-next",
                            "\u{2192}",
                            "next",
                            theme,
                            cx.listener(|this, _, window, cx| this.right(&Right, window, cx)),
                        )),
                )
                .into_any_element(),
        )
    }

    /// While shots are marked: how many, and what can be done with them.
    /// After ctrl delete it turns into the question itself.
    fn render_mark_bar(
        &mut self,
        theme: Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let count = if self.confirming {
            self.trash_targets().len()
        } else {
            self.marked.len()
        };
        let showing = (self.confirming || self.detail.is_none()) && count > 0 && !self.on_panel();
        let calm = self.calm(cx);

        // Gone from the state, the bar still sinks away saying what it said.
        let mut leaving = None;
        if showing {
            self.bar_leaving = None;
            self.bar_last = Some(Bar {
                count,
                confirming: self.confirming,
                can_mark_more: self.searching() && count < self.found,
            });
        } else {
            self.bar_shown = None;
            self.bar_last?;
            let since = *self.bar_leaving.get_or_insert_with(Instant::now);
            let t = since.elapsed().as_secs_f32() / BAR_OUT.as_secs_f32();
            if calm || t >= 1.0 {
                self.bar_last = None;
                self.bar_leaving = None;
                return None;
            }
            window.request_animation_frame();
            leaving = Some(ease_out_quint()(t));
        }
        let state = self.bar_last?;
        let n = state.count;
        let shown = match self.bar_shown {
            Some(b) => b,
            None if showing => {
                self.bars += 1;
                self.bar_shown = Some(self.bars);
                self.bars
            }
            None => 0,
        };

        let mut row = div().flex().items_center().gap_2();
        if state.confirming {
            let question = if n == 1 {
                "move this screenshot to the trash?".to_string()
            } else {
                format!("move {} screenshots to the trash?", thousands(n))
            };
            row = row
                .child(div().font_weight(FontWeight::MEDIUM).pr_2().child(question))
                .child(button(
                    "bar-yes",
                    "enter",
                    "move",
                    theme,
                    // The bar is still on screen for a moment as it slides
                    // away, so a click then mustn't count as a yes.
                    cx.listener(|this, _, _, cx| {
                        if this.confirming {
                            this.trash_now(cx)
                        }
                    }),
                ))
                .child(button(
                    "bar-no",
                    "esc",
                    "cancel",
                    theme,
                    cx.listener(|this, _, _, cx| {
                        this.confirming = false;
                        cx.notify();
                    }),
                ));
        } else {
            row = row
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .pr_2()
                        .child(format!("{} marked", thousands(n))),
                )
                .child(button(
                    "bar-trash",
                    keys::shown("trash", cx),
                    "move to trash",
                    theme,
                    cx.listener(|this, _, _, cx| {
                        if !this.trash_targets().is_empty() {
                            this.confirming = true;
                            cx.notify();
                        }
                    }),
                ))
                .when(state.can_mark_more, |row| {
                    row.child(button(
                        "bar-all",
                        keys::shown("mark_all", cx),
                        "mark all",
                        theme,
                        cx.listener(|this, _, window, cx| this.mark_all(&MarkAll, window, cx)),
                    ))
                })
                .child(button(
                    "bar-clear",
                    "esc",
                    "clear",
                    theme,
                    cx.listener(|this, _, _, cx| {
                        this.clear_marks();
                        cx.notify();
                    }),
                ));
        }

        // Turning into the question (and back) swaps what it says with a
        // quick fade rather than a cut.
        let row = if calm {
            row.into_any_element()
        } else {
            row.with_animation(
                ElementId::Name(format!("bar-{shown}-{}", state.confirming).into()),
                Animation::new(BAR_SWAP).with_easing(ease_out_quint()),
                |el, t| el.opacity(t),
            )
            .into_any_element()
        };
        // Clicks on the bar stop at the bar, rather than also landing on the
        // tile underneath it and opening that.
        let pill = div()
            .id("mark-bar")
            .occlude()
            .pl_4()
            .pr_1p5()
            .py_1p5()
            .rounded_full()
            .bg(theme.panel)
            .border_1()
            .border_color(theme.hairline)
            .shadow_lg()
            .text_sm()
            .child(row);
        let bar = div()
            .absolute()
            .left_0()
            .right_0()
            .bottom(px(16.))
            .flex()
            .justify_center()
            .child(pill);

        if let Some(t) = leaving {
            return Some(
                bar.bottom(px(16. - 8. * t))
                    .opacity(1. - t)
                    .into_any_element(),
            );
        }
        if calm {
            return Some(bar.into_any_element());
        }
        Some(
            bar.with_animation(
                ElementId::Name(format!("bar-{shown}").into()),
                Animation::new(BAR_IN).with_easing(ease_out_quint()),
                |el, t| el.bottom(px(8. + 8. * t)).opacity(t),
            )
            .into_any_element(),
        )
    }

    fn render_toast(
        &self,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let toast = self.toast.as_ref()?;
        let (id, leaving) = (toast.id, toast.leaving);
        // The fade out has its own id so it starts fresh instead of carrying
        // on from where the fade in ended.
        let phase = if leaving { "out" } else { "in" };
        Some(
            div()
                .absolute()
                .bottom(px(DETAIL_BOTTOM + 12.))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(
                    div()
                        .id("toast")
                        .occlude()
                        .px_4()
                        .py_2()
                        .rounded_full()
                        .bg(theme.text)
                        .text_color(theme.panel)
                        .text_sm()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(toast.message.clone())
                        .children(toast.keys.clone().map(|(keys, what)| {
                            // The only key a toast offers is undo, so it's
                            // also the one thing the toast can be clicked for.
                            div()
                                .id("toast-action")
                                .flex()
                                .items_center()
                                .gap_1p5()
                                .text_xs()
                                .opacity(0.7)
                                .when(what == "undo", |d| {
                                    d.cursor(CursorStyle::PointingHand)
                                        .hover(|s| s.opacity(1.0))
                                        .on_click(cx.listener(
                                            |this, _: &ClickEvent, window, cx| {
                                                this.undo(&Undo, window, cx)
                                            },
                                        ))
                                })
                                .child(
                                    div()
                                        .px(px(6.))
                                        .rounded(px(5.))
                                        .border_1()
                                        .border_color(theme.panel)
                                        .child(keys),
                                )
                                .child(what)
                        }))
                        .with_animation(
                            ElementId::Name(format!("toast-{id}-{phase}").into()),
                            Animation::new(TOAST_FADE).with_easing(ease_out_quint()),
                            move |el, t| el.opacity(if leaving { 1.0 - t } else { t }),
                        ),
                ),
        )
    }
}

impl Focusable for Gyotaku {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self.page {
            Page::Search => self.input.focus_handle(cx),
            Page::Settings(_) | Page::Onboarding(_) => self.panel_focus.clone(),
        }
    }
}

impl Render for Gyotaku {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let started = self.stats.begin();
        self.thumbs.new_frame();
        self.full.new_frame();
        let theme = *cx.global::<Theme>();

        // The real time since the last frame, so motion takes as long on a
        // slow machine as on a fast one (it just gets fewer frames). Capping
        // this at a 30 fps step made a 0.2 s fade last seconds where frames
        // take 150 ms. The spring sub-steps big gaps itself.
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.25);
        self.last_frame = now;
        if let Some(d) = &mut self.detail {
            if !d.open.is_settled() {
                d.open.step(dt);
                window.request_animation_frame();
            } else if d.open.target() == 0.0 {
                self.detail = None;
            }
        }

        let width = window.viewport_size().width.as_f32() - PAD * 2.0;
        if (width - self.laid_out_for).abs() > 0.5 && width > 0.0 {
            self.relayout(width);
        }

        let toast = self.render_toast(theme, cx);
        let mut content: Vec<AnyElement> = match self.page {
            Page::Settings(_) => vec![self.render_settings(theme, window, cx)],
            Page::Onboarding(_) => vec![self.render_onboarding(theme, cx)],
            Page::Search => {
                let body: AnyElement = if self.hits.is_empty() {
                    self.render_empty(theme, cx).into_any_element()
                } else {
                    list(self.list.clone(), cx.processor(Self::render_row))
                        .flex_1()
                        .size_full()
                        .pt(px(6.))
                        .into_any_element()
                };
                let mut content = vec![self.render_header(theme, window, cx), body];
                content.extend(self.render_scrollbar(theme).map(|s| s.into_any_element()));
                content.extend(self.render_detail(window, cx));
                content.extend(self.render_mark_bar(theme, window, cx));
                content
            }
        };
        // Settings and onboarding get their own focus and key context, so
        // space and delete mean "toggle" and "remove" there instead of typing.
        let panel = self.on_panel().then(|| {
            div()
                .key_context("Panel")
                .track_focus(&self.panel_focus)
                .on_action(cx.listener(Self::toggle))
                .on_action(cx.listener(Self::remove))
                .on_action(cx.listener(Self::add_folder))
                .on_key_down(cx.listener(Self::panel_key_down))
                .flex_1()
                // Without this a flex item is at least as tall as what's in
                // it, so a long settings page pushes past the window instead
                // of scrolling inside it.
                .min_h(px(0.))
                .flex()
                .flex_col()
                .children(content.drain(..))
        });

        let root = div()
            .key_context("Gyotaku")
            .on_action(cx.listener(Self::back))
            .on_action(cx.listener(Self::open))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::page_up))
            .on_action(cx.listener(Self::page_down))
            .on_action(cx.listener(Self::copy_text))
            .on_action(cx.listener(Self::copy_image))
            .on_action(cx.listener(Self::open_external))
            .on_action(cx.listener(Self::reveal))
            .on_action(cx.listener(Self::quit))
            .on_action(cx.listener(Self::open_settings_action))
            .on_action(cx.listener(Self::mark_up))
            .on_action(cx.listener(Self::mark_down))
            .on_action(cx.listener(Self::mark_left))
            .on_action(cx.listener(Self::mark_right))
            .on_action(cx.listener(Self::mark_all))
            .on_action(cx.listener(Self::trash))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::similar))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(theme.panel)
            .rounded(self.corner())
            .when(self.own_frame(), |panel| {
                panel.border_1().border_color(theme.hairline)
            })
            .text_color(theme.text)
            .font_family("IBM Plex Sans")
            .children(panel)
            .children(content)
            .children(toast);
        self.stats.end(started);
        root
    }
}

/// A check mark drawn as a stroke. The font's ✓ at this size reads as a v.
fn check(color: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |b, _, window, _| {
            let at = |x: f32, y: f32| {
                point(
                    b.origin.x + b.size.width * x,
                    b.origin.y + b.size.height * y,
                )
            };
            let mut path = PathBuilder::stroke(px(1.75));
            path.move_to(at(0.28, 0.52));
            path.line_to(at(0.44, 0.67));
            path.line_to(at(0.73, 0.35));
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
        },
    )
    .size_full()
}

/// A key hint that can also be clicked, for whoever reached for the mouse.
fn button(
    id: impl Into<ElementId>,
    keys: impl Into<SharedString>,
    what: &'static str,
    theme: Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px_2()
        .py_1()
        .rounded_full()
        .cursor(CursorStyle::PointingHand)
        .hover(move |s| s.bg(theme.hover_wash))
        .on_click(on_click)
        .child(hint(keys, what, theme))
}

fn hint(keys: impl Into<SharedString>, what: &'static str, theme: Theme) -> impl IntoElement {
    let keys: SharedString = keys.into();
    div()
        .flex()
        .items_center()
        .gap_2()
        .text_xs()
        .child(
            div()
                .px(px(6.))
                .py(px(1.))
                .rounded(px(5.))
                .bg(theme.keycap)
                .border_1()
                .border_color(theme.hairline)
                .text_color(theme.text)
                .child(keys),
        )
        .child(div().text_color(theme.muted).child(what))
}

/// A line's box in tile pixels, grown a little so the outline doesn't sit on
/// the glyphs. None when the line falls outside the part the tile shows.
fn in_tile(r: Rect, crop: Rect, w: f32, h: f32) -> Option<Rect> {
    const GROW: f32 = 2.0;
    let x0 = ((r.x - crop.x) / crop.w * w - GROW).max(0.0);
    let y0 = ((r.y - crop.y) / crop.h * h - GROW).max(0.0);
    let x1 = ((r.x + r.w - crop.x) / crop.w * w + GROW).min(w);
    let y1 = ((r.y + r.h - crop.y) / crop.h * h + GROW).min(h);
    (x1 - x0 >= 3.0 && y1 - y0 >= 3.0).then_some(Rect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    })
}

fn on_screen(r: Rect, image: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        point(
            image.origin.x + image.size.width * r.x,
            image.origin.y + image.size.height * r.y,
        ),
        size(image.size.width * r.w, image.size.height * r.h),
    )
}

/// Use the available preview area, including for smaller screenshots.
fn fit(aspect: f32, area: Bounds<Pixels>) -> Bounds<Pixels> {
    let (aw, ah) = (area.size.width.as_f32(), area.size.height.as_f32());
    let (w, h) = if aw / ah > aspect {
        (ah * aspect, ah)
    } else {
        (aw, aw / aspect)
    };
    Bounds::new(
        point(
            area.origin.x + px((aw - w) / 2.0),
            area.origin.y + px((ah - h) / 2.0),
        ),
        size(px(w), px(h)),
    )
}

fn lerp_bounds(a: Bounds<Pixels>, b: Bounds<Pixels>, t: f32) -> Bounds<Pixels> {
    let l = |x: Pixels, y: Pixels| x + (y - x) * t;
    Bounds::new(
        point(l(a.origin.x, b.origin.x), l(a.origin.y, b.origin.y)),
        size(
            l(a.size.width, b.size.width),
            l(a.size.height, b.size.height),
        ),
    )
}

fn placeholder(searchable: usize) -> SharedString {
    match searchable {
        0 => "nothing to search yet".into(),
        1 => "search 1 screenshot".into(),
        n => format!("search {} screenshots", thousands(n)).into(),
    }
}

#[derive(Clone, PartialEq)]
struct ReaderState {
    running: bool,
    /// What it last said it was doing, see `gyotaku_core::status`.
    line: String,
}

impl ReaderState {
    /// `reading 12 of 340` as (12, 340).
    fn progress(&self) -> Option<(usize, usize)> {
        let (done, of) = self.line.strip_prefix("reading ")?.split_once(" of ")?;
        Some((done.parse().ok()?, of.parse().ok()?))
    }

    fn getting_ready(&self) -> bool {
        self.line.starts_with("getting")
    }
}

/// `nothing_searchable` is when the empty screen needs to know whether the
/// reader is alive at all. Otherwise that's only checked while its last word
/// says it's busy, so a reader that died mid-way doesn't leave a progress
/// bar standing.
fn reader_state(nothing_searchable: bool) -> ReaderState {
    let line = gyotaku_core::status::get().unwrap_or_default();
    let busy = line.starts_with("reading") || line.starts_with("getting");
    ReaderState {
        running: (busy || nothing_searchable) && gyotaku_core::status::reader_running(),
        line,
    }
}

pub(crate) fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn taken_at(mtime: i64) -> String {
    jiff::Timestamp::from_second(mtime)
        .map(|t| {
            t.to_zoned(jiff::tz::TimeZone::system())
                .strftime("%-d %b %Y, %H:%M")
                .to_string()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_progress_comes_from_the_status_line() {
        let state = |line: &str| ReaderState {
            running: true,
            line: line.into(),
        };
        assert_eq!(state("reading 12 of 340").progress(), Some((12, 340)));
        assert_eq!(state("up to date").progress(), None);
        assert_eq!(state("reading your screenshots").progress(), None);
        assert!(state("getting the text reader ready").getting_ready());
    }

    #[test]
    fn thousands_separators() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(5159), "5,159");
        assert_eq!(thousands(1234567), "1,234,567");
    }

    #[test]
    fn boxes_outside_the_crop_are_dropped() {
        let crop = Rect {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 0.5,
        };
        let below = Rect {
            x: 0.1,
            y: 0.7,
            w: 0.3,
            h: 0.05,
        };
        assert!(in_tile(below, crop, 200.0, 400.0).is_none());
        let inside = Rect {
            x: 0.1,
            y: 0.1,
            w: 0.3,
            h: 0.05,
        };
        let b = in_tile(inside, crop, 200.0, 400.0).unwrap();
        assert!((b.y - (0.2 * 400.0 - 2.0)).abs() < 1e-3);
    }

    #[test]
    fn fit_keeps_the_aspect_and_centres() {
        let area = Bounds::new(point(px(0.), px(0.)), size(px(1000.), px(500.)));
        let r = fit(1.0, area);
        assert_eq!(r.size, size(px(500.), px(500.)));
        assert_eq!(r.origin.x, px(250.));
    }

    #[test]
    fn smaller_shots_use_the_available_width() {
        let area = Bounds::new(point(px(0.), px(0.)), size(px(1000.), px(500.)));
        let r = fit(586.0 / 134.0, area);
        let close = |a: Pixels, b: f32| (a.as_f32() - b).abs() < 0.01;
        assert!(
            close(r.size.width, 1000.) && close(r.size.height, 1000. * 134. / 586.),
            "{:?}",
            r.size
        );
        assert!(
            close(r.origin.x, 0.) && close(r.origin.y, (500. - 1000. * 134. / 586.) / 2.),
            "{:?}",
            r.origin
        );
    }
}
