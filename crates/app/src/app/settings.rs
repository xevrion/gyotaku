//! Onboarding (the first time, when there's no config yet) and settings
//! (the settings shortcut, any time after). Both are lists of rows you move through with
//! the arrows, or click. Hovering only tints a row: the highlight that keys
//! act on moves with the keys and with clicks, never with a resting mouse,
//! so enter does what the screen says rather than whatever the pointer is
//! over. Every change is saved as it's made, there's no apply button.

use std::path::PathBuf;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, App, ClickEvent, Context, CursorStyle, ElementId,
    Focusable as _, FontWeight, KeyDownEvent, PathPromptOptions, ScrollHandle, SharedString,
    Window, div, ease_out_quint, prelude::*, px,
};
use gyotaku_core::{Config, Script, ThemeChoice, tidy};

use super::{Gyotaku, Page, button, hint, thousands};
use crate::keys::{self, SHORTCUTS};
use crate::platform::{self, Service};
use crate::setup::{self, Candidate, Counts};
use crate::theme::Theme;

#[derive(Clone, Copy)]
pub(super) enum Key {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Space,
    Remove,
}

pub(super) struct Settings {
    config: Config,
    cursor: usize,
    service: Service,
    /// How many indexed shots live under each folder, in config order.
    shots: Vec<usize>,
    /// Index and thumbnail cache sizes, filled in from a background thread.
    sizes: Option<(u64, u64)>,
    /// The shortcut waiting for its new keys, by its place in SHORTCUTS.
    recording: Option<usize>,
    scroll: ScrollHandle,
    /// The cursor moved by keyboard, so its row gets scrolled into view.
    reveal: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Row {
    Folder(usize),
    AddFolder,
    Theme,
    Similar,
    Background,
    Clipboard,
    /// An extra writing system to read, by its place in `Script::ALL`.
    Script(usize),
    Threads,
    ClearThumbs,
    Support,
    Updates,
    Shortcut(usize),
}

// Both open in the browser through the system; the app itself never touches
// the network for them. Updates goes to the site's signup rather than the
// mailing list directly, so the list can move without a release.
const SPONSOR_URL: &str = "https://github.com/sponsors/xevrion";
const UPDATES_URL: &str = "https://gyotaku.app/#updates";

impl Settings {
    fn rows(&self) -> Vec<Row> {
        let mut rows: Vec<Row> = (0..self.config.folders.len()).map(Row::Folder).collect();
        rows.extend([
            Row::AddFolder,
            Row::Theme,
            Row::Similar,
            Row::Background,
            Row::Clipboard,
        ]);
        rows.extend((0..Script::ALL.len()).map(Row::Script));
        rows.extend([Row::Threads, Row::ClearThumbs, Row::Support, Row::Updates]);
        rows.extend((0..SHORTCUTS.len()).map(Row::Shortcut));
        rows
    }
}

pub(super) struct Onboarding {
    step: Step,
    picks: Vec<Pick>,
    cursor: usize,
    background: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Step {
    Folders,
    Background,
}

struct Pick {
    candidate: Candidate,
    count: Option<Counts>,
    on: bool,
}

impl Onboarding {
    /// Folder rows, then "add a folder". On the second step, yes and no.
    fn row_count(&self) -> usize {
        match self.step {
            Step::Folders => self.picks.len() + 1,
            Step::Background => 2,
        }
    }
}

// Counting past this would only slow the answer down, "10,000+" says enough.
const COUNT_CAP: usize = 10_000;
// A folder with this many screenshot-looking files is ticked up front.
const LOOKS_LIKE_SCREENSHOTS: usize = 10;

impl Gyotaku {
    pub(super) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A trash question left open would otherwise be answered by the
        // first enter after coming back.
        self.confirming = false;
        let config = Config::load_or_default();
        let paths = self.index.paths().unwrap_or_default();
        let shots = config
            .folders
            .iter()
            .map(|f| paths.iter().filter(|p| p.starts_with(f)).count())
            .collect();
        self.page = Page::Settings(Settings {
            config,
            cursor: 0,
            service: platform::service_status(),
            shots,
            sizes: None,
            recording: None,
            scroll: ScrollHandle::new(),
            reveal: false,
        });
        window.focus(&self.panel_focus, cx);

