// No console window behind the app on Windows.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod grid;
mod images;
mod input;
mod keys;
mod platform;
mod setup;
mod spring;
mod stats;
mod theme;

use std::borrow::Cow;

use anyhow::Result;
use futures::StreamExt as _;
use gpui::{
    App, AppContext, Bounds, DisplayId, Entity, Global, QuitMode, Size, TitlebarOptions,
    WindowBounds, WindowOptions, px, size,
};
use gpui_platform::application;
use gyotaku_core::Index;

use app::Gyotaku;

const FONTS: [&[u8]; 3] = [
    include_bytes!("../fonts/IBMPlexSans-Regular.ttf"),
    include_bytes!("../fonts/IBMPlexSans-Medium.ttf"),
    include_bytes!("../fonts/IBMPlexSans-SemiBold.ttf"),
];

fn main() -> Result<()> {
    // gpui reports window and gpu trouble through `log`, RUST_LOG=warn shows it.
    env_logger::Builder::from_default_env()
        .format_timestamp_millis()
        .init();
    let args: Vec<String> = std::env::args().collect();
    // --window skips the launcher window and opens an ordinary one, for
    // poking at it. --once exits when the window closes instead of staying
    // resident. --background starts resident with no window, which is how
    // it's started at sign-in on Windows, ready for the summon key.
    let windowed = args.iter().any(|a| a == "--window");
    let once = args.iter().any(|a| a == "--once");
    let background = args.iter().any(|a| a == "--background");

    let address = platform::resident_address();
    if !once && platform::wake(&address) {
        return Ok(());
    }
    let listener = if once {
        None
    } else {
        platform::listen(&address)
    };
    platform::tune_allocator();

    let quit_mode = if listener.is_some() {
        QuitMode::Explicit
    } else {
        QuitMode::LastWindowClosed
    };
    let app = application().with_quit_mode(quit_mode);
    // Opening the app again (Spotlight, Finder) while it waits brings the
    // window up, the same as the summon key.
    app.on_reopen(summon);
    app.run(move |cx: &mut App| {
        cx.text_system()
            .add_fonts(FONTS.iter().map(|f| Cow::Borrowed(*f)).collect())
            .expect("the bundled fonts load");
        let config = gyotaku_core::Config::load_or_default();
        keys::bind_all(cx, &config.keys);
        cx.on_action(|_: &HideWindow, cx| hide(cx));
        cx.set_global(Launch {
            windowed,
            resident: listener.is_some(),
        });
        // Settled in before the first window opens: becoming a menu bar
        // app on macOS while the window is up would deactivate it, and
        // the window puts itself away when it isn't active.
        if listener.is_some() {
            let (commands, mut picked) = futures::channel::mpsc::unbounded();
            platform::settle_in(commands, &config.keys, cx);
            cx.spawn(async move |cx| {
                while let Some(command) = picked.next().await {
                    cx.update(|cx| tray(command, cx));
                }
            })
            .detach();
        }
        if !(background && listener.is_some()) {
            toggle(cx);
        }
        platform::revive_reader();

        // The view lives on for next time, but most of its thumbnails
        // don't need to. Hand the freed pages back so an idle gyotaku
        // stays small.
        cx.on_window_closed(|cx, _| {
            if let Some(view) = cx.try_global::<Kept>().map(|k| k.0.clone()) {
                view.update(cx, |view, cx| view.hidden(cx));
            }
            platform::release_memory();
        })
        .detach();

        // A second launch, or the summon key where the app registers one
        // itself, knocks. The summon key toggles the window; a launch says
        // whether to toggle (Linux, where the desktop's shortcut is a
        // launch) or just show it (opening the app on macOS and Windows).
        let Some(listener) = listener else { return };
        let (knocks, mut knocked) = futures::channel::mpsc::unbounded();
        let (shows, mut shown) = futures::channel::mpsc::unbounded();
        platform::register_summon(knocks.clone(), &config.keys, cx);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let mut said = String::new();
                let _ =
                    std::io::BufRead::read_line(&mut std::io::BufReader::new(stream), &mut said);
                let sent = if said.trim() == "show" {
                    shows.unbounded_send(())
                } else {
                    knocks.unbounded_send(())
                };
                if sent.is_err() {
                    break;
                }
            }
        });
        cx.spawn(async move |cx| {
            while knocked.next().await.is_some() {
                cx.update(toggle);
            }
        })
        .detach();
        cx.spawn(async move |cx| {
            while shown.next().await.is_some() {
                cx.update(summon);
            }
        })
        .detach();
    });
    Ok(())
}

