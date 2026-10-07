//! Linux: a layer-shell overlay where the compositor has one, the desktop's
//! own shortcut to summon it, and the watcher as a systemd user service or an
//! XDG autostart entry.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use futures::channel::mpsc::UnboundedSender;
use gpui::{
    App, Bounds, ClipboardItem, DisplayId, Entity, Size, Window, WindowBackgroundAppearance,
    WindowBounds, WindowHandle, WindowKind, WindowOptions,
};

use super::{Service, TrayCommand, Words};
use crate::app::Gyotaku;

pub const WORDS: Words = Words {
    background_setting: "read new screenshots in the background",
    background_title: "keep reading new ones?",
    background_body: "so a screenshot you take now is searchable a second later.",
    background_yes: (
        "yes, start it now and at every login",
        "a small background process reads each new screenshot about a second after you take it, at idle priority",
    ),
    background_no: (
        "no, i'll run gyotaku watch myself",
        "nothing runs in the background",
    ),
    chose_no: "saved, run gyotaku watch to start reading",
    default_folder: "where most desktops save screenshots",
    copy_image_failed: "couldn't copy the image, is wl-copy installed?",
};

// Staying resident.

pub type Listener = UnixListener;

pub fn resident_address() -> PathBuf {
    match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(dir) => PathBuf::from(dir).join("gyotaku.sock"),
        None => std::env::temp_dir().join(format!("gyotaku-{}.sock", unsafe { libc::getuid() })),
    }
}

/// Asks a running instance to toggle its window. False if there isn't one.
pub fn wake(socket: &Path) -> bool {
    match UnixStream::connect(socket) {
        Ok(mut stream) => stream.write_all(b"toggle\n").is_ok(),
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

/// The monitor being worked on. Always `None`: the overlay is a layer shell
/// surface, and the compositor puts it on the output being used, which is
/// the answer this would be looking for. `LayerShellOptions` has no output
/// to name anyway. The ordinary window opened elsewhere gets the primary
/// monitor, as before.
pub fn active_display() -> Option<DisplayId> {
    None
}

/// Nothing to put right: the compositor places the overlay, and an ordinary
/// window opens where it is told.
pub fn settle_position(_window: WindowHandle<Gyotaku>, _display: Option<DisplayId>, _cx: &mut App) {
}

/// On Wayland compositors with layer shell (niri, sway, Hyprland, KDE) the
/// window floats above everything like a launcher, with no title bar and all
/// keyboard input going to it. None elsewhere (X11, GNOME), where an
/// ordinary window is opened instead.
pub fn open_launcher(
    _display: Option<DisplayId>,
    size: Size<gpui::Pixels>,
    cx: &mut App,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<Gyotaku> + 'static,
) -> Option<WindowHandle<Gyotaku>> {
    use gpui::layer_shell::*;
    std::env::var_os("WAYLAND_DISPLAY")?;
    cx.open_window(
        WindowOptions {
            titlebar: None,
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                Default::default(),
                size,
            ))),
            app_id: Some("gyotaku".into()),
            window_background: WindowBackgroundAppearance::Transparent,
            kind: WindowKind::LayerShell(LayerShellOptions {
                namespace: "gyotaku".into(),
                layer: Layer::Overlay,
                keyboard_interactivity: KeyboardInteractivity::Exclusive,
                ..Default::default()
            }),
            ..Default::default()
        },
        build,
    )
    .ok()
}

/// A layer-shell overlay has no frame, so the panel draws its own rounded
/// corners and outline.
pub const SYSTEM_FRAMES_WINDOW: bool = false;

/// The compositor puts an overlay away by itself when focus moves on.
pub fn hides_when_inactive() -> bool {
    false
}

/// The overlay asks for the keyboard as it opens, and an ordinary window
/// opened by a key press gets it. Asking again on Wayland only makes some
/// desktops flag the app as wanting attention.
pub fn take_focus(_: &mut Window) {}

