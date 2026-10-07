//! Windows: a borderless popup above everything, summoned by a global hotkey
//! the app registers itself, started at sign-in from the Run key.
//!
//! The reader (`gyotaku.exe watch`) is started by the app, hidden, whenever
//! the app runs and isn't already reading. It holds a lock file while it
//! runs, which is how the app knows.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write as _;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::os::windows::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use futures::channel::mpsc::UnboundedSender;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use gpui::{
    App, Bounds, ClipboardItem, Entity, Global, Size, Window, WindowBackgroundAppearance,
    WindowBounds, WindowHandle, WindowKind, WindowOptions,
};

use super::{Service, TrayCommand, Words};
use crate::app::Gyotaku;

pub const WORDS: Words = Words {
    background_setting: "start with windows",
    background_title: "start with windows?",
    background_body: "so it's ready the moment you need it, and a screenshot you take now is searchable a second later.",
    background_yes: (
        "yes, start with windows",
        "it waits hidden after you sign in, ready for alt shift s, and reads each new screenshot about a second after you take it",
    ),
    background_no: (
        "no, only when i open it",
        "open it from the Start menu, it reads new screenshots while it's running",
    ),
    chose_no: "reading your screenshots, they show up here as they're read",
    default_folder: "where Windows saves screenshots",
    copy_image_failed: "couldn't copy the image",
};

// Staying resident. std has no unix sockets on Windows, so the running copy
// listens on a loopback port and leaves the number in a file. The worst
// anyone else on the machine could do with it is open or close the window.

pub struct Listener {
    tcp: TcpListener,
    /// Held while this process lives, so a second one starting at the same
    /// moment can't also become the resident.
    _lock: File,
}

impl Listener {
    pub fn incoming(&self) -> std::net::Incoming<'_> {
        self.tcp.incoming()
    }
}

pub fn resident_address() -> PathBuf {
    gyotaku_core::data_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("resident.port")
}

pub fn wake(port_file: &Path) -> bool {
    let Some(port) = std::fs::read_to_string(port_file)
        .ok()
        .and_then(|p| p.trim().parse::<u16>().ok())
    else {
        return false;
    };
    // This launch was started by the user (the Start menu, a shortcut), so
    // it's allowed to bring a window forward; the resident process that will
    // open the window isn't, unless it's handed that right first.
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(
            windows_sys::Win32::UI::WindowsAndMessaging::ASFW_ANY,
        );
    }
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    // Opening it from the Start menu always shows the window; only the
    // summon key, which the app handles itself, toggles.
    match TcpStream::connect_timeout(&addr, Duration::from_millis(300)) {
        Ok(mut stream) => stream.write_all(b"show\n").is_ok(),
        Err(_) => false,
    }
}

pub fn listen(port_file: &Path) -> Option<Listener> {
    let dir = port_file.parent()?;
    std::fs::create_dir_all(dir).ok()?;
    let lock = File::create(dir.join("resident.lock")).ok()?;
    lock.try_lock().ok()?;
    let tcp = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).ok()?;
    let port = tcp.local_addr().ok()?.port();
    std::fs::write(port_file, port.to_string()).ok()?;
    Some(Listener { tcp, _lock: lock })
}

// The window.

/// A borderless window that stays on top and out of the taskbar, centred on
/// the screen, the way a launcher sits.
pub fn open_launcher(
    size: Size<gpui::Pixels>,
    cx: &mut App,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<Gyotaku> + 'static,
) -> Option<WindowHandle<Gyotaku>> {
    let window = cx
        .open_window(
            WindowOptions {
                titlebar: None,
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size, cx))),
                app_id: Some("gyotaku".into()),
                window_background: WindowBackgroundAppearance::Transparent,
                kind: WindowKind::PopUp,
                ..Default::default()
            },
            build,
        )
        .ok()?;
    let _ = window.update(cx, |_, window, _| round_corners(window));
    Some(window)
}

/// Asks DWM for Windows 11's rounded window corners. A tool-window popup
/// isn't always given them by default, and then its border and shadow come
/// out square. Windows 10 doesn't know the attribute and keeps the square
/// frame, which the square panel matches too.
fn round_corners(window: &Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
    };
    // Spelled out: gpui's Window has an inherent `window_handle` of its own.
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    let preference = DWMWCP_ROUND;
    // SAFETY: the handle is this live window's, and the attribute's value is
    // a DWM_WINDOW_CORNER_PREFERENCE read from the pointer for its size.
    unsafe {
        DwmSetWindowAttribute(
            handle.hwnd.get() as _,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            (&raw const preference).cast(),
            size_of_val(&preference) as u32,
        );
    }
}

/// DWM frames the popup: on Windows 11 it rounds the window (asked for in
/// `round_corners`), clips it, and draws its border and shadow along that
/// shape; on Windows 10 the same frame is square. The panel fills the window
/// square either way, so the frame and the panel always agree.
pub const SYSTEM_FRAMES_WINDOW: bool = true;