/// How this process was started, for the parts of the app that need to
/// close and reopen the window themselves.
struct Launch {
    windowed: bool,
    resident: bool,
}

impl Global for Launch {}

/// Whether closing the window leaves the process running.
pub(crate) fn is_resident(cx: &App) -> bool {
    cx.try_global::<Launch>().is_some_and(|l| l.resident)
}

/// Opens the search window, or closes it if it's already up, so one key
/// both summons and dismisses it.
fn toggle(cx: &mut App) {
    if let Some(open) = cx.windows().first().copied() {
        let _ = open.update(cx, |_, window, _| window.remove_window());
        return;
    }
    summon(cx);
}

gpui::actions!(gyotaku_app, [HideWindow]);

/// Puts the window away and keeps waiting, for the system's own close keys
/// (Cmd+W and Cmd+Q on macOS), so they never end the process by accident.
fn hide(cx: &mut App) {
    for window in cx.windows() {
        let _ = window.update(cx, |_, window, _| window.remove_window());
    }
}

/// What the menu bar or notification area icon asked for.
fn tray(command: platform::TrayCommand, cx: &mut App) {
    match command {
        platform::TrayCommand::Open => summon(cx),
        platform::TrayCommand::Settings => {
            summon(cx);
            if let Some(window) = cx.windows().first().copied() {
                let _ = window.update(cx, |_, window, cx| {
                    window.dispatch_action(Box::new(app::OpenSettings), cx);
                });
            }
        }
        platform::TrayCommand::Quit => cx.quit(),
    }
}

/// Opens the window if it isn't open: the system's launcher window where it
/// has one, an ordinary window otherwise.
pub(crate) fn summon(cx: &mut App) {
    if !cx.windows().is_empty() {
        return;
    }
    let windowed = cx.try_global::<Launch>().is_some_and(|l| l.windowed);
    // Settled once per summon, so the window is measured for the same screen
    // it opens on. Asking twice could read two different monitors.
    let display = platform::active_display();
    let size = window_size(display, cx);
    let window = if windowed {
        None
    } else {
        platform::open_launcher(display, size, cx, |window, cx| root(true, window, cx))
    };
    let window = window.unwrap_or_else(|| open_window(display, size, cx));
    platform::settle_position(window, display, cx);
    let _ = window.update(cx, |view, window, cx| {
        window.focus(&gpui::Focusable::focus_handle(view, cx), cx);
        cx.activate(true);
        platform::take_focus(window);
    });
    platform::revive_reader();
}

/// A generous palette, but never more than most of the screen: the one it is
/// about to open on, which can be the smaller of several.
fn window_size(display: Option<DisplayId>, cx: &App) -> Size<gpui::Pixels> {
    let screen = display
        .and_then(|id| cx.find_display(id))
        .or_else(|| cx.primary_display())
        .map(|d| d.bounds().size)
        .unwrap_or(size(px(1920.), px(1080.)));
    size(
        px(1180.).min(screen.width * 0.86),
        px(780.).min(screen.height * 0.84),
    )
}

/// The one view, kept across windows so every summon picks up where the
/// last one left off.
struct Kept(Entity<Gyotaku>);

impl Global for Kept {}

fn root(floating: bool, window: &mut gpui::Window, cx: &mut App) -> Entity<Gyotaku> {
    if let Some(view) = cx.try_global::<Kept>().map(|k| k.0.clone()) {
        view.update(cx, |view, cx| view.reopen(window, cx));
        return view;
    }
    let index = Index::open_default().expect("the index opens");
    let view = cx.new(|cx| Gyotaku::new(index, floating, window, cx));
    cx.set_global(Kept(view.clone()));
    view
}

fn open_window(
    display: Option<DisplayId>,
    size: Size<gpui::Pixels>,
    cx: &mut App,
) -> gpui::WindowHandle<Gyotaku> {
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(display, size, cx))),
            display_id: display,
            titlebar: Some(TitlebarOptions {
                title: Some("gyotaku".into()),
                ..Default::default()
            }),
            app_id: Some("gyotaku".into()),
            ..Default::default()
        },
        |window, cx| root(false, window, cx),
    )
    .expect("a window opens")
}