        cx.spawn(async move |this, cx| {
            let sizes = cx
                .background_executor()
                .spawn(async move {
                    let data = gyotaku_core::data_dir().unwrap_or_default();
                    let index =
                        setup::folder_size(&data) - setup::folder_size(&data.join("models"));
                    let thumbs = gyotaku_core::thumb_path(std::path::Path::new("x"))
                        .ok()
                        .and_then(|p| p.parent().map(setup::folder_size))
                        .unwrap_or(0);
                    (index, thumbs)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Page::Settings(s) = &mut this.page {
                    s.sizes = Some(sizes);
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn start_onboarding(&mut self, cx: &mut Context<Self>) {
        let picks: Vec<Pick> = setup::candidates()
            .into_iter()
            .map(|candidate| Pick {
                candidate,
                count: None,
                on: false,
            })
            .collect();
        let folders: Vec<PathBuf> = picks.iter().map(|p| p.candidate.path.clone()).collect();
        self.page = Page::Onboarding(Onboarding {
            step: Step::Folders,
            picks,
            cursor: 0,
            background: true,
        });
        self.count_folders(folders, cx);
    }

    /// Counts images in the background, then ticks the folders worth reading:
    /// wherever a screenshot tool is set to save, plus any folder that's
    /// clearly full of screenshots going by the file names. Size alone isn't
    /// enough, a Desktop can hold a thousand images that aren't screenshots.
    fn count_folders(&mut self, folders: Vec<PathBuf>, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let counted = cx
                .background_executor()
                .spawn(async move {
                    folders
                        .into_iter()
                        .map(|f| (setup::count_images(&f, COUNT_CAP), f))
                        .collect::<Vec<_>>()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let Page::Onboarding(o) = &mut this.page else {
                    return;
                };
                let untouched = o.picks.iter().all(|p| p.count.is_none() && !p.on);
                for (count, folder) in counted {
                    if let Some(pick) = o.picks.iter_mut().find(|p| p.candidate.path == folder) {
                        pick.count = Some(count);
                    }
                }
                if untouched {
                    for pick in &mut o.picks {
                        let counts = pick.count.unwrap_or_default();
                        pick.on = (pick.candidate.tool && counts.images > 0)
                            || counts.screenshots >= LOOKS_LIKE_SCREENSHOTS;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn leave_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.stop_recording(cx);
        self.page = Page::Search;
        window.focus(&self.input.focus_handle(cx), cx);
        cx.notify();
    }

    pub(super) fn panel_key(&mut self, key: Key, window: &mut Window, cx: &mut Context<Self>) {
        // While a shortcut listens no keys are bound, so this can only be a
        // click, somewhere else. That's a change of mind.
        self.stop_recording(cx);
        match &mut self.page {
            Page::Search => {}
            Page::Settings(s) => {
                let rows = s.rows();
                let row = rows.get(s.cursor).copied();
                match key {
                    Key::Up => {
                        s.cursor = s.cursor.saturating_sub(1);
                        s.reveal = true;
                    }
                    Key::Down => {
                        s.cursor = (s.cursor + 1).min(rows.len() - 1);
                        s.reveal = true;
                    }
                    Key::Left | Key::Right => {
                        let forward = matches!(key, Key::Right);
                        match row {
                            Some(Row::Theme) => self.cycle_theme(forward, window, cx),
                            Some(Row::Threads) => {
                                self.change_threads(if forward { 1 } else { -1 }, cx)
                            }
                            Some(Row::Background) => self.set_background(forward, cx),
                            Some(Row::Clipboard) => self.set_clipboard(forward, cx),
                            Some(Row::Script(i)) => self.set_script(i, forward, cx),
                            Some(Row::Similar) => self.set_grouped(forward, cx),
                            _ => {}
                        }
                    }
                    Key::Enter | Key::Space => match row {
                        Some(Row::AddFolder) => self.add_folders(window, cx),
                        Some(Row::Theme) => self.cycle_theme(true, window, cx),
                        Some(Row::Background) => {
                            let on = matches!(s.service, Service::Running);
                            self.set_background(!on, cx)
                        }
                        Some(Row::Clipboard) => {
                            let on = s.config.clipboard;
                            self.set_clipboard(!on, cx)
                        }
                        Some(Row::Script(i)) => {
                            let on = s.config.scripts().contains(&Script::ALL[i]);
                            self.set_script(i, !on, cx)
                        }
                        Some(Row::Similar) => {
                            let on = s.config.group_similar;
                            self.set_grouped(!on, cx)
                        }
                        Some(Row::Threads) => self.change_threads(1, cx),
                        Some(Row::ClearThumbs) => self.clear_thumbnails(cx),
                        Some(Row::Support) => cx.open_url(SPONSOR_URL),
                        Some(Row::Updates) => cx.open_url(UPDATES_URL),
                        Some(Row::Shortcut(i)) => self.start_recording(i, cx),
                        Some(Row::Folder(_)) | None => {}
                    },
                    Key::Remove => match row {
                        Some(Row::Folder(i)) => self.remove_folder(i, cx),
                        Some(Row::Shortcut(i)) => self.reset_shortcut(i, cx),
                        _ => {}
                    },
                }
            }
            // Each step has one way forward and enter always takes it, the
            // way Raycast's onboarding works. Picking and adding folders
            // have keys and clicks of their own, so nothing the cursor rests
            // on can turn enter into something else.
            Page::Onboarding(o) => {
                let last = o.row_count() - 1;
                match (key, o.step) {
                    (Key::Up, Step::Folders) => o.cursor = o.cursor.saturating_sub(1),
                    (Key::Down, Step::Folders) => o.cursor = (o.cursor + 1).min(last),
                    (Key::Space, Step::Folders) => {
                        if let Some(pick) = o.picks.get_mut(o.cursor) {
                            pick.on = !pick.on;
                        } else {
                            self.add_folders(window, cx);
                        }
                    }
                    (Key::Enter, Step::Folders) => {
                        if o.picks.iter().any(|p| p.on) {
                            o.step = Step::Background;
                            o.cursor = if o.background { 0 } else { 1 };
                        } else {
                            self.flash("pick at least one folder", cx);
                        }
                    }
                    // Two answers, one of them always chosen: the arrows
                    // choose, like any radio group.
                    (Key::Up, Step::Background) => {
                        o.background = true;
                        o.cursor = 0;
                    }
                    (Key::Down, Step::Background) => {
                        o.background = false;
                        o.cursor = 1;
                    }
                    (Key::Space | Key::Left | Key::Right, Step::Background) => {
                        o.background = !o.background;
                        o.cursor = if o.background { 0 } else { 1 };
                    }
                    (Key::Enter, Step::Background) => self.finish_onboarding(window, cx),
                    _ => {}
                }
            }
        }
        cx.notify();
    }

    /// Esc: onboarding steps back, settings goes back to searching.
    pub(super) fn panel_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match &mut self.page {
            Page::Onboarding(o) if o.step == Step::Background => {
                o.step = Step::Folders;
                o.cursor = 0;
                cx.notify();
            }
            // Nothing to go back to before the first step, so it just hides,
            // and the next summon lands here again.
            Page::Onboarding(_) => window.remove_window(),
            Page::Settings(_) => self.leave_panel(window, cx),
            Page::Search => {}
        }
    }

    fn finish_onboarding(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Page::Onboarding(o) = &self.page else {
            return;
        };
        let folders = setup::without_nested(
            o.picks
                .iter()
                .filter(|p| p.on)
                .map(|p| p.candidate.path.clone())
                .collect(),
        );
        let background = o.background;
        let config = Config {
            folders,
            ..Config::default()
        };
        if let Err(e) = config.save() {
            self.flash(format!("couldn't save the config: {e}"), cx);
            return;
        }
        self.theme_choice = config.theme;
        if background && platform::service_status() != Service::Running {
            cx.background_executor()
                .spawn(async { platform::start_service() })
                .detach();
        }
        // Whatever was set up before (a start-with-Windows entry left from an
        // earlier install looks like "already running"), reading starts now.
        platform::keep_reading();
        self.leave_panel(window, cx);
        self.flash(
            if background {
                "reading your screenshots, they show up here as they're read"
            } else {
                platform::WORDS.chose_no
            },
            cx,
        );
    }

    /// Writes the config and puts the live copy in settings in step with it.
    fn save(&mut self, config: Config, cx: &mut Context<Self>) {
        if let Err(e) = config.save() {
            self.flash(format!("couldn't save: {e}"), cx);
            return;
        }
        if let Page::Settings(s) = &mut self.page {
            let paths = self.index.paths().unwrap_or_default();
            s.shots = config
                .folders
                .iter()
                .map(|f| paths.iter().filter(|p| p.starts_with(f)).count())
                .collect();
            s.config = config;
        }
        cx.notify();
    }

    /// Waits for the next keys pressed, which become the shortcut. Every
    /// binding is lifted meanwhile, or pressing ctrl c to bind it would copy.
    fn start_recording(&mut self, i: usize, cx: &mut Context<Self>) {
        if let Page::Settings(s) = &mut self.page {
            s.recording = Some(i);
            cx.clear_key_bindings();
        }
    }

    /// Back to listening for shortcuts, with whatever is saved now.
    pub(super) fn stop_recording(&mut self, cx: &mut Context<Self>) {
        if let Page::Settings(s) = &mut self.page
            && s.recording.take().is_some()
        {
            let overrides = s.config.keys.clone();
            keys::bind_all(cx, &overrides);
            cx.notify();
        }
    }

    pub(super) fn panel_key_down(
        &mut self,
        e: &KeyDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Page::Settings(s) = &self.page else {
            return;
        };
        let Some(i) = s.recording else {
            return;
        };
        cx.stop_propagation();
        let k = &e.keystroke;
        if k.key == "escape" && !k.modifiers.modified() {
            return self.stop_recording(cx);
        }
        let key = k.unparse();
        // A refusal keeps listening, so the next try can just be pressed.
        if let Err(why) = keys::usable(&key) {
            return self.flash(why, cx);
        }
        let mut config = s.config.clone();
        if let Some(j) = keys::taken_by(&key, i, &config.keys) {
            return self.flash(
                format!("{} is already {}", keys::pretty(&key), SHORTCUTS[j].label),
                cx,
            );
        }
        let shortcut = &SHORTCUTS[i];
        if key == keys::default(shortcut) {
            config.keys.remove(shortcut.name);
        } else {
            config.keys.insert(shortcut.name.into(), key.clone());
        }
        self.save(config, cx);
        self.stop_recording(cx);
        self.flash(
            format!("{} is {} now", shortcut.label, keys::pretty(&key)),
            cx,
        );
    }

    fn reset_shortcut(&mut self, i: usize, cx: &mut Context<Self>) {
        let mut config = self.current_config();
        let shortcut = &SHORTCUTS[i];
        // The default may have been given to another shortcut since.
        if let Some(j) = keys::taken_by(keys::default(shortcut), i, &config.keys) {
            return self.flash(
                format!(
                    "{} is {} now, change that one first",
                    keys::pretty(keys::default(shortcut)),
                    SHORTCUTS[j].label
                ),
                cx,
            );
        }
        if config.keys.remove(shortcut.name).is_none() {
            return;
        }
        keys::bind_all(cx, &config.keys);
        self.save(config, cx);
        self.flash(
            format!(
                "{} is back to {}",
                shortcut.label,
                keys::pretty(keys::default(shortcut))
            ),
            cx,
        );
    }

    fn current_config(&self) -> Config {
        match &self.page {
            Page::Settings(s) => s.config.clone(),
            _ => Config::load_or_default(),
        }
    }

    fn cycle_theme(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let mut config = self.current_config();
        let order = [ThemeChoice::System, ThemeChoice::Light, ThemeChoice::Dark];
        let at = order.iter().position(|t| *t == config.theme).unwrap_or(0);
        config.theme = order[if forward { (at + 1) % 3 } else { (at + 2) % 3 }];
        self.set_theme(config, window, cx);
    }

    fn set_theme(&mut self, config: Config, window: &mut Window, cx: &mut Context<Self>) {
        self.theme_choice = config.theme;
        cx.set_global(Theme::resolve(config.theme, window.appearance()));
        self.save(config, cx);
    }

    fn change_threads(&mut self, by: isize, cx: &mut Context<Self>) {
        let mut config = self.current_config();
        let max = std::thread::available_parallelism()
            .map_or(8, |n| n.get())
            .min(16);
        config.threads = (config.threads as isize + by).clamp(1, max as isize) as usize;
        self.save(config, cx);
    }

    /// The reader follows the config, so saving it is all that turns the
    /// clipboard watching on or off. The folder is written down the first
    /// time, so it's still read after saving is turned off again.
    fn set_clipboard(&mut self, on: bool, cx: &mut Context<Self>) {
        let mut config = self.current_config();
        if config.clipboard == on {
            return;
        }
        config.clipboard = on;
        if config.clipboard_folder.is_none() {
            config.clipboard_folder = config.clipboard_folder();
        }
        self.save(config, cx);
    }

    /// Like the clipboard, the reader follows the config: it downloads the
    /// script's model and starts using it on the next screenshot.
    fn set_script(&mut self, i: usize, on: bool, cx: &mut Context<Self>) {
        let mut config = self.current_config();
        let script = Script::ALL[i];
        if config.scripts().contains(&script) == on {
            return;
        }
        config.set_script(script, on);
        self.save(config, cx);
    }

    /// Stacks similar shots, or shows every one. The results behind settings
    /// are redone, so they're right when it closes.
    fn set_grouped(&mut self, on: bool, cx: &mut Context<Self>) {
        let mut config = self.current_config();
        if config.group_similar == on {
            return;
        }
        config.group_similar = on;
        self.grouped = on;
        self.unfolded.clear();
        self.refresh(cx);
        self.save(config, cx);
    }

    fn remove_folder(&mut self, i: usize, cx: &mut Context<Self>) {
        let mut config = self.current_config();
        if i < config.folders.len() {
            let gone = config.folders.remove(i);
            self.flash(format!("stopped reading {}", tidy(&gone)), cx);
            if let Page::Settings(s) = &mut self.page {
                s.cursor = s.cursor.min(config.folders.len());
            }
            self.save(config, cx);
        }
    }

    fn clear_thumbnails(&mut self, cx: &mut Context<Self>) {
        if let Some(dir) = gyotaku_core::thumb_path(std::path::Path::new("x"))
            .ok()
            .and_then(|p| p.parent().map(PathBuf::from))
        {
            let _ = std::fs::remove_dir_all(&dir);
        }
        // They come back one by one as tiles are drawn, from the originals.
        if let Page::Settings(s) = &mut self.page
            && let Some((index, _)) = s.sizes
        {
            s.sizes = Some((index, 0));
        }
        self.flash("thumbnails cleared, they'll be redrawn as needed", cx);
    }

    fn set_background(&mut self, on: bool, cx: &mut Context<Self>) {
        let Page::Settings(s) = &mut self.page else {
            return;
        };
        if (s.service == Service::Running) == on {
            return;
        }
        cx.spawn(async move |this, cx| {
            let status = cx
                .background_executor()
                .spawn(async move {
                    if on {
                        platform::start_service()
                    } else {
                        platform::stop_service()
                    };
                    platform::service_status()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Page::Settings(s) = &mut this.page {
                    s.service = status;
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Opens the desktop's folder picker. The overlay sits above every
    /// window, the picker included, so it gets out of the way while you pick
    /// and comes back after, right where it was.
    pub(super) fn add_folders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: true,
            prompt: Some("Read this folder".into()),
        });
        let step_aside = self.floating && crate::is_resident(cx);
        if step_aside {
            window.remove_window();
        }
        platform::before_picker(cx);
        cx.spawn(async move |this, cx| {
            let result = picked.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(Ok(Some(paths))) => this.folders_picked(paths, cx),
                Ok(Ok(None)) => {}
                _ => this.flash("couldn't open a folder picker", cx),
            });
            if step_aside {
                cx.update(crate::summon);
            }
        })
        .detach();
    }

    fn folders_picked(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let (paths, refused): (Vec<PathBuf>, Vec<PathBuf>) =
            paths.into_iter().partition(|p| !gyotaku_core::too_broad(p));
        if let Some(broad) = refused.first() {
            self.flash(
                format!(
                    "{} is everything, pick the folder your screenshots are in",
                    tidy(broad)
                ),
                cx,
            );
        }
        if paths.is_empty() {
            return;
        }
        match &mut self.page {
            Page::Onboarding(o) => {
                let mut fresh = Vec::new();
                for path in paths {
                    if let Some(pick) = o.picks.iter_mut().find(|p| p.candidate.path == path) {
                        pick.on = true;
                    } else {
                        let candidate = Candidate {
                            path: path.clone(),
                            why: "added by you".into(),
                            tool: false,
                        };
                        o.picks.push(Pick {
                            candidate,
                            count: None,
                            on: true,
                        });
                        fresh.push(path);
                    }
                }
                self.count_folders(fresh, cx);
            }
            Page::Settings(_) => {
                let mut config = self.current_config();
                config.folders.extend(paths);
                config.folders = setup::without_nested(config.folders);
                self.save(config, cx);
                self.flash("added, the watcher starts reading it now", cx);
            }
            Page::Search => {}
        }
        cx.notify();
    }

    /// Moves the highlight keys act on. Only keys and clicks call this; on
    /// the background question the highlight is the answer, so it chooses.
    fn set_cursor(&mut self, at: usize, cx: &mut Context<Self>) {
        let cursor = match &mut self.page {
            Page::Settings(s) => &mut s.cursor,
            Page::Onboarding(o) => {
                if o.step == Step::Background {
                    o.background = at == 0;
                }
                &mut o.cursor
            }
            Page::Search => return,
        };
        if *cursor != at {
            *cursor = at;
            cx.notify();
        }
    }

    /// One selectable line. Hovering tints it, clicking moves the highlight
    /// here and then does what `key` would, if anything. Rows where a click
    /// could cost something (clearing the thumbnails) pass no key and keep
    /// their action on a button of its own.
    fn row(
        &self,
        ix: usize,
        selected: bool,
        theme: Theme,
        cx: &mut Context<Self>,
        key: Option<Key>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(("row", ix))
            .group(row_group(ix))
            .min_h(px(52.))
            .py_2()
            .px(px(14.))
            .flex()
            .items_center()
            .gap_3()
            .rounded(px(10.))
            .when(selected, |r| r.bg(theme.hover_wash))
            // Half the highlight, so the row under the mouse never reads as
            // the one enter would act on.
            .when(!selected, |r| {
                r.hover(move |s| s.bg(theme.hover_wash.opacity(0.5)))
            })
            .cursor(CursorStyle::PointingHand)
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.set_cursor(ix, cx);
                match key {
                    Some(key) => this.panel_key(key, window, cx),
                    None => this.stop_recording(cx),
                }
            }))
    }

    fn pick_theme(&mut self, choice: ThemeChoice, window: &mut Window, cx: &mut Context<Self>) {
        let mut config = self.current_config();
        if config.theme != choice {
            config.theme = choice;
            self.set_theme(config, window, cx);
        }
    }

    pub(super) fn render_settings(
        &mut self,
        theme: Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Page::Settings(s) = &self.page else {
            return div().into_any_element();
        };
        let rows = s.rows();
        let cursor = s.cursor;
        let config = s.config.clone();
        let shots = s.shots.clone();
        let service = s.service;
        let sizes = s.sizes;

        let recording = s.recording;
        let scroll = s.scroll.clone();
        let reveal = s.reveal;

        // Rows are direct children of the scrolling column, so the one under
        // the cursor can be scrolled into view by its place.
        let mut list: Vec<AnyElement> = Vec::new();
        let mut cursor_child = 0;
        let section = |title: &'static str| {
            div()
                .px(px(14.))
                .pt(px(18.))
                .pb(px(4.))
                .text_xs()
                .text_color(theme.muted)
                .child(title)
                .into_any_element()
        };

        list.push(section("folders it reads"));
        for (ix, row) in rows.iter().enumerate() {
            let selected = ix == cursor;
            let el = match *row {
                Row::Folder(i) => {
                    let n = shots.get(i).copied().unwrap_or(0);
                    self.row(ix, selected, theme, cx, None)
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(short_path(&tidy(&config.folders[i]))),
                        )
                        .child(div().flex_none().text_sm().text_color(theme.muted).child(
                            if n == 1 {
                                "1 shot".to_string()
                            } else {
                                format!("{} shots", thousands(n))
                            },
                        ))
                        .child(on_row_hover(
                            ix,
                            selected,
                            button(
                                ("remove", i),
                                "del",
                                "remove",
                                theme,
                                // The row around it has a click of its own,
                                // which would then act on whatever row took
                                // this one's place.
                                cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    cx.stop_propagation();
                                    this.remove_folder(i, cx)
                                }),
                            ),
                        ))
                        .into_any_element()
                }
                Row::AddFolder => self
                    .row(ix, selected, theme, cx, Some(Key::Enter))
                    .child(
                        div()
                            .flex_1()
                            .text_color(theme.muted)
                            .child("add a folder\u{2026}"),
                    )
                    .child(on_row_hover(
                        ix,
                        selected,
                        hint(
                            keys::pretty(keys::fixed("add_folder", "ctrl-o")),
                            "add",
                            theme,
                        ),
                    ))
                    .into_any_element(),
                Row::Theme => {
                    list.push(section("look"));
                    self.row(ix, selected, theme, cx, Some(Key::Right))
                        .child(div().flex_1().child("theme"))
                        .child(segmented(config.theme, theme, cx))
                        .into_any_element()
                }
                Row::Similar => {
                    let detail: SharedString = format!(
                        "near-identical shots taken close together show as one, {} shows the rest",
                        keys::shown("similar", cx)
                    )
                    .into();
                    self.row(ix, selected, theme, cx, Some(Key::Enter))
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .child("group similar screenshots")
                                .child(div().text_xs().text_color(theme.muted).child(detail)),
                        )
                        .child(switch("similar", config.group_similar, theme))
                        .into_any_element()
                }
                Row::Background => {
                    list.push(section("reading"));
                    let on = service == Service::Running;
                    let status = platform::background_status(service, self.searchable);
                    self.row(ix, selected, theme, cx, Some(Key::Enter))
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .child(platform::WORDS.background_setting)
                                .child(div().text_xs().text_color(theme.muted).child(status)),
                        )
                        .child(switch("background", on, theme))
                        .into_any_element()
                }
                Row::Clipboard => {
                    let detail: SharedString = match config.clipboard_folder() {
                        Some(folder) => format!("saved in {}", tidy(&folder)).into(),
                        None => {
                            "keeps images you copy but never save, so they're searchable too".into()
                        }
                    };
                    self.row(ix, selected, theme, cx, Some(Key::Enter))
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .child("save copied images")
                                .child(div().text_xs().text_color(theme.muted).child(detail)),
                        )
                        .child(switch("clipboard", config.clipboard, theme))
                        .into_any_element()
                }
                Row::Script(i) => {
                    let script = Script::ALL[i];
                    let on = config.scripts().contains(&script);
                    let (name, detail) = script_words(script, on);
                    self.row(ix, selected, theme, cx, Some(Key::Enter))
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .child(name)
                                .child(div().text_xs().text_color(theme.muted).child(detail)),
                        )
                        .child(switch(("script", i), on, theme))
                        .into_any_element()
                }
                Row::Threads => {
                    self.row(ix, selected, theme, cx, None)
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .child("cores per screenshot")
                                .child(div().text_xs().text_color(theme.muted).child(
                                    "more reads each one faster, fewer leaves more for you",
                                )),
                        )
                        .child(stepper(config.threads, theme, cx))
                        .into_any_element()
                }
                Row::ClearThumbs => {
                    list.push(section("storage"));
                    let text: SharedString = match sizes {
                        Some((index, thumbs)) => {
                            format!("text {}, thumbnails {}", mb(index), mb(thumbs)).into()
                        }
                        None => "measuring\u{2026}".into(),
                    };
                    // A click on the row only highlights it: clearing takes
                    // the button, or enter once the highlight is here.
                    self.row(ix, selected, theme, cx, None)
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .child("clear thumbnails")
                                .child(div().text_xs().text_color(theme.muted).child(text)),
                        )
                        .child(button(
                            "clear-thumbs",
                            "enter",
                            "clear",
                            theme,
                            cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.set_cursor(ix, cx);
                                this.clear_thumbnails(cx);
                            }),
                        ))
                        .into_any_element()
                }
                Row::Support | Row::Updates => {
                    let (title, detail) = if matches!(*row, Row::Support) {
                        list.push(section("gyotaku"));
                        (
                            "support gyotaku",
                            "free and stays free, sponsoring keeps it maintained",
                        )
                    } else {
                        (
                            "get updates",
                            "new releases and features by email, signed up on gyotaku.app",
                        )
                    };
                    self.row(ix, selected, theme, cx, Some(Key::Enter))
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .child(title)
                                .child(div().text_xs().text_color(theme.muted).child(detail)),
                        )
                        .child(on_row_hover(ix, selected, hint("enter", "open", theme)))
                        .into_any_element()
                }
                Row::Shortcut(k) => {
                    if k == 0 {
                        list.push(section("shortcuts"));
                    }
                    let shortcut = &SHORTCUTS[k];
                    let bound = keys::current(k, &config.keys);
                    let changed = bound != keys::default(shortcut);
                    let listening = recording == Some(k);
                    let reset = changed.then(|| {
                        button(
                            ("reset", k),
                            "del",
                            "reset",
                            theme,
                            cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.stop_recording(cx);
                                this.reset_shortcut(k, cx);
                            }),
                        )
                    });
                    // While it listens, the way out and the way back to the
                    // default are buttons as well as keys. Otherwise reset
                    // shows on the highlighted or hovered row only.
                    let actions = if listening {
                        Some(
                            div()
                                .flex()
                                .gap_1()
                                .child(button(
                                    ("cancel-keys", k),
                                    "esc",
                                    "cancel",
                                    theme,
                                    cx.listener(|this, _: &ClickEvent, _, cx| {
                                        cx.stop_propagation();
                                        this.stop_recording(cx);
                                    }),
                                ))
                                .children(reset)
                                .into_any_element(),
                        )
                    } else {
                        reset.map(|r| on_row_hover(ix, selected, r).into_any_element())
                    };
                    self.row(ix, selected, theme, cx, Some(Key::Enter))
                        .child(div().flex_1().flex().flex_col().child(shortcut.label).when(
                            changed,
                            |d| {
                                d.child(div().text_xs().text_color(theme.muted).child(format!(
                                    "changed from {}",
                                    keys::pretty(keys::default(shortcut))
                                )))
                            },
                        ))
                        .children(actions)
                        .child(keycap(k, &bound, listening, theme))
                        .into_any_element()
                }
            };
            if ix == cursor {
                cursor_child = list.len();
            }
            list.push(el);
        }
        // The keys that don't change, listed so this is the one place to
        // look any of them up.
        list.push(
            div()
                .px(px(14.))
                .pt_3()
                .flex()
                .flex_wrap()
                .gap_x_5()
                .gap_y_2()
                .text_color(theme.muted)
                .children(keys::FIXED.iter().map(|(k, what)| hint(*k, what, theme)))
                .into_any_element(),
        );

        // The first frame hasn't been laid out yet, so there's no telling
        // whether a scrollbar is needed until the next one.
        if scroll.bounds().size.height <= px(0.) {
            window.request_animation_frame();
        }
        if reveal {
            scroll.scroll_to_item(cursor_child);
            // The scroll happens while this frame lays out, after the
            // scrollbar below was placed, so one more frame puts it right.
            window.request_animation_frame();
            if let Page::Settings(s) = &mut self.page {
                s.reveal = false;
            }
        }
        let footer = if recording.is_some() {
            div()
                .flex()
                .items_center()
                .gap_5()
                .child(div().child("press the new keys"))
                .child(button(
                    "footer-cancel",
                    "esc",
                    "cancel",
                    theme,
                    cx.listener(|this, _: &ClickEvent, _, cx| this.stop_recording(cx)),
                ))
        } else {
            div()
                .flex()
                .gap_5()
                .child(hint("\u{2191} \u{2193}", "move", theme))
                .child(hint("\u{2190} \u{2192}", "change", theme))
                .child(div().child(format!("gyotaku {}", env!("CARGO_PKG_VERSION"))))
        };

        div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_none()
                    .h(px(super::HEADER))
                    .px(px(super::PAD + 8.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(theme.hairline)
                    .child(div().text_size(px(21.)).child("settings"))
                    .child(button(
                        "settings-back",
                        "esc",
                        "back",
                        theme,
                        cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.leave_panel(window, cx)
                        }),
                    )),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h(px(0.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("settings")
                            .track_scroll(&scroll)
                            .flex_1()
                            .min_h(px(0.))
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_1()
                            .pb_6()
                            .on_scroll_wheel(cx.listener(|_, _, _, cx| cx.notify()))
                            .children(
                                list.into_iter()
                                    .map(|el| div().w(px(620.)).max_w_full().px_4().child(el)),
                            ),
                    )
                    .children(scrollbar(&scroll, theme)),
            )
            .child(
                div()
                    .flex_none()
                    .h(px(44.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .text_color(theme.faint)
                    .child(footer),
            )
            .into_any_element()
    }

    pub(super) fn render_onboarding(&mut self, theme: Theme, cx: &mut Context<Self>) -> AnyElement {
        let Page::Onboarding(o) = &self.page else {
            return div().into_any_element();
        };
        let step = o.step;
        let cursor = o.cursor;

        let (title, body, rows): (&str, &str, Vec<AnyElement>) = match step {
            Step::Folders => {
                let picks: Vec<(PathBuf, String, Option<Counts>, bool)> = o
                    .picks
                    .iter()
                    .map(|p| {
                        (
                            p.candidate.path.clone(),
                            p.candidate.why.clone(),
                            p.count,
                            p.on,
                        )
                    })
                    .collect();
                let add_ix = picks.len();
                let mut rows: Vec<AnyElement> = picks
                    .into_iter()
                    .enumerate()
                    .map(|(ix, (path, why, count, on))| {
                        let count = match count.map(|c| c.images) {
                            None => "counting\u{2026}".to_string(),
                            Some(COUNT_CAP..) => format!("{}+ images", thousands(COUNT_CAP)),
                            Some(1) => "1 image".into(),
                            Some(n) => format!("{} images", thousands(n)),
                        };
                        self.row(ix, ix == cursor, theme, cx, Some(Key::Space))
                            .child(switch(("pick", ix), on, theme))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .text_ellipsis()
                                            .child(short_path(&tidy(&path))),
                                    )
                                    .child(div().text_xs().text_color(theme.muted).child(why)),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_sm()
                                    .text_color(theme.muted)
                                    .child(count),
                            )
                            .into_any_element()
                    })
                    .collect();
                rows.push(
                    self.row(add_ix, cursor == add_ix, theme, cx, Some(Key::Space))
                        .child(div().w(px(30.)))
                        .child(
                            div()
                                .flex_1()
                                .text_color(theme.muted)
                                .child("add another folder\u{2026}"),
                        )
                        .child(hint(
                            keys::pretty(keys::fixed("add_folder", "ctrl-o")),
                            "add",
                            theme,
                        ))
                        .into_any_element(),
                );
                (
                    "where do your screenshots go?",
                    "gyotaku reads every image in the folders you pick, subfolders too, and keeps up with new ones.",
                    rows,
                )
            }
            Step::Background => {
                let words = &platform::WORDS;
                let options = [words.background_yes, words.background_no];
                let rows =
                    options
                        .iter()
                        .enumerate()
                        .map(|(ix, (label, detail))| {
                            self.row(ix, ix == cursor, theme, cx, None)
                                .child(radio(ix == cursor, theme))
                                .child(
                                    div().flex_1().flex().flex_col().child(*label).child(
                                        div().text_xs().text_color(theme.muted).child(*detail),
                                    ),
                                )
                                .into_any_element()
                        })
                        .collect();
                (words.background_title, words.background_body, rows)
            }
        };

        // Secondary actions on the left as quiet buttons, the one way
        // forward on the right in ink. Enter is that button and nothing else.
        let footer = match step {
            Step::Folders => div()
                .flex()
                .items_center()
                .gap_2()
                .child(hint("space", "pick", theme))
                .child(div().w(px(12.)))
                .child(button(
                    "onboarding-add",
                    keys::pretty(keys::fixed("add_folder", "ctrl-o")),
                    "add a folder",
                    theme,
                    cx.listener(|this, _: &ClickEvent, window, cx| this.add_folders(window, cx)),
                ))
                .child(div().w(px(8.)))
                .child(primary(
                    "onboarding-continue",
                    "continue",
                    "\u{21b5}",
                    theme,
                    cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.panel_key(Key::Enter, window, cx)
                    }),
                )),
            Step::Background => div()
                .flex()
                .items_center()
                .gap_2()
                .child(button(
                    "onboarding-back",
                    "esc",
                    "back",
                    theme,
                    cx.listener(|this, _: &ClickEvent, window, cx| this.panel_back(window, cx)),
                ))
                .child(div().w(px(8.)))
                .child(primary(
                    "onboarding-finish",
                    "done",
                    "\u{21b5}",
                    theme,
                    cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.panel_key(Key::Enter, window, cx)
                    }),
                )),
        };

        div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .id("onboarding")
                    .flex_1()
                    .min_h(px(0.))
                    .w(px(620.))
                    .max_w_full()
                    .px_4()
                    .pt(px(56.))
                    .overflow_y_scroll()
                    .child(div().px(px(14.)).text_xs().text_color(theme.muted).child(
                        if step == Step::Folders {
                            "1 of 2"
                        } else {
                            "2 of 2"
                        },
                    ))
                    .child(
                        div()
                            .px(px(14.))
                            .pt_2()
                            .text_size(px(26.))
                            .line_height(px(34.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(title),
                    )
                    .child(
                        div()
                            .px(px(14.))
                            .pt_2()
                            .pb_6()
                            .text_color(theme.muted)
                            .child(body),
                    )
                    .child(div().flex().flex_col().gap_1().children(rows)),
            )
            .child(
                div()
                    .flex_none()
                    .h(px(56.))
                    .flex()
                    .items_center()
                    .child(footer),
            )
            .into_any_element()
    }
}

