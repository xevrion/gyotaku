//! Reading screenshots with the project's own recognizers (`gyotaku-ocr`),
//! for the scripts the phone's built in text recognition can't read. With no
//! extra script turned on, nothing here is used and no model is downloaded.

use std::sync::Mutex;

use anyhow::{Context, Result};
use flutter_rust_bridge::frb;
use gyotaku_core::{Config, Script};
use gyotaku_ocr::Ocr;

use super::index::Line;

// Loaded on the first read and dropped when the scripts change, so a script
// turned off gives its memory back.
static OCR: Mutex<Option<Ocr>> = Mutex::new(None);

// The download under way, if there is one, for the app to draw.
static DOWNLOAD: Mutex<Option<DownloadProgress>> = Mutex::new(None);

/// How far along the download of one model is.
#[derive(Debug, Clone, PartialEq)]
pub struct DownloadProgress {
    /// The file's name, like `bengali_easyocr_rec.onnx`.
    pub file: String,
    pub done_kb: u32,
    /// 0 when the server didn't say.
    pub total_kb: u32,
}

/// Tells the core where it lives on this phone. `gyotaku-core` finds its
/// folders the way a Linux program does, from HOME and the XDG variables, and
/// an app has neither until it sets them. `runtime` is the ONNX Runtime
/// library that ships inside the app, for systems where one does.
///
/// Call once, before anything else, while nothing else is running.
#[frb(sync)]
pub fn settle(home: String, runtime: Option<String>) {
    // SAFETY: called from the app's first line of `main`, before the index,
    // the readers or any worker thread of ours exists to read these.
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("XDG_DATA_HOME", format!("{home}/data"));
        std::env::set_var("XDG_CONFIG_HOME", format!("{home}/config"));
        std::env::set_var("XDG_CACHE_HOME", format!("{home}/cache"));
        if let Some(runtime) = runtime {
            std::env::set_var("ORT_DYLIB_PATH", runtime);
        }
    }
    gyotaku_ocr::watch_downloads(|d| {
        *DOWNLOAD.lock().unwrap_or_else(|e| e.into_inner()) = Some(DownloadProgress {
            file: d.file.to_owned(),
            done_kb: (d.done / 1024) as u32,
            total_kb: (d.total.unwrap_or(0) / 1024) as u32,
        });
    });
}

/// The model being downloaded right now, or None. Cheap enough to ask a few
/// times a second.
#[frb(sync)]
pub fn download_progress() -> Option<DownloadProgress> {
    DOWNLOAD.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Reading is background work: it should take whatever the phone has to
/// spare and step aside for whatever the person is doing, the way the
/// desktop reader runs at idle priority. Threads made while this is in force
/// inherit it, which is how ONNX Runtime's own workers get it too.
struct Polite;

impl Polite {
    #[cfg(unix)]
    fn new() -> Self {
        // SAFETY: changes the scheduling priority of the calling thread
        // only, and failing to is harmless.
        unsafe { libc::setpriority(libc::PRIO_PROCESS as _, 0, 10) };
        Polite
    }

    #[cfg(not(unix))]
    fn new() -> Self {
        Polite
    }
}

impl Drop for Polite {
    // The thread belongs to the bridge and answers searches next.
    fn drop(&mut self) {
        #[cfg(unix)]
        // SAFETY: as above.
        unsafe {
            libc::setpriority(libc::PRIO_PROCESS as _, 0, 0)
        };
    }
}

fn loaded(guard: &mut Option<Ocr>) -> Result<&mut Ocr> {
    if guard.is_none() {
        let scripts = Config::load_or_default().scripts();
        let ocr = Ocr::new(gyotaku_core::default_threads(), &scripts);
        *DOWNLOAD.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *guard = Some(ocr?);
    }
    Ok(guard.as_mut().expect("loaded just above"))
}

/// Gets the readers ready: downloads the models that aren't there yet and
/// loads them. Until this or the first `read_image` returns,
/// `download_progress` says how far along it is.
pub fn load_readers() -> Result<()> {
    let _polite = Polite::new();
    let mut guard = OCR.lock().unwrap_or_else(|e| e.into_inner());
    loaded(&mut guard).map(|_| ())
}

/// A writing system that can be turned on, see `gyotaku_core::Script`.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptChoice {
    /// The name in the config, like `bengali`.
    pub name: String,
    pub enabled: bool,
}

/// Every script this version knows, and whether it is on.
pub fn scripts() -> Vec<ScriptChoice> {
    let on = Config::load_or_default().scripts();
    Script::ALL
        .iter()
        .map(|s| ScriptChoice {
            name: s.name().to_owned(),
            enabled: on.contains(s),
        })
        .collect()
}

/// Turns a script on or off. Like on the desktop, it applies to screenshots
/// read from then on.
pub fn set_script(name: String, enabled: bool) -> Result<()> {
    let script = Script::from_name(&name).with_context(|| format!("no script called {name}"))?;
    let mut config = Config::load_or_default();
    config.set_script(script, enabled);
    config.save().context("saving the settings")?;
    *OCR.lock().unwrap_or_else(|e| e.into_inner()) = None;
    Ok(())
}

/// Reads one image with the default recognizer and every script that is on.
/// The first call downloads the models it needs, which is the only time this
/// touches the network.
pub fn read_image(path: String) -> Result<Vec<Line>> {
    let _polite = Polite::new();
    let mut guard = OCR.lock().unwrap_or_else(|e| e.into_inner());
    let ocr = loaded(&mut guard)?;
    let img = gyotaku_ocr::load_image(std::path::Path::new(&path))?;
    Ok(ocr.read(&img)?.into_iter().map(Into::into).collect())
}

/// Reads only what `known` leaves: the boxes of lines the phone's own
/// reader already read well. The slow recognizers are then spent on the
/// Bangla or Devanagari it could make nothing of, not on Latin text it read
/// in a fraction of the time. See `gyotaku_ocr::Ocr::read_rest`.
pub fn read_rest(path: String, known: Vec<super::index::Rect>) -> Result<Vec<Line>> {
    let _polite = Polite::new();
    let mut guard = OCR.lock().unwrap_or_else(|e| e.into_inner());
    let ocr = loaded(&mut guard)?;
    let img = gyotaku_ocr::load_image(std::path::Path::new(&path))?;
    let known: Vec<gyotaku_core::Rect> = known
        .into_iter()
        .map(|r| gyotaku_core::Rect {
            x: r.x,
            y: r.y,
            w: r.w,
            h: r.h,
        })
        .collect();
    Ok(ocr
        .read_rest(&img, &known)?
        .into_iter()
        .map(Into::into)
        .collect())
}
