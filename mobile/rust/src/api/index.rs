//! The screenshot index, as the Flutter app sees it. Everything here is a
//! thin wrapper over `gyotaku-core`: the phone does the reading (the system's
//! own text recognition) and hands the lines over, and search behaves exactly
//! as it does on the desktop because it is the same code.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use flutter_rust_bridge::frb;
use gyotaku_core::{Index, Shot};

// One connection for the whole app. Calls arrive from flutter_rust_bridge's
// worker threads, and SQLite wants one writer at a time anyway.
static INDEX: Mutex<Option<Index>> = Mutex::new(None);

fn with<T>(f: impl FnOnce(&mut Index) -> Result<T>) -> Result<T> {
    // A panic in an earlier call must not lock the index away for good.
    let mut guard = INDEX.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.as_mut().context("the index is not open yet")?)
}

/// A box in normalized image coordinates, 0 to 1 on both axes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl From<gyotaku_core::Rect> for Rect {
    fn from(r: gyotaku_core::Rect) -> Self {
        Rect {
            x: r.x,
            y: r.y,
            w: r.w,
            h: r.h,
        }
    }
}

/// One line of text and where it sits in the screenshot.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub text: String,
    pub rect: Rect,
    pub score: f32,
}

impl From<gyotaku_core::Line> for Line {
    fn from(l: gyotaku_core::Line) -> Self {
        Line {
            text: l.text,
            rect: l.rect.into(),
            score: l.score,
        }
    }
}