/// A popup that stays on top of everything has to be put away when you
/// click elsewhere, like the Start menu.
pub fn hides_when_inactive() -> bool {
    true
}

/// Windows only lets the process the user just used bring a window forward.
/// The resident process opens the window on a knock from someone else (the
/// Start menu launch, which hands its right over in `wake`), so it asks
/// explicitly; gpui's app-level activate does nothing on Windows.
pub fn take_focus(window: &mut Window) {
    window.activate_window();
}

/// Win+S is Windows search and Win+Shift+S the Snipping Tool, so the
/// default stays clear of the Windows key.
const SUMMON: &str = "alt-shift-s";

/// Kept alive for as long as the app runs; dropping it unregisters the key.
struct Summon(#[allow(dead_code)] GlobalHotKeyManager);

impl Global for Summon {}

/// Windows has no way for the desktop to run a command on a key, so the
/// resident process registers the key itself, and each press sends the same
/// knock a second launch would. If another program holds the key, opening
/// gyotaku from the Start menu still works.
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

/// An icon in the notification area while it waits, the way Raycast and
/// the other launchers on Windows sit: a click opens the window, a right
/// click has Settings and Quit. The window itself is a tool window, so it
/// never puts a button on the taskbar.
pub fn settle_in(
    commands: UnboundedSender<TrayCommand>,
    keys: &BTreeMap<String, String>,
    cx: &mut App,
) {
    let Some(icon) = super::tray::icon(include_bytes!("../../../../assets/tray.png")) else {
        return;
    };
    let look = super::tray::Look {
        icon: tray_icon::TrayIconBuilder::new().with_icon(icon),
        click_opens: true,
    };
    let key = keys.get("summon").map_or(SUMMON, String::as_str);
    super::tray::show(look, key, commands, cx);
}

/// Nothing to do: the folder dialog comes up in front on its own.
pub fn before_picker(_: &mut App) {}

/// Nothing extra: Alt+F4 closes the window, and the process carries on
/// waiting for the next summon, since it only quits when asked to.
pub fn hide_keys() -> &'static [&'static str] {
    &[]
}

/// No override: the shared `ctrl` defaults in `keys.rs` stand.
pub fn default_key(_name: &str) -> Option<&'static str> {
    None
}

/// The platform modifier as hints spell it. Unchanged by this patch.
pub const MODIFIER_NAME: &str = "super";

// The clipboard. Windows keeps its own copy of whatever is put there, so it
// outlives the window without any help.

pub fn copy_text(text: &str, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(text.to_owned()));
}

/// Put on the clipboard twice over: as a PNG, which browsers, Discord and
/// newer apps take, and as a plain bitmap (CF_DIB), which is all that Paint,
/// Office and most older programs understand. gpui only writes the first.
pub fn copy_image(path: &Path, _: &mut App) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    let Ok(image) = image::load_from_memory(&bytes) else {
        return false;
    };
    let rgba = image.to_rgba8();
    let png = if image::guess_format(&bytes).is_ok_and(|f| f == image::ImageFormat::Png) {
        bytes
    } else {
        let mut png = Vec::new();
        if image
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .is_err()
        {
            return false;
        }
        png
    };
    clipboard::put(&[(clipboard::CF_DIB, dib(&rgba)), (clipboard::png(), png)])
}

/// A 32 bit, bottom-up device independent bitmap: a BITMAPINFOHEADER, then
/// the rows from the last one up, each pixel as blue, green, red, alpha.
fn dib(image: &image::RgbaImage) -> Vec<u8> {
    let (w, h) = image.dimensions();
    let mut out = Vec::with_capacity(40 + (w * h * 4) as usize);
    out.extend_from_slice(&40u32.to_le_bytes()); // header size
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes()); // positive: bottom-up
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB, uncompressed
    out.extend_from_slice(&(w * h * 4).to_le_bytes());
    out.extend_from_slice(&[0; 16]); // resolution and palette, unused
    for row in image.rows().rev() {
        for p in row {
            out.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
    }
    out
}

