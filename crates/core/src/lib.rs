pub mod burst;
pub mod clipboard;
mod config;
mod index;
pub mod naming;
pub mod query;
pub mod status;
pub mod trash;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

pub use config::{Config, Script, ThemeChoice, default_threads, pictures_dir, tidy, too_broad};
pub use index::{Hit, Index};
pub use naming::{
    CaseStyle, FallbackStrategy, InfographicNature, NameOptions, PlannedRename, TitleCandidate,
};
pub use query::{Filter, Query};

/// A box in normalized image coordinates, so the same numbers work on a
/// 240px thumbnail and on the full size view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

// Grid tiles keep a screenshot's shape, within limits. A 5000 px tall scroll
// capture or a thin banner would otherwise wreck the row it lands in.
const TILE_MIN_ASPECT: f32 = 0.5;
const TILE_MAX_ASPECT: f32 = 3.0;

/// The part of a screenshot its grid tile shows, as a fraction of the whole.
/// Tall shots keep their top (where a page or chat title usually is), wide
/// ones keep their middle. Thumbnails are cut to exactly this, and the app
/// uses it to place highlight boxes, so the two can never disagree.
pub fn tile_crop(width: u32, height: u32) -> Rect {
    let aspect = width as f32 / height.max(1) as f32;
    if aspect < TILE_MIN_ASPECT {
        Rect {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: aspect / TILE_MIN_ASPECT,
        }
    } else if aspect > TILE_MAX_ASPECT {
        let w = TILE_MAX_ASPECT / aspect;
        Rect {
            x: (1.0 - w) / 2.0,
            y: 0.0,
            w,
            h: 1.0,
        }
    } else {
        Rect {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
        }
    }
}

/// Width over height of a tile, after the crop above.
pub fn tile_aspect(width: u32, height: u32) -> f32 {
    (width as f32 / height.max(1) as f32).clamp(TILE_MIN_ASPECT, TILE_MAX_ASPECT)
}

/// One line of text the OCR found, and where it found it.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub text: String,
    pub rect: Rect,
    pub score: f32,
}

#[derive(Debug, Clone)]
pub struct Shot {
    pub path: PathBuf,
    /// Seconds since the epoch. Doubles as the "taken at" time and as the
    /// cheap check for whether a file changed since we indexed it.
    pub mtime: i64,
    /// 0 x 0 for files that couldn't be read or are too small to be a real
    /// screenshot. They stay in the index so they aren't retried every run,
    /// but search never returns them.
    pub width: u32,
    pub height: u32,
    /// The thumbnail's difference hash, see `burst::look`. None when it
    /// couldn't be made; the reader fills it in later from the thumbnail.
    pub look: Option<u64>,
}

fn dirs() -> Result<directories::ProjectDirs> {
    directories::ProjectDirs::from("", "", "gyotaku").context("could not work out a home directory")
}

pub fn data_dir() -> Result<PathBuf> {
    Ok(dirs()?.data_dir().to_path_buf())
}

/// Where the grid thumbnail for a screenshot lives. Named after the path, not
/// the row id, so re-indexing a changed file overwrites its old thumbnail
/// instead of leaving it behind.
pub fn thumb_path(shot: &Path) -> Result<PathBuf> {
    let digest = Sha256::digest(shot.as_os_str().as_encoded_bytes());
    let name: String = digest[..12].iter().map(|b| format!("{b:02x}")).collect();
    Ok(dirs()?
        .cache_dir()
        .join("thumbs")
        .join(format!("{name}.jpg")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_shots_are_not_cropped() {
        assert_eq!(
            tile_crop(1920, 1080),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0
            }
        );
        assert_eq!(tile_aspect(1920, 1080), 1920.0 / 1080.0);
    }

    #[test]
    fn tall_shots_keep_their_top() {
        // 400x2000 is aspect 0.2, the tile shows the top 400x800
        let c = tile_crop(400, 2000);
        assert_eq!((c.x, c.y, c.w), (0.0, 0.0, 1.0));
        assert!((c.h - 0.4).abs() < 1e-6);
        assert_eq!(tile_aspect(400, 2000), 0.5);
    }

    #[test]
    fn wide_shots_keep_their_middle() {
        // 3000x250 is aspect 12, the tile shows the middle 750x250
        let c = tile_crop(3000, 250);
        assert!((c.w - 0.25).abs() < 1e-6 && (c.x - 0.375).abs() < 1e-6);
        assert_eq!(tile_aspect(3000, 250), 3.0);
    }

    #[test]
    fn zero_height_does_not_divide_by_zero() {
        assert!(tile_crop(10, 0).w.is_finite());
    }
}