impl From<Line> for gyotaku_core::Line {
    fn from(l: Line) -> Self {
        gyotaku_core::Line {
            text: l.text,
            rect: gyotaku_core::Rect {
                x: l.rect.x,
                y: l.rect.y,
                w: l.rect.w,
                h: l.rect.h,
            },
            score: l.score,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Hit {
    pub id: i64,
    pub path: String,
    /// Seconds since the epoch.
    pub mtime: i64,
    pub width: u32,
    pub height: u32,
    /// The lines that contain a query term. Empty without a query.
    pub lines: Vec<Line>,
    /// Found only by treating look-alike characters as one (`0` for `O`).
    pub near: bool,
}

/// Opens the index at `path`, creating it if needed. The app passes a file
/// in its own data directory; nothing here guesses where that is.
pub fn open_index(path: String) -> Result<()> {
    let index = Index::open(Path::new(&path)).with_context(|| format!("opening {path}"))?;
    *INDEX.lock().unwrap_or_else(|e| e.into_inner()) = Some(index);
    Ok(())
}

/// How many screenshots can be searched.
pub fn shot_count() -> Result<u32> {
    with(|i| Ok(i.visible_len()? as u32))
}

/// Whether `path` is already indexed as it was at `mtime`.
pub fn is_current(path: String, mtime: i64) -> Result<bool> {
    with(|i| i.is_current(Path::new(&path), mtime))
}

/// `is_current` for a whole page of images at once. Asking one at a time is
/// a trip across the bridge each, and a library is thousands of them.
pub fn are_current(paths: Vec<String>, mtimes: Vec<i64>) -> Result<Vec<bool>> {
    with(|i| {
        paths
            .iter()
            .zip(&mtimes)
            .map(|(p, &m)| i.is_current(Path::new(p), m))
            .collect()
    })
}

/// Stores a screenshot and the lines read from it, replacing whatever was
/// there for the same path. Pass 0 x 0 for an image that could not be read,
/// so it is remembered but never shown.
pub fn insert_shot(
    path: String,
    mtime: i64,
    width: u32,
    height: u32,
    lines: Vec<Line>,
) -> Result<()> {
    let shot = Shot {
        path: PathBuf::from(path),
        mtime,
        width,
        height,
        look: None,
    };
    let lines: Vec<gyotaku_core::Line> = lines.into_iter().map(Into::into).collect();
    with(|i| i.insert(&shot, &lines).map(|_| ()))
}

/// Forgets a screenshot. The file itself is never touched.
pub fn remove_shot(path: String) -> Result<bool> {
    with(|i| i.remove(Path::new(&path)))
}

/// Forgets every screenshot, so the next look through the library reads
/// them all again. For after a script is turned on. Only the index is
/// emptied; the files are never touched.
pub fn forget_all() -> Result<u32> {
    with(|i| {
        let paths = i.paths()?;
        for p in &paths {
            i.remove(p)?;
        }
        Ok(paths.len() as u32)
    })
}

/// Forgets every screenshot whose path is not in `paths`: ones deleted from
/// the phone, and ones in a folder that was turned off. Only the index
/// changes; the files are never touched.
pub fn keep_only(paths: Vec<String>) -> Result<u32> {
    let keep: std::collections::HashSet<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    with(|i| {
        let mut gone = 0;
        for p in i.paths()? {
            if !keep.contains(&p) {
                i.remove(&p)?;
                gone += 1;
            }
        }
        Ok(gone)
    })
}

/// Every word of `query` has to appear in the screenshot; an empty query
/// lists the newest. Filters (`in:`, `date:`) work as on the desktop.
pub fn search(query: String, limit: u32) -> Result<Vec<Hit>> {
    with(|i| {
        let hits = i.search(&query, limit as usize)?;
        Ok(hits
            .into_iter()
            .map(|h| Hit {
                id: h.id,
                path: h.path.to_string_lossy().into_owned(),
                mtime: h.mtime,
                width: h.width,
                height: h.height,
                lines: h.lines.into_iter().map(Into::into).collect(),
                near: h.near,
            })
            .collect())
    })
}

/// All the text of one screenshot, top to bottom.
pub fn shot_lines(id: i64) -> Result<Vec<Line>> {
    with(|i| Ok(i.lines(id)?.into_iter().map(Into::into).collect()))
}

/// The part of a screenshot its grid tile shows, see `gyotaku_core::tile_crop`.
#[frb(sync)]
pub fn tile_crop(width: u32, height: u32) -> Rect {
    gyotaku_core::tile_crop(width, height).into()
}

/// Width over height of a grid tile.
#[frb(sync)]
pub fn tile_aspect(width: u32, height: u32) -> f32 {
    gyotaku_core::tile_aspect(width, height)
}

#[frb(init)]
pub fn init_app() {
    flutter_rust_bridge::setup_default_user_utils();
}

#[cfg(test)]
mod tests {
    use super::*;

    // One test, because the index is one global.
    #[test]
    fn a_shot_put_in_can_be_found() {
        assert!(shot_count().is_err(), "nothing is open yet");

        let dir = std::env::temp_dir().join(format!("gyotaku-mobile-{}", std::process::id()));
        open_index(dir.join("index.db").to_string_lossy().into_owned()).unwrap();

        let path = "/storage/emulated/0/Pictures/Screenshots/one.png".to_string();
        let line = Line {
            text: "Order confirmation 0RDER-4471".into(),
            rect: Rect {
                x: 0.1,
                y: 0.2,
                w: 0.5,
                h: 0.03,
            },
            score: 0.98,
        };
        insert_shot(path.clone(), 1_700_000_000, 1080, 2400, vec![line.clone()]).unwrap();

        assert_eq!(shot_count().unwrap(), 1);
        assert!(is_current(path.clone(), 1_700_000_000).unwrap());
        assert!(!is_current(path.clone(), 1_700_000_001).unwrap());
        assert_eq!(
            are_current(
                vec![path.clone(), path.clone(), "/nowhere.png".into()],
                vec![1_700_000_000, 1, 1_700_000_000],
            )
            .unwrap(),
            vec![true, false, false]
        );

        // a substring, as the trigram index allows
        let hits = search("nfirma".into(), 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, path);
        assert_eq!(hits[0].lines, vec![line.clone()]);
        assert!(!hits[0].near);
        assert_eq!(shot_lines(hits[0].id).unwrap(), vec![line]);

        // the folder filter reads the path
        assert_eq!(search("in:screenshots".into(), 10).unwrap().len(), 1);
        assert!(search("in:camera".into(), 10).unwrap().is_empty());

        // a folder turned off takes its shots out, and leaves the rest
        let other = "/storage/emulated/0/DCIM/Camera/two.jpg".to_string();
        insert_shot(other.clone(), 1_700_000_100, 4000, 3000, vec![]).unwrap();
        assert_eq!(keep_only(vec![path.clone()]).unwrap(), 1);
        assert_eq!(shot_count().unwrap(), 1);
        assert_eq!(keep_only(vec![path.clone(), other]).unwrap(), 0);

        assert!(remove_shot(path).unwrap());
        assert_eq!(shot_count().unwrap(), 0);
        std::fs::remove_dir_all(dir).ok();
    }
}