/// A script's settings row: its name, and what turning it on does. Screenshots
/// already read aren't read again, that would be the whole library over.
fn script_words(script: Script, on: bool) -> (&'static str, &'static str) {
    match (script, on) {
        (Script::Devanagari, false) => (
            "read Devanagari",
            "Hindi, Marathi, Nepali and more, an 8 MB download",
        ),
        (Script::Devanagari, true) => (
            "read Devanagari",
            "on for new screenshots, ones read before stay as they were",
        ),
    }
}

/// The hover group of one row, so a control in it can show while the row
/// is hovered as well as while it's highlighted.
fn row_group(ix: usize) -> SharedString {
    format!("row-{ix}").into()
}

/// A control that belongs to one row and only shows on that row: while
/// it's highlighted, or the mouse is over it.
fn on_row_hover(ix: usize, selected: bool, child: impl IntoElement) -> impl IntoElement {
    div()
        .opacity(if selected { 1.0 } else { 0.0 })
        .group_hover(row_group(ix), |s| s.opacity(1.0))
        .child(child)
}

/// The one way forward on an onboarding step: ink, with the key that does
/// the same, the way Raycast labels its primary action.
fn primary(
    id: &'static str,
    label: &'static str,
    keys: &'static str,
    theme: Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_2()
        .h(px(32.))
        .pl_3()
        .pr(px(6.))
        .rounded(px(8.))
        .bg(theme.text)
        .text_color(theme.panel)
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .cursor(CursorStyle::PointingHand)
        .hover(|s| s.opacity(0.9))
        .active(|s| s.opacity(0.75))
        .on_click(on_click)
        .child(label)
        .child(
            div()
                .px(px(6.))
                .rounded(px(5.))
                .border_1()
                .border_color(theme.panel)
                .text_xs()
                .opacity(0.7)
                .child(keys),
        )
}

