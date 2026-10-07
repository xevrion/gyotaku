//! macOS: a borderless popup above everything, summoned by a global hotkey
//! the app registers itself, and the watcher as a launchd agent.
//!
//! The reader (`gyotaku watch`) is also started by the app, hidden, whenever
//! the app runs and isn't already reading. It holds a lock file while it
//! runs, which is how the app knows.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use futures::channel::mpsc::UnboundedSender;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use gpui::{
    App, Bounds, ClipboardItem, DisplayId, Entity, Global, Image, ImageFormat, Size, Window,
    WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind, WindowOptions,
};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

use super::{Service, TrayCommand, Words};
use crate::app::Gyotaku;

pub const WORDS: Words = Words {
    background_setting: "start at login",
    background_title: "start at login?",
    background_body: "so it's ready the moment you need it, and a screenshot you take now is searchable a second later.",
    background_yes: (
        "yes, start it now and at every login",
        "it waits hidden after you log in, ready for alt shift s, and reads each new screenshot about a second after you take it",
    ),
    background_no: (
        "no, only when i open it",
        "open it from Spotlight, it reads new screenshots while it's running",
    ),
    chose_no: "reading your screenshots, they show up here as they're read",
    default_folder: "the screenshots folder people commonly point Cmd Shift 5 at",
    copy_image_failed: "couldn't copy the image",
};

// Staying resident. macOS has unix sockets, and its TMPDIR is already one
// per user; tagging the name with the uid keeps it that way if TMPDIR is
// ever pointed somewhere shared.

pub type Listener = UnixListener;

pub fn resident_address() -> PathBuf {
    std::env::temp_dir().join(format!("gyotaku-{}.sock", unsafe { libc::getuid() }))
}

/// Asks a running instance to show its window. False if there isn't one.
/// Opening the app (Spotlight, Finder) always shows it, like Raycast; only
/// the summon key, which the app handles itself, toggles.
pub fn wake(socket: &Path) -> bool {
    match UnixStream::connect(socket) {
        Ok(mut stream) => stream.write_all(b"show\n").is_ok(),
        Err(_) => false,
    }
}

/// Becomes the running instance. None if another process won the race to
/// the socket, in which case this one just runs once and exits.
pub fn listen(socket: &Path) -> Option<Listener> {
    // Left behind by an instance that crashed or was killed, nobody answered
    // `wake` so nobody is using it.
    let _ = std::fs::remove_file(socket);
    UnixListener::bind(socket).ok()
}

// The window.

/// The monitor being worked on. Always `None` for now, which centres on the
/// primary one the way it always has. Following the active display here
/// needs the frontmost app's window or the cursor from CoreGraphics, neither
/// of which this crate has a dependency for yet.
pub fn active_display() -> Option<DisplayId> {
    None
}

/// Nothing to put right: a panel opens where it is told.
pub fn settle_position(_window: WindowHandle<Gyotaku>, _display: Option<DisplayId>, _cx: &mut App) {
}

/// A borderless panel that floats above everything, centred on `display`,
/// the way Spotlight sits. gpui makes a pop-up on macOS an NSPanel, so it
/// never takes focus from what you're working on.
pub fn open_launcher(
    display: Option<DisplayId>,
    size: Size<gpui::Pixels>,
    cx: &mut App,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<Gyotaku> + 'static,
) -> Option<WindowHandle<Gyotaku>> {
    cx.open_window(
        WindowOptions {
            titlebar: None,
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(display, size, cx))),
            display_id: display,
            app_id: Some("gyotaku".into()),
            window_background: WindowBackgroundAppearance::Transparent,
            kind: WindowKind::PopUp,
            ..Default::default()
        },
        build,
    )
    .ok()
}

/// gpui makes the popup a titled NSPanel with a full-size content view, so
/// AppKit rounds it at the system's own radius (which varies by macOS
/// version) and draws the rim and shadow along that curve. The panel fills
/// the window square and lets AppKit cut the corners.
pub const SYSTEM_FRAMES_WINDOW: bool = true;

/// A panel that floats above everything has to be put away when you click
/// elsewhere, like Spotlight.
pub fn hides_when_inactive() -> bool {
    true
}

/// Nothing to do on macOS: this panel is a non-activating NSPanel, so a
/// knock makes it the key window on its own, unlike Windows.
pub fn take_focus(_window: &mut Window) {}

/// Cmd is every app's own shortcut namespace, so the default stays on
/// Option+Shift, like opening Spotlight used to be.
const SUMMON: &str = "alt-shift-s";

