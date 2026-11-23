//! The few things the reader does differently on each system: getting out of
//! the way of everything else, handing memory back, noticing a battery, and
//! watching the clipboard. Each system has one file providing these with the
//! same signatures.
//!
//! | | Linux | Windows |
//! |---|---|---|
//! | the clipboard | `wl-paste --watch` on Wayland, XFixes and `xclip` on X11 | a clipboard format listener |

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as imp;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as imp;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as imp;

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
compile_error!(
    "gyotaku runs on Linux, macOS and Windows so far. A port means adding a file next to \
     linux.rs, macos.rs and windows.rs that provides the same items."
);

/// Drops the process to the lowest priority the system has, so reading only
/// ever runs on time nothing else wants. Failing is fine, it just runs at
/// normal priority.
pub use imp::become_idle;

/// Gives memory freed after a screenshot back to the system. Right after OCR
/// that's a couple hundred MB nobody needs until the next screenshot, which
/// for the watcher can be hours away.
pub use imp::release_memory;

/// Whether the machine is running on battery right now. Desktops never are.
pub use imp::on_battery;

/// How often a running reader looks over its folders on its own, where the
/// system's file events can be lost without a word. None where they can't.
pub use imp::RESCAN_EVERY;

/// What the window should say when a folder can't be opened for lack of
/// permission, with the folder already tidied for showing.
pub use imp::denied;

/// Sends stderr somewhere it can be read later when whoever started the
/// reader left it nowhere.
pub use imp::keep_a_log;

/// Whether a file is only a placeholder for one in cloud storage (OneDrive's
/// files on demand), which reading would download. Pictures is often synced
/// to OneDrive on Windows, and reading would pull the whole library down.
pub use imp::only_in_the_cloud;

/// Watches the clipboard and sends every image put on it, as the encoded
/// bytes the clipboard holds (PNG, or BMP built from a bitmap), until the
/// returned handle is dropped. Never fails: where the clipboard can't be
/// watched (no desktop session yet, a helper missing) it says so on stderr
/// and keeps trying now and then, since a reader started at login can come
/// up before the desktop does. A port with no way to watch the clipboard
/// returns a handle that never sends anything.
pub use imp::{ClipboardWatch, watch_clipboard};

/// Prints a shell script for safe batch renaming (copying with no overwrite).
pub use imp::print_rename_script;