/// Nothing to register: the desktop's own shortcut runs `gyotaku-app`, and
/// that knocks on the running copy.
pub fn register_summon(_: UnboundedSender<()>, _: &BTreeMap<String, String>, _: &mut App) {}

/// Nothing shows while it waits: there's no Dock or taskbar button to hide,
/// and the desktop's shortcut is the way back in. A tray icon would need a
/// StatusNotifier host most Wayland desktops don't run.
pub fn settle_in(_: UnboundedSender<TrayCommand>, _: &BTreeMap<String, String>, _: &mut App) {}

/// Nothing to do: the portal's folder picker comes up in front on its own.
pub fn before_picker(_: &mut App) {}

/// The window is an ordinary one or an overlay; the desktop's own close key
/// already just closes it, leaving the resident process waiting.
pub fn hide_keys() -> &'static [&'static str] {
    &[]
}

// The clipboard. A Wayland clipboard is served by the app that set it, so
// whatever gpui copies vanishes the moment this window closes. wl-copy forks
// a tiny process that keeps serving it, which is what makes "copy, esc,
// paste" work.

pub fn copy_text(text: &str, cx: &mut App) {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() && pipe("wl-copy", &[], text.as_bytes()) {
        return;
    }
    cx.write_to_clipboard(ClipboardItem::new_string(text.to_owned()));
}

pub fn copy_image(path: &Path, _: &mut App) -> bool {
    let mime = match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => "image/png",
    };
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        pipe("wl-copy", &["--type", mime], &bytes)
    } else {
        pipe("xclip", &["-selection", "clipboard", "-t", mime], &bytes)
    }
}

fn pipe(program: &str, args: &[&str], input: &[u8]) -> bool {
    let Ok(mut child) = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let wrote = child
        .stdin
        .take()
        .is_some_and(|mut stdin| stdin.write_all(input).is_ok());
    child.wait().is_ok_and(|s| s.success()) && wrote
}

// Memory.

/// A decoded thumbnail is ~0.5 MB. glibc normally raises its mmap threshold
/// after the first few of those are freed, and from then on puts them on the
/// heap, where scrolling through thousands of them fragments it for good.
/// Pinning the threshold keeps every image in its own mapping that goes back
/// to the kernel when freed: paging through 80 screens peaked at ~140 MB with
/// this and ~250 MB without.
pub fn tune_allocator() {
    #[cfg(target_env = "gnu")]
    unsafe {
        libc::mallopt(libc::M_MMAP_THRESHOLD, 128 * 1024);
    }
}

pub fn release_memory() {
    #[cfg(target_env = "gnu")]
    unsafe {
        libc::malloc_trim(0);
    }
}

// Reading in the background: a systemd user service where there's systemd,
// an XDG autostart entry anywhere else. Every desktop that follows the
// freedesktop specs runs those.

const SERVICE: &str = "gyotaku-watch.service";

pub fn background_status(service: Service, searchable: usize) -> String {
    match service {
        Service::Running => format!("running, {} searchable", crate::app::thousands(searchable)),
        Service::Stopped => "off, new screenshots won't be read".into(),
    }
}

/// Running if the service is up, or if a `gyotaku watch` started some other
/// way (by hand, by the autostart entry) is.
pub fn service_status() -> Service {
    let service = has_systemd() && systemctl(&["is-active", SERVICE]) == Some(true);
    if service || !watcher_pids().is_empty() {
        Service::Running
    } else {
        Service::Stopped
    }
}

/// Starts the watcher and makes it start again at every login.
pub fn start_service() -> bool {
    if has_systemd() {
        if install_unit().is_err() {
            return false;
        }
        let _ = systemctl(&["daemon-reload"]);
        return systemctl(&["enable", "--now", SERVICE]) == Some(true);
    }
    if install_autostart().is_err() {
        return false;
    }
    if watcher_pids().is_empty() {
        spawn_watcher()
    } else {
        true
    }
}

pub fn stop_service() -> bool {
    if let Some(entry) = autostart_path() {
        let _ = std::fs::remove_file(entry);
    }
    if has_systemd() {
        return systemctl(&["disable", "--now", SERVICE]) == Some(true);
    }
    for pid in watcher_pids() {
        unsafe { libc::kill(pid, libc::SIGTERM) };
    }
    true
}