fn mb(bytes: u64) -> String {
    format!("{} MB", (bytes as f64 / 1_048_576.0).round() as u64)
}

/// Longer paths lose folders from the middle, never the start (where it
/// lives) or the end (which folder it is): `~/Downloads/College/…/Pictures`.
/// The row still ellipsizes as a last resort in a very narrow window.
const PATH_CHARS: usize = 44;

fn short_path(path: &str) -> String {
    if path.chars().count() <= PATH_CHARS {
        return path.to_string();
    }
    let parts: Vec<&str> = path.split('/').collect();
    let width = |head: usize, tail: usize| {
        parts[..head].join("/").chars().count()
            + parts[parts.len() - tail..].join("/").chars().count()
            + 3
    };
    // As many leading folders as fit, keeping at least the first one and the
    // last one, then as many trailing ones as still fit.
    let (mut head, mut tail) = (1, 1);
    while head + tail < parts.len() - 1 && width(head + 1, tail) <= PATH_CHARS {
        head += 1;
    }
    while head + tail < parts.len() - 1 && width(head, tail + 1) <= PATH_CHARS {
        tail += 1;
    }
    if head + tail >= parts.len() {
        return path.to_string();
    }
    format!(
        "{}/\u{2026}/{}",
        parts[..head].join("/"),
        parts[parts.len() - tail..].join("/")
    )
}