/// Kept alive for as long as the app runs; dropping it unregisters the key.
struct Summon(#[allow(dead_code)] GlobalHotKeyManager);

impl Global for Summon {}

/// The resident process registers the key itself, and each press sends the
/// same knock a second launch would. If another program holds the key,
/// opening gyotaku from Spotlight still works.
pub fn register_summon(knocks: UnboundedSender<()>, keys: &BTreeMap<String, String>, cx: &mut App) {
    let key = keys.get("summon").map_or(SUMMON, String::as_str);
    let registered = (|| {
        let hotkey = key.replace('-', "+").parse::<HotKey>().ok()?;
        let manager = GlobalHotKeyManager::new().ok()?;
        manager.register(hotkey).ok()?;
        Some(manager)
    })();
    let Some(manager) = registered else {
        log::warn!("couldn't register {key}, another program may be using it");
        return;
    };
    GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
        if e.state == HotKeyState::Pressed {
            let _ = knocks.unbounded_send(());
        }
    }));
    cx.set_global(Summon(manager));
}

/// Becomes an accessory app, the way Raycast and Spotlight-style launchers
/// run: no Dock icon and no Cmd+Tab entry, so there's nothing to quit by
/// accident and the summon key always finds it waiting. gpui makes every app
/// a regular one as it finishes launching, which is before this runs, so
/// this has the last word. A menu bar icon takes the Dock icon's place.
pub fn settle_in(
    commands: UnboundedSender<TrayCommand>,
    keys: &BTreeMap<String, String>,
    cx: &mut App,
) {
    if let Some(main) = MainThreadMarker::new() {
        NSApplication::sharedApplication(main)
            .setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    }
    let Some(icon) = super::tray::icon(include_bytes!("../../../../assets/menubar.png")) else {
        return;
    };
    let look = super::tray::Look {
        icon: tray_icon::TrayIconBuilder::new().with_icon_templated(icon),
        click_opens: false,
    };
    let key = keys.get("summon").map_or(SUMMON, String::as_str);
    super::tray::show(look, key, commands, cx);
}

/// The window is a non-activating panel, so gyotaku is never the active app
/// even while you type in it, and the folder picker, which macOS shows for
/// the active app, would open behind whatever was in front. Activating first
/// puts it on top with the keyboard.
pub fn before_picker(cx: &mut App) {
    cx.activate(true);
}

/// Cmd+W and Cmd+Q put the window away like Escape does. Quitting for good
/// is in the menu bar icon's menu, where Raycast keeps it too.
pub fn hide_keys() -> &'static [&'static str] {
    &["cmd-w", "cmd-q"]
}

// The clipboard. The macOS pasteboard server keeps its own copy, so a copy
// outlives the window without any help.

pub fn copy_text(text: &str, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(text.to_owned()));
}

pub fn copy_image(path: &Path, cx: &mut App) -> bool {
    let format = match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jpg" | "jpeg") => ImageFormat::Jpeg,
        Some("webp") => ImageFormat::Webp,
        _ => ImageFormat::Png,
    };
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    cx.write_to_clipboard(ClipboardItem::new_image(&Image::from_bytes(format, bytes)));
    true
}

// Memory. The macOS allocator hands large freed blocks back to the kernel by
// itself, and asking for pressure relief drops what it can spare.

pub fn tune_allocator() {}

pub fn release_memory() {
    unsafe {
        malloc_zone_pressure_relief(malloc_default_zone(), 0);
    }
}

// libSystem defines these, and every binary links libSystem, so no #[link].
unsafe extern "C" {
    fn malloc_default_zone() -> *mut std::ffi::c_void;
    fn malloc_zone_pressure_relief(zone: *mut std::ffi::c_void, goal: usize) -> usize;
}

// Reading in the background: a launchd agent, started with the document
// based plist format launchd has always read. Every mac boots launchd.
//
// The agent restarts gyotaku only if it exits with an error, so a crash
// brings it back after 30 seconds while Quit from the menu bar icon stays
// quit until the next login, or until it's opened again.

const LABEL: &str = "io.github.xevrion.gyotaku.watch";

pub fn background_status(service: Service, _searchable: usize) -> String {
    match service {
        Service::Running => "on, waits hidden after you log in".into(),
        Service::Stopped => "off, open it from Spotlight".into(),
    }
}

/// Running if the agent is there to start gyotaku at next login. The reader
/// being up says nothing, since the app starts it whenever it is open.
pub fn service_status() -> Service {
    if plist().is_some_and(|p| p.exists()) {
        Service::Running
    } else {
        Service::Stopped
    }
}