/// The service, if there is one, was started at login and systemd restarts
/// it when it fails.
pub fn revive_reader() {}

/// Saying no in onboarding means no: `gyotaku watch` is run by hand.
pub fn keep_reading() {}

/// Whether there's a systemd user manager to hand the watcher to. The same
/// test sd_booted(3) does, plus the user instance's socket, since having the
/// systemctl binary around proves nothing (containers, systemd installed but
/// not running as init).
fn has_systemd() -> bool {
    Path::new("/run/systemd/system").is_dir()
        && std::env::var_os("XDG_RUNTIME_DIR")
            .is_some_and(|dir| Path::new(&dir).join("systemd/private").exists())
}

/// `setsid -f` forks the watcher off into its own session, so it isn't a
/// child of this window, doesn't die with it, and never lingers as a zombie.
/// Busybox's setsid has no -f, so failing that it's started directly.
fn spawn_watcher() -> bool {
    let quiet = |c: &mut Command| {
        c.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
    };
    let cli = crate::setup::cli_path();
    let mut detached = Command::new("setsid");
    detached.args(["-f", &cli, "watch"]);
    quiet(&mut detached);
    if detached.status().is_ok_and(|s| s.success()) {
        return true;
    }
    let mut direct = Command::new(&cli);
    direct.arg("watch");
    quiet(&mut direct);
    direct.spawn().is_ok()
}

/// Processes running `gyotaku watch`, found by reading /proc.
fn watcher_pids() -> Vec<i32> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let pid: i32 = entry.file_name().to_str()?.parse().ok()?;
            let cmdline = std::fs::read(entry.path().join("cmdline")).ok()?;
            let mut args = cmdline.split(|b| *b == 0);
            let program = Path::new(std::str::from_utf8(args.next()?).ok()?);
            let is_watcher = program.file_name()? == "gyotaku" && args.next()? == b"watch";
            is_watcher.then_some(pid)
        })
        .collect()
}

fn config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|d| d.home_dir().join(".config")))
}

fn autostart_path() -> Option<PathBuf> {
    Some(config_home()?.join("autostart/gyotaku-watch.desktop"))
}

fn install_autostart() -> std::io::Result<()> {
    let path = autostart_path().ok_or(std::io::ErrorKind::NotFound)?;
    std::fs::create_dir_all(path.parent().expect("has a parent"))?;
    std::fs::write(path, autostart_entry(&crate::setup::cli_path()))
}

fn autostart_entry(exec: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=gyotaku watcher\n\
         Comment=Reads new screenshots so they can be searched\n\
         Exec={exec} watch\n\
         NoDisplay=true\n\
         X-GNOME-Autostart-enabled=true\n"
    )
}

/// None if systemctl couldn't be run at all, otherwise whether it succeeded.
fn systemctl(args: &[&str]) -> Option<bool> {
    Command::new("systemctl")
        .arg("--user")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()
        .map(|s| s.success())
}

fn install_unit() -> std::io::Result<()> {
    let Some(home) = directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf()) else {
        return Err(std::io::ErrorKind::NotFound.into());
    };
    let unit = home.join(".config/systemd/user").join(SERVICE);
    // Rewritten when it differs, so a service left over from an earlier
    // install (built from source into ~/.cargo/bin, say) runs the gyotaku
    // next to this app instead of an old or deleted one. Local tweaks belong
    // in a drop-in (systemctl --user edit), which this never touches.
    let wanted = unit_file(&crate::setup::cli_path());
    if std::fs::read_to_string(&unit).is_ok_and(|have| have == wanted) {
        return Ok(());
    }
    std::fs::create_dir_all(unit.parent().expect("has a parent"))?;
    std::fs::write(&unit, wanted)
}