/// On is ink, off is a grey groove, and the knob slides between the two.
fn switch(id: impl Into<ElementId>, on: bool, theme: Theme) -> impl IntoElement {
    let (from, to) = if on { (2.0, 14.0) } else { (14.0, 2.0) };
    let id: ElementId = id.into();
    div()
        .flex_none()
        .relative()
        .w(px(30.))
        .h(px(18.))
        .rounded_full()
        .bg(if on { theme.text } else { theme.track })
        .child(
            div()
                .absolute()
                .top(px(2.))
                .size(px(14.))
                .rounded_full()
                .bg(if on { theme.panel } else { theme.muted })
                .with_animation(
                    ElementId::Name(format!("{id:?}-{on}").into()),
                    Animation::new(Duration::from_millis(140)).with_easing(ease_out_quint()),
                    move |knob, t| knob.left(px(from + (to - from) * t)),
                ),
        )
}

/// Only there when there's more than fits, so its being there at all says
/// "keep scrolling". Sized and placed like any scrollbar: the thumb is the
/// share of the page on screen, at where the screen is.
fn scrollbar(handle: &ScrollHandle, theme: Theme) -> Option<impl IntoElement + use<>> {
    let max = handle.max_offset().y.as_f32();
    let height = handle.bounds().size.height.as_f32();
    if max <= 1.0 || height <= 0.0 {
        return None;
    }
    let offset = (-handle.offset().y.as_f32()).clamp(0.0, max);
    let pad = 6.0;
    let track = height - pad * 2.0;
    let thumb = (track * height / (height + max)).max(32.0);
    let top = pad + (track - thumb) * offset / max;
    Some(
        div()
            .absolute()
            .right(px(6.))
            .top(px(top))
            .w(px(5.))
            .h(px(thumb))
            .rounded_full()
            .bg(theme.faint),
    )
}

