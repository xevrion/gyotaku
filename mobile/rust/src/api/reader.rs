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
    let mut guard = OCR.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        let scripts = Config::load_or_default().scripts();
        *guard = Some(Ocr::new(gyotaku_core::default_threads(), &scripts)?);
    }
    let img = gyotaku_ocr::load_image(std::path::Path::new(&path))?;
    let lines = guard.as_mut().expect("loaded just above").read(&img)?;
    Ok(lines.into_iter().map(Into::into).collect())
}