mod clipboard {
    use std::time::Duration;

    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock,
    };

    pub const CF_DIB: u32 = 8;

    /// The format id browsers and image apps agree on for PNG data.
    pub fn png() -> u32 {
        let name: Vec<u16> = "PNG\0".encode_utf16().collect();
        unsafe { RegisterClipboardFormatW(name.as_ptr()) }
    }

    /// Replaces the clipboard with these formats. Another program can hold
    /// the clipboard for a moment (clipboard managers do), so opening it is
    /// tried a few times before giving up.
    pub fn put(formats: &[(u32, Vec<u8>)]) -> bool {
        let opened = (0..10).any(|_| {
            let ok = unsafe { OpenClipboard(std::ptr::null_mut()) } != 0;
            if !ok {
                std::thread::sleep(Duration::from_millis(20));
            }
            ok
        });
        if !opened {
            return false;
        }
        let mut wrote = unsafe { EmptyClipboard() } != 0;
        for (format, bytes) in formats {
            wrote &= unsafe { set(*format, bytes) };
        }
        unsafe { CloseClipboard() };
        wrote
    }

    /// The clipboard owns the memory once it's handed over, so it's never
    /// freed here unless handing it over failed.
    unsafe fn set(format: u32, bytes: &[u8]) -> bool {
        unsafe {
            let memory = GlobalAlloc(GMEM_MOVEABLE, bytes.len());
            if memory.is_null() {
                return false;
            }
            let at = GlobalLock(memory) as *mut u8;
            if at.is_null() {
                windows_sys::Win32::Foundation::GlobalFree(memory);
                return false;
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), at, bytes.len());
            GlobalUnlock(memory);
            if SetClipboardData(format, memory).is_null() {
                windows_sys::Win32::Foundation::GlobalFree(memory);
                return false;
            }
            true
        }
    }
}

// Memory. The Windows heap hands freed pages back by itself.

pub fn tune_allocator() {}

pub fn release_memory() {}

// Reading in the background. The switch in settings is "start with Windows";
// new screenshots are read whenever gyotaku runs either way, because nobody
// on Windows is going to start a reader from a terminal.

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

pub fn background_status(service: Service, _searchable: usize) -> String {
    match service {
        Service::Running => "on, waits hidden after you sign in".into(),
        Service::Stopped => "off, open it from the Start menu".into(),
    }
}

pub fn service_status() -> Service {
    if starts_at_sign_in() {
        Service::Running
    } else {
        Service::Stopped
    }
}

pub fn start_service() -> bool {
    let Ok(app) = std::env::current_exe() else {
        return false;
    };
    let command = format!("\"{}\" --background", app.display());
    let registered = hidden("reg")
        .args([
            "add", RUN_KEY, "/v", "gyotaku", "/t", "REG_SZ", "/d", &command, "/f",
        ])
        .status()
        .is_ok_and(|s| s.success());
    start_reader();
    registered
}

pub fn stop_service() -> bool {
    hidden("reg")
        .args(["delete", RUN_KEY, "/v", "gyotaku", "/f"])
        .status()
        .is_ok_and(|s| s.success())
}

/// Brings the reader up if it isn't running, once there's a config saying
/// what to read: at every start of the app, and every time the window
/// opens, so a reader that died (no network on the first run, ended in Task
/// Manager) comes back without waiting for the next sign-in. Checking is a
/// lock probe; off the main thread, since starting a process takes a moment.
pub fn revive_reader() {
    std::thread::spawn(|| {
        if matches!(gyotaku_core::Config::load(), Ok(Some(_))) {
            start_reader();
        }
    });
}

/// Saying no to starting with Windows still reads while gyotaku is open.
pub fn keep_reading() {
    std::thread::spawn(start_reader);
}

fn start_reader() {
    if gyotaku_core::status::reader_running() {
        return;
    }
    let mut reader = hidden(&crate::setup::cli_path());
    reader
        .arg("watch")
        .creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
    // It runs with no console, so what it says goes to a file, kept short.
    if let Some(log) = reader_log() {
        if let Ok(err) = log.try_clone() {
            reader.stderr(err);
        }
        reader.stdout(log);
    }
    let _ = reader.spawn();
}

/// `watch.log` next to the index, started over once it passes 1 MB.
fn reader_log() -> Option<File> {
    let path = gyotaku_core::data_dir().ok()?.join("watch.log");
    let big = std::fs::metadata(&path).is_ok_and(|m| m.len() > 1 << 20);
    std::fs::OpenOptions::new()
        .create(true)
        .append(!big)
        .write(true)
        .truncate(big)
        .open(path)
        .ok()
}

/// reg and the reader are console programs. Started from a window app each
/// would flash a console up, so they get none.
fn hidden(program: &str) -> Command {
    let mut command = Command::new(program);
    command
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

fn starts_at_sign_in() -> bool {
    hidden("reg")
        .args(["query", RUN_KEY, "/v", "gyotaku"])
        .status()
        .is_ok_and(|s| s.success())
}

// Screenshot folders. Win+PrtScn and the Snipping Tool save to
// Pictures\Screenshots, which the shared defaults already offer.

/// Game Bar (Win+Alt+PrtScn, and what most PC games' capture key goes
/// through) and ShareX each have a folder of their own.
pub fn tool_folders(home: &Path) -> Vec<PathBuf> {
    let documents = directories::UserDirs::new()
        .and_then(|d| d.document_dir().map(Path::to_path_buf))
        .unwrap_or_else(|| home.join("Documents"));
    let videos = directories::UserDirs::new()
        .and_then(|d| d.video_dir().map(Path::to_path_buf))
        .unwrap_or_else(|| home.join("Videos"));
    vec![
        videos.join("Captures"),
        documents.join("ShareX").join("Screenshots"),
    ]
}