/// A shortcut's keys. While it listens for new ones it says so, outlined
/// in ink, and whatever it shows fades in when it changes.
fn keycap(k: usize, bound: &str, listening: bool, theme: Theme) -> impl IntoElement {
    let text: SharedString = if listening {
        "press keys\u{2026}".into()
    } else {
        keys::pretty(bound)
    };
    div()
        .flex_none()
        .px(px(8.))
        .py(px(3.))
        .rounded(px(6.))
        .text_sm()
        .bg(theme.keycap)
        .border_1()
        .border_color(if listening {
            theme.text
        } else {
            theme.hairline
        })
        .text_color(if listening { theme.muted } else { theme.text })
        .child(text.clone())
        .with_animation(
            ElementId::Name(format!("cap-{k}-{text}").into()),
            Animation::new(Duration::from_millis(120)).with_easing(ease_out_quint()),
            |el, t| el.opacity(0.5 + 0.5 * t),
        )
}

fn radio(on: bool, theme: Theme) -> impl IntoElement {
    div()
        .flex_none()
        .size(px(16.))
        .rounded_full()
        .border_1()
        .border_color(if on { theme.text } else { theme.faint })
        .flex()
        .items_center()
        .justify_center()
        .when(on, |r| {
            r.child(div().size(px(8.)).rounded_full().bg(theme.text))
        })
}