/// Writes the agent and starts it now. bootstrap complains when the agent
/// is already loaded, so falling that back to a kickstart still starts it.
pub fn start_service() -> bool {
    let Ok(app) = std::env::current_exe() else {
        return false;
    };
    if install_plist(&app.to_string_lossy()).is_err() {
        return false;
    }
    let Some(plist) = plist() else { return false };
    let uid = unsafe { libc::getuid() };
    let domain = format!("gui/{uid}");
    launchctl(&["bootstrap", &domain, &plist.to_string_lossy()])
        || launchctl(&["kickstart", &format!("{domain}/{LABEL}")])
}

pub fn stop_service() -> bool {
    let uid = unsafe { libc::getuid() };
    let _ = launchctl(&["bootout", &format!("gui/{uid}/{LABEL}")]);
    if let Some(plist) = plist() {
        let _ = std::fs::remove_file(plist);
    }
    true
}

/// The app at start, and the panel at every knock, brings the reader up if
/// there is a config saying what to read, so a reader that died comes back.
/// Off the main thread, since starting a process takes a moment.
pub fn revive_reader() {
    std::thread::spawn(|| {
        if matches!(gyotaku_core::Config::load(), Ok(Some(_))) {
            start_reader();
        }
    });
}

/// Saying no to starting at login still reads while gyotaku is open.
pub fn keep_reading() {
    std::thread::spawn(start_reader);
}

fn start_reader() {
    if gyotaku_core::status::reader_running() {
        return;
    }
    // A child outlives its parent here with no ceremony; launchd adopts it.
    let _ = Command::new(crate::setup::cli_path())
        .arg("watch")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

fn plist() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|d| {
        d.home_dir()
            .join("Library/LaunchAgents")
            .join(format!("{LABEL}.plist"))
    })
}

fn launchctl(args: &[&str]) -> bool {
    Command::new("launchctl")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn install_plist(app: &str) -> std::io::Result<()> {
    let Some(plist) = plist() else {
        return Err(std::io::ErrorKind::NotFound.into());
    };
    // Rewritten when it differs, so an agent left over from an earlier
    // install points at this app instead of an old or
    // deleted one. Edits made with launchctl override live outside this
    // file, which this never touches.
    let wanted = plist_file(app);
    if std::fs::read_to_string(&plist).is_ok_and(|have| have == wanted) {
        return Ok(());
    }
    std::fs::create_dir_all(plist.parent().expect("has a parent"))?;
    std::fs::write(&plist, wanted)
}

fn plist_file(exec: &str) -> String {
    let exec = xml(exec);
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
         \t<key>Label</key><string>{LABEL}</string>\n\
         \t<key>ProgramArguments</key><array><string>{exec}</string><string>--background</string></array>\n\
         \t<key>RunAtLoad</key><true/>\n\
         \t<key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>\n\
         \t<key>ThrottleInterval</key><integer>30</integer>\n\
         \t<key>StandardOutPath</key><string>/dev/null</string>\n\
         \t<key>StandardErrorPath</key><string>/dev/null</string>\n\
         </dict>\n\
         </plist>\n"
    )
}

/// A path could hold one of the few characters an XML plist can't.
fn xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// Screenshot folders. macOS itself saves them to the Desktop, which the
// shared defaults already offer.

/// The folder each of the common mac screenshot tools uses by default, most
/// specific first. Tools nobody has installed simply never show up, since
/// only folders that exist are offered.
pub fn tool_folders(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join("Pictures/Shottr"),
        home.join("Pictures/CleanShot X"),
        home.join("Pictures/Shots"),
        home.join("Pictures/Monosnap"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_agent_starts_the_app_hidden() {
        let plist = plist_file("/x/gyotaku-app");
        assert!(plist.contains("<string>/x/gyotaku-app</string><string>--background</string>"));
        assert!(!plist.contains("<string>watch</string>"));
    }

    #[test]
    fn the_agent_keeps_quotes_out_of_paths() {
        let plist = plist_file("/a&b/gyotaku-app");
        assert!(plist.contains("/a&amp;b/gyotaku-app"));
    }

    #[test]
    fn mac_tools_each_have_a_folder() {
        let folders = tool_folders(Path::new("/home/me"));
        assert!(folders[0].ends_with("Pictures/Shottr"));
        assert!(folders[1].ends_with("Pictures/CleanShot X"));
        assert!(folders[2].ends_with("Pictures/Shots"));
        assert!(folders[3].ends_with("Pictures/Monosnap"));
    }
}
