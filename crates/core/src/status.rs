//! What the reader (`gyotaku watch`) is doing, in one short line the window
//! can show while there's nothing to search yet: getting the models, how far
//! through the backlog it is, or why it stopped. The reader runs hidden, so
//! without this a failed first run just looks like nothing happening.

use std::path::PathBuf;

fn path() -> Option<PathBuf> {
    crate::data_dir().ok().map(|d| d.join("reader.status"))
}

/// Best effort: a status that can't be written is not worth failing over.
/// Written next to it and renamed over, so the window never reads half.
pub fn set(line: &str) {
    let Some(path) = path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, line).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

pub fn get() -> Option<String> {
    let line = std::fs::read_to_string(path()?).ok()?;
    let line = line.trim();
    (!line.is_empty()).then(|| line.to_owned())
}

/// Whether a reader is running right now. It holds `watch.lock` for as long
/// as it lives, and the system drops the lock when it exits however it
/// exits, so a lock that can't be taken means one is running.
pub fn reader_running() -> bool {
    let Some(dir) = crate::data_dir().ok() else {
        return false;
    };
    let Ok(lock) = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("watch.lock"))
    else {
        return false;
    };
    matches!(lock.try_lock(), Err(std::fs::TryLockError::WouldBlock))
}

/// The running reader's process id, which it writes down once it holds the
/// lock. None while no reader runs, so the id one that's gone left behind
/// never names some other process.
pub fn reader_pid() -> Option<u32> {
    if !reader_running() {
        return None;
    }
    let dir = crate::data_dir().ok()?;
    std::fs::read_to_string(dir.join("watch.pid"))
        .ok()?
        .trim()
        .parse()
        .ok()
}
