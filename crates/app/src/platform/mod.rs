//! Everything that depends on the operating system, behind one interface.
//!
//! The rest of the app calls what's re-exported here and never asks which
//! system it's running on. Each system has one file that provides every item
//! below with the same signature, so porting to another one is a matter of
//! writing that file, and the compiler lists exactly what's still missing.
//!
//! | | Linux | Windows | macOS |
//! |---|---|---|---|
//! | the window | a layer-shell overlay on Wayland, an ordinary window elsewhere | a borderless popup above everything | a borderless popup above everything |
//! | while it waits | nothing on screen, the desktop's shortcut is the way in | a notification area icon, no taskbar button | a menu bar icon, no Dock icon |
//! | summoning it | the desktop's own shortcut runs `gyotaku-app` | a global hotkey the app registers | a global hotkey the app registers |
//! | waking the running copy | a unix socket | a loopback port | a unix socket |
//! | reading in the background | a systemd user service, or an XDG autostart entry | the app starts the reader, and the Run key starts the app at sign-in | a launchd agent, and the app starts the reader while it runs |
//! | the clipboard | `wl-copy` or `xclip`, so a copy outlives the window | the system clipboard | the system pasteboard |
//! | screenshot folders | the configs of the common tools | the Windows defaults, Game Bar, ShareX | the macOS tool defaults |

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as imp;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as imp;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as imp;

// The menu bar and notification area icon, shared by the two systems that
// have the app register its own hotkey and live in the background.
#[cfg(any(windows, target_os = "macos"))]
mod tray;

#[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
compile_error!(
    "gyotaku runs on Linux, Windows and macOS so far. A port means adding a file next to \
     linux.rs, windows.rs and macos.rs that provides the same items."
);

// Staying resident between searches. Most of a cold start is the gpu getting
// set up (300 to 800 ms on a laptop with an Nvidia card), far too slow for
// something bound to a key. So the first launch stays around after its window
// closes, and later launches knock on the running one and exit. Whatever
// `listen` returns has an `incoming()` that yields one item per knock.
pub use imp::{listen, resident_address, wake};

// The window, and the key that brings it up.
pub use imp::{hides_when_inactive, open_launcher, register_summon, take_focus};

// Who draws the window's corners. macOS and Windows give the popup a frame of
// their own (rounded where the system rounds, with its rim and shadow along
// that shape), so the panel fills it edge to edge; drawing our own corner
// inside theirs left a sliver between the two curves. On Linux nothing frames
// the overlay, so the panel rounds and outlines itself.
pub use imp::SYSTEM_FRAMES_WINDOW;

// Living in the background like a launcher, the way Raycast does on macOS
// and Windows: no Dock or taskbar button to quit by accident, an icon in the
// menu bar or notification area with the way back in and the way out, and
// the system's close keys putting the window away instead of quitting.
pub use imp::{MODIFIER_NAME, default_key, hide_keys, settle_in};

// Getting the system's folder picker in front of everything.
pub use imp::before_picker;

// Copying out of the window.
pub use imp::{copy_image, copy_text};

// Handing memory back while hidden.
pub use imp::{release_memory, tune_allocator};

// Reading new screenshots without being asked.
pub use imp::{
    background_status, keep_reading, revive_reader, service_status, start_service, stop_service,
};

// Where screenshot tools save, and the wording that differs.
pub use imp::{WORDS, tool_folders};

/// What the menu bar or notification area icon asks of the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(target_os = "linux", allow(dead_code))]
pub enum TrayCommand {
    Open,
    Settings,
    Quit,
}

/// Whether new screenshots are picked up without anyone asking: on Linux,
/// whether the watcher runs in the background; on Windows, whether gyotaku
/// starts with Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    Running,
    Stopped,
}

/// The few things said differently on each system, because what happens
/// differs too.
pub struct Words {
    /// The background reading switch in settings.
    pub background_setting: &'static str,
    /// The onboarding step that asks about it: title, explanation, then the
    /// two answers, each with a line of detail.
    pub background_title: &'static str,
    pub background_body: &'static str,
    pub background_yes: (&'static str, &'static str),
    pub background_no: (&'static str, &'static str),
    /// Shown after onboarding when the answer was no.
    pub chose_no: &'static str,
    /// Why the usual screenshot folder is suggested.
    pub default_folder: &'static str,
    pub copy_image_failed: &'static str,
}