fn unit_file(exec: &str) -> String {
    format!(
        "[Unit]\n\
         Description=gyotaku, reads new screenshots so they can be searched\n\
         Documentation=https://github.com/xevrion/gyotaku\n\
         \n\
         [Service]\n\
         ExecStart={exec} watch\n\
         Restart=on-failure\n\
         RestartSec=30\n\
         Nice=19\n\
         CPUSchedulingPolicy=idle\n\
         IOSchedulingClass=idle\n\
         MemoryHigh=400M\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n"
    )
}

// Screenshot folders.

/// Where the common tools are told to save, most specific first.
pub fn tool_folders(home: &Path) -> Vec<PathBuf> {
    let config = config_home().unwrap_or_else(|| home.join(".config"));
    let read = |p: PathBuf| std::fs::read_to_string(p).unwrap_or_default();
    let env_dir = |var: &str| std::env::var(var).ok().map(|v| expand(&v, home));
    [
        env_dir("XDG_SCREENSHOTS_DIR"),
        niri_folder(&read(config.join("niri/config.kdl")), home),
        ini_value(&read(config.join("flameshot/flameshot.ini")), "savePath")
            .map(|v| expand(&v, home)),
        ini_value(&read(config.join("spectaclerc")), "imageSaveLocation")
            .map(|v| expand(v.trim_start_matches("file://"), home)),
        ini_value(&read(config.join("ksnip/ksnip.conf")), "SaveDirectory")
            .map(|v| expand(&v, home)),
        env_dir("GRIM_DEFAULT_DIR"),
        env_dir("HYPRSHOT_DIR"),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// niri's `screenshot-path "~/Pictures/Screenshots/Screenshot from %Y.png"`,
/// minus the file name pattern.
fn niri_folder(config: &str, home: &Path) -> Option<PathBuf> {
    let line = config
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("screenshot-path") && !l.starts_with("//"))?;
    let value = line.split('"').nth(1)?;
    Some(expand(value, home).parent()?.to_path_buf())
}

/// `key=value` out of an ini style file, whichever section it's in.
fn ini_value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == key && !v.trim().is_empty()).then(|| v.trim().to_owned())
    })
}

fn expand(path: &str, home: &Path) -> PathBuf {
    let path = path.trim().trim_matches('"');
    match path.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None if path == "~" => home.to_path_buf(),
        None => PathBuf::from(path.replace("$HOME", &home.to_string_lossy())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn niri_path_loses_its_file_pattern() {
        let config = r#"
            // screenshot-path "~/old/%Y.png"
            screenshot-path "~/Pictures/Screenshots/Screenshot from %Y-%m-%d.png"
        "#;
        let home = Path::new("/home/me");
        assert_eq!(
            niri_folder(config, home),
            Some("/home/me/Pictures/Screenshots".into())
        );
        assert_eq!(niri_folder("binds {}", home), None);
    }

    #[test]
    fn ini_values_are_found_in_any_section() {
        let ini = "[General]\nsavePath=/home/me/shots\nother=1\n";
        assert_eq!(ini_value(ini, "savePath"), Some("/home/me/shots".into()));
        assert_eq!(ini_value("[General]\nsavePath=\n", "savePath"), None);
    }

    #[test]
    fn home_is_expanded() {
        let home = Path::new("/home/me");
        assert_eq!(expand("~/a", home), PathBuf::from("/home/me/a"));
        assert_eq!(expand("$HOME/b", home), PathBuf::from("/home/me/b"));
        assert_eq!(expand("/abs", home), PathBuf::from("/abs"));
    }

    #[test]
    fn the_autostart_entry_runs_the_watcher_hidden() {
        let entry = autostart_entry("/usr/bin/gyotaku");
        assert!(entry.contains("Exec=/usr/bin/gyotaku watch"));
        assert!(entry.contains("NoDisplay=true"));
    }

    #[test]
    fn the_unit_runs_the_watcher_at_idle() {
        let unit = unit_file("/x/gyotaku");
        assert!(unit.contains("ExecStart=/x/gyotaku watch"));
        assert!(unit.contains("CPUSchedulingPolicy=idle"));
    }
}
