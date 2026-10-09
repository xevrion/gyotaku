//! Reading screenshots with the project's own recognizers (`gyotaku-ocr`),
//! for the scripts the phone's built in text recognition can't read. With no
//! extra script turned on, nothing here is used and no model is downloaded.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};

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

// How far the reading threads step aside for everything else, as a nice
// value. 10 while the app is on screen, so its own window stays smooth; 0
// once it is put away, where Android already holds a background app back
// and stepping aside twice over made a slow read three times slower.
static NICE: AtomicI32 = AtomicI32::new(10);

// Set when the scripts change: the readers loaded are for the old ones.
static STALE: AtomicBool = AtomicBool::new(false);

// ONNX Runtime's own worker threads, noted when the readers were loaded.
static WORKERS: Mutex<Vec<i32>> = Mutex::new(Vec::new());

#[cfg(unix)]
fn renice(thread: i32, nice: i32) {
    // SAFETY: changes the scheduling priority of one thread of this
    // process, and failing to is harmless.
    unsafe { libc::setpriority(libc::PRIO_PROCESS as _, thread as _, nice) };
}

#[cfg(not(unix))]
fn renice(_thread: i32, _nice: i32) {}

/// The processor's faster cores: those whose top speed is within reach of
/// the fastest. On a phone that is the big and middle cores, on a machine
/// whose cores are all alike it is all of them. Empty when there is no
/// telling.
///
/// A recognizer splits each line evenly between its threads and waits for
/// the slowest, so one thread left on a small core at a low clock sets the
/// pace for all of them. Put away, a Pixel 6 gave the reader two small
/// cores and two middle ones, and it read at a seventh of its speed.
fn fast_cores() -> &'static [usize] {
    static CORES: OnceLock<Vec<usize>> = OnceLock::new();
    CORES.get_or_init(|| {
        let speeds: Vec<(usize, u64)> = (0..256)
            .map_while(|n| {
                let path = format!("/sys/devices/system/cpu/cpu{n}/cpufreq/cpuinfo_max_freq");
                let speed = std::fs::read_to_string(path).ok()?.trim().parse().ok()?;
                Some((n, speed))
            })
            .collect();
        let top = speeds.iter().map(|&(_, s)| s).max().unwrap_or(0);
        speeds
            .into_iter()
            .filter(|&(_, s)| s * 10 >= top * 7)
            .map(|(n, _)| n)
            .collect()
    })
}

/// Where the reading threads should run just now. On screen, anywhere: the
/// system gives the app in front its pick of the cores and does the
/// choosing well. Put away it hands out the small cores too, and that is
/// when the reader is kept to the faster ones it is still allowed (on a
/// Pixel 6, about twice the speed of leaving it be: 9 s an image against
/// 19 s).
fn cores() -> &'static [usize] {
    if NICE.load(Ordering::Relaxed) > 0 {
        &[]
    } else {
        fast_cores()
    }
}

/// Keeps a thread to `cores`, or with none lets it run anywhere. The
/// system has the last word: cores it doesn't allow this app are left out,
/// and if that leaves none the call fails and nothing changes.
#[cfg(any(target_os = "linux", target_os = "android"))]
// CPU_SETSIZE is an int on Linux and already a usize on Android.
#[allow(clippy::unnecessary_cast)]
fn keep_to(thread: i32, cores: &[usize]) {
    // SAFETY: a zeroed cpu_set_t is an empty set, and the calls only read
    // it. Failing is harmless.
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        if cores.is_empty() {
            for n in 0..libc::CPU_SETSIZE as usize {
                libc::CPU_SET(n, &mut set);
            }
        } else {
            for &n in cores {
                libc::CPU_SET(n, &mut set);
            }
        }
        libc::sched_setaffinity(thread, std::mem::size_of::<libc::cpu_set_t>(), &set);
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn keep_to(_thread: i32, _cores: &[usize]) {}

/// The ids of every thread in this process.
fn threads() -> Vec<i32> {
    std::fs::read_dir("/proc/self/task")
        .map(|dir| {
            dir.filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Says whether the app is on screen, which is when reading steps aside
/// the most. Cheap enough to call before every image.
#[frb(sync)]
pub fn be_polite(polite: bool) {
    let nice = if polite { 10 } else { 0 };
    let changed = NICE.swap(nice, Ordering::Relaxed) != nice;
    for &t in WORKERS.lock().unwrap_or_else(|e| e.into_inner()).iter() {
        if changed {
            renice(t, nice);
        }
        // Every time, not only on a change: which cores the app is allowed
        // moves as it comes and goes from the screen, and some kernels
        // forget a thread's own choice when it does.
        keep_to(t, cores());
    }
}

/// The calling thread at the reading priority, and back when dropped: it
/// belongs to the bridge and answers searches next.
struct Polite;

impl Polite {
    fn new() -> Self {
        renice(0, NICE.load(Ordering::Relaxed));
        keep_to(0, cores());
        Polite
    }
}

impl Drop for Polite {
    fn drop(&mut self) {
        renice(0, 0);
        keep_to(0, &[]);
    }
}

fn loaded(guard: &mut Option<Ocr>) -> Result<&mut Ocr> {
    if STALE.swap(false, Ordering::Relaxed) {
        *guard = None;
    }
    if guard.is_none() {
        let scripts = Config::load_or_default().scripts();
        // The first version of the Bengali model, 54 MB that nothing reads
        // any more.
        if let Ok(dir) = gyotaku_ocr::models_dir() {
            let _ = std::fs::remove_file(dir.join("bengali_easyocr_rec.onnx"));
        }
        let before = threads();
        let ocr = Ocr::new(gyotaku_core::default_threads(), &scripts);
        // Whatever threads appeared while loading are the runtime's workers.
        let workers: Vec<i32> = threads()
            .into_iter()
            .filter(|t| !before.contains(t))
            .collect();
        let nice = NICE.load(Ordering::Relaxed);
        for &t in &workers {
            renice(t, nice);
            keep_to(t, cores());
        }
        *WORKERS.lock().unwrap_or_else(|e| e.into_inner()) = workers;
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
    // Not dropped here: the readers may be mid download, which holds them
    // for minutes, and a switch in settings must not wait for that. The
    // next use sees they are out of date and loads the right ones.
    STALE.store(true, Ordering::Relaxed);
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

/// Lets go of the readers, a few hundred megabytes of models, once there
/// is nothing left to read. The next read loads them again.
pub fn release_readers() {
    *OCR.lock().unwrap_or_else(|e| e.into_inner()) = None;
    WORKERS.lock().unwrap_or_else(|e| e.into_inner()).clear();
}