/// The three themes, each one clickable. A click picks that theme straight
/// away rather than stepping to the next, and stops at the option so the
/// row's own click (which steps) doesn't follow it.
fn segmented(selected: ThemeChoice, theme: Theme, cx: &mut Context<Gyotaku>) -> impl IntoElement {
    let options = [
        ("system", ThemeChoice::System),
        ("light", ThemeChoice::Light),
        ("dark", ThemeChoice::Dark),
    ];
    // The pill sits 3 px inside the track, so its radius is the track's minus 3.
    div()
        .flex()
        .p(px(3.))
        .rounded(px(9.))
        .bg(theme.keycap)
        .children(options.into_iter().map(|(label, choice)| {
            let on = choice == selected;
            div()
                .id(label)
                .px_3()
                .py(px(3.))
                .rounded(px(6.))
                .text_sm()
                .cursor(CursorStyle::PointingHand)
                .when(on, |s| s.bg(theme.text).text_color(theme.panel))
                .when(!on, |s| {
                    s.text_color(theme.muted)
                        .hover(move |s| s.text_color(theme.text))
                })
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    cx.stop_propagation();
                    this.pick_theme(choice, window, cx);
                }))
                .child(label)
        }))
}

/// Fewer and more cores, as two buttons around the number. Clicking the row
/// itself only highlights it, so a stray click never changes the count.
fn stepper(value: usize, theme: Theme, cx: &mut Context<Gyotaku>) -> impl IntoElement {
    fn step(
        id: &'static str,
        keys: &'static str,
        by: isize,
        theme: Theme,
        cx: &mut Context<Gyotaku>,
    ) -> AnyElement {
        button(
            id,
            keys,
            "",
            theme,
            cx.listener(move |this, _: &ClickEvent, _, cx| {
                cx.stop_propagation();
                this.change_threads(by, cx);
            }),
        )
        .into_any_element()
    }
    div()
        .flex()
        .items_center()
        .gap_1()
        .child(step("threads-less", "\u{2190}", -1, theme, cx))
        .child(
            div()
                .w(px(20.))
                .flex()
                .justify_center()
                .child(value.to_string()),
        )
        .child(step("threads-more", "\u{2192}", 1, theme, cx))
}

#[cfg(test)]
mod short_path_tests {
    use super::short_path;

    #[test]
    fn short_paths_stay_whole() {
        assert_eq!(
            short_path("~/Pictures/Screenshots"),
            "~/Pictures/Screenshots"
        );
    }

    #[test]
    fn long_paths_lose_the_middle() {
        let s = short_path("~/Downloads/College/Cybersecurity/HTB/Linux/Paperwork/Pictures");
        assert!(s.starts_with("~/Downloads"), "{s}");
        assert!(s.ends_with("/Pictures"), "{s}");
        assert!(s.contains('\u{2026}'), "{s}");
        assert!(s.chars().count() <= super::PATH_CHARS, "{s}");
    }

    #[test]
    fn one_long_folder_name_is_left_for_the_ellipsis() {
        let name = "a".repeat(60);
        let s = short_path(&format!("~/{name}"));
        assert_eq!(s, format!("~/{name}"));
    }
}
