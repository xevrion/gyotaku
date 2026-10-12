//! `~/.config/gyotaku/config.toml`, shared by the app (which writes it from
//! onboarding and settings) and the watcher (which follows it live).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Every image under these is read, subfolders included.
    pub folders: Vec<PathBuf>,
    pub theme: ThemeChoice,
    /// Cores one screenshot may use while being read.
    pub threads: usize,
    /// Images copied to the clipboard get saved, so the ones that were never
    /// saved anywhere can be searched too. Off unless asked for.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub clipboard: bool,
    /// Where those go, when not the default folder in Pictures.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clipboard_folder: Option<PathBuf>,
    /// Writing systems read on top of the default ones (Latin, Chinese and
    /// Japanese), by name, like `scripts = ["devanagari"]`. Names, not an
    /// enum, so a config from a newer version with a script this one doesn't
    /// know still loads; see `Config::scripts`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scripts: Vec<String>,
    /// Near-identical shots taken close together show as one tile, the rest
    /// a key away (see `burst`). On unless turned off, and only written then.
    #[serde(skip_serializing_if = "Clone::clone")]
    pub group_similar: bool,
    /// Shortcuts changed from their defaults, by name, like
    /// `trash = "ctrl-backspace"`. Only the changed ones are written. Last,
    /// since a table has to come after the plain values.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub keys: BTreeMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            folders: pictures_dir().into_iter().collect(),
            theme: ThemeChoice::System,
            threads: default_threads(),
            clipboard: false,
            clipboard_folder: None,
            scripts: Vec::new(),
            group_similar: true,
            keys: BTreeMap::new(),
        }
    }
}

/// A writing system the default recognizer can't read, with a model of its
/// own that's downloaded the first time it's turned on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Script {
    /// Hindi, Marathi, Nepali, Sanskrit and the rest written in it.
    Devanagari,
    /// Bangla, Assamese and the rest written in it.
    Bengali,
}

impl Script {
    pub const ALL: [Script; 2] = [Script::Devanagari, Script::Bengali];

    /// The name used in the config.
    pub fn name(self) -> &'static str {
        match self {
            Script::Devanagari => "devanagari",
            Script::Bengali => "bengali",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|s| s.name().eq_ignore_ascii_case(name.trim()))
    }
}

/// Reading a screenshot stops getting much faster past 4 cores, and on a
/// dual core laptop 4 would just fight over 2.
pub fn default_threads() -> usize {
    std::thread::available_parallelism().map_or(2, |n| n.get().min(4))
}

/// Folders gyotaku won't take: the whole home folder or the whole disk.
/// Reading those means every image anyone ever saved, hundreds of thousands
/// of files, and more inotify watches than a default system allows.
pub fn too_broad(folder: &Path) -> bool {
    let home = directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf());
    folder.parent().is_none() || home.as_deref() == Some(folder)
}

impl Config {
    /// The extra scripts to read, skipping names this version doesn't know.
    pub fn scripts(&self) -> Vec<Script> {
        let mut out: Vec<Script> = self
            .scripts
            .iter()
            .filter_map(|n| Script::from_name(n))
            .collect();
        out.dedup();
        out
    }

    /// Turns one script on or off, keeping any names this version doesn't
    /// know as they were.
    pub fn set_script(&mut self, script: Script, on: bool) {
        self.scripts
            .retain(|n| Script::from_name(n) != Some(script));
        if on {
            self.scripts.push(script.name().to_owned());
        }
    }

    /// Where copied images are saved, while saving them is on.
    pub fn clipboard_folder(&self) -> Option<PathBuf> {
        if !self.clipboard {
            return None;
        }
        self.clipboard_folder
            .clone()
            .or_else(|| pictures_dir().map(|p| p.join("Clipboard")))
    }

    /// The folders the reader reads: the chosen ones, plus the clipboard
    /// folder unless one of them holds it already. That's while saving is
    /// on, and after it's turned off too, for as long as the folder is there,
    /// so what was saved stays searchable. The app writes the folder down
    /// when saving is turned on, which is how it's still known after.
    pub fn reading_folders(&self) -> Vec<PathBuf> {
        let mut folders = self.folders.clone();
        let clips = self
            .clipboard_folder()
            .or_else(|| self.clipboard_folder.clone().filter(|f| f.is_dir()));
        if let Some(clips) = clips
            && !folders.iter().any(|f| clips.starts_with(f))
        {
            folders.push(clips);
        }
        folders
    }

    pub fn path() -> Result<PathBuf> {
        let dirs = directories::ProjectDirs::from("", "", "gyotaku")
            .context("could not work out a home directory")?;
        Ok(dirs.config_dir().join("config.toml"))
    }

    /// None before onboarding has ever been finished, which is how the app
    /// knows to show it.
    pub fn load() -> Result<Option<Self>> {
        Self::load_from(&Self::path()?)
    }

    /// The saved config, or the defaults if there isn't one yet.
    pub fn load_or_default() -> Self {
        Self::load().ok().flatten().unwrap_or_default()
    }

    pub fn load_from(path: &Path) -> Result<Option<Self>> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let config =
            toml::from_str(&text).with_context(|| format!("reading {}", path.display()))?;
        Ok(Some(config))
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::path()?)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // Written next to it and renamed over, so the watcher (which reloads
        // on every change) never reads half a file.
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, toml::to_string_pretty(self)?)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

pub fn pictures_dir() -> Option<PathBuf> {
    directories::UserDirs::new()
        .and_then(|d| d.picture_dir().map(Path::to_path_buf))
        .or_else(|| directories::BaseDirs::new().map(|d| d.home_dir().join("Pictures")))
}

/// `/home/you/Pictures` as `~/Pictures`, for showing to people.
pub fn tidy(path: &Path) -> String {
    let home = directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf());
    match home.as_deref().and_then(|h| path.strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("gyotaku-config-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("config.toml")
    }

    #[test]
    fn missing_file_means_not_set_up_yet() {
        assert_eq!(Config::load_from(&scratch("missing")).unwrap(), None);
    }

    #[test]
    fn round_trips() {
        let path = scratch("round");
        let config = Config {
            folders: vec!["/a/b".into(), "/c".into()],
            theme: ThemeChoice::Dark,
            threads: 2,
            clipboard: true,
            clipboard_folder: Some("/c/copied".into()),
            scripts: vec!["devanagari".into()],
            group_similar: false,
            keys: BTreeMap::from([("trash".into(), "ctrl-backspace".into())]),
        };
        config.save_to(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("[keys]\ntrash = \"ctrl-backspace\""),
            "{text}"
        );
        assert!(text.contains("group_similar = false"), "{text}");
        assert_eq!(Config::load_from(&path).unwrap(), Some(config));
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn unchanged_shortcuts_leave_no_trace() {
        let path = scratch("nokeys");
        Config::default().save_to(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            !text.contains("keys") && !text.contains("clipboard") && !text.contains("similar"),
            "{text}"
        );
        // Missing means on.
        assert!(Config::load_from(&path).unwrap().unwrap().group_similar);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn the_clipboard_folder_is_read_only_while_saving_is_on() {
        let mut config = Config {
            folders: vec!["/p/shots".into()],
            clipboard_folder: Some("/p/copied".into()),
            ..Config::default()
        };
        assert_eq!(config.clipboard_folder(), None);
        assert_eq!(config.reading_folders(), vec![PathBuf::from("/p/shots")]);

        config.clipboard = true;
        assert_eq!(config.clipboard_folder(), Some("/p/copied".into()));
        assert_eq!(
            config.reading_folders(),
            vec![PathBuf::from("/p/shots"), "/p/copied".into()]
        );

        // Already inside a folder that's read, so not listed twice.
        config.folders = vec!["/p".into()];
        assert_eq!(config.reading_folders(), vec![PathBuf::from("/p")]);
    }

    #[test]
    fn what_was_saved_stays_searchable_after_turning_it_off() {
        let clips = scratch("clips").parent().unwrap().to_path_buf();
        std::fs::create_dir_all(&clips).unwrap();
        let config = Config {
            folders: vec!["/p/shots".into()],
            clipboard: false,
            clipboard_folder: Some(clips.clone()),
            ..Config::default()
        };
        assert_eq!(config.clipboard_folder(), None, "nothing new is saved");
        assert_eq!(
            config.reading_folders(),
            vec!["/p/shots".into(), clips.clone()]
        );
        std::fs::remove_dir_all(clips).unwrap();
    }

    #[test]
    fn unknown_scripts_are_skipped_but_kept() {
        let path = scratch("scripts");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "scripts = [\"Devanagari\", \"klingon\"]\n").unwrap();
        let mut config = Config::load_from(&path).unwrap().unwrap();
        assert_eq!(config.scripts(), vec![Script::Devanagari]);

        config.set_script(Script::Devanagari, false);
        assert!(config.scripts().is_empty());
        assert_eq!(
            config.scripts,
            vec!["klingon".to_string()],
            "left for a newer version"
        );

        config.set_script(Script::Devanagari, true);
        config.set_script(Script::Devanagari, true);
        assert_eq!(config.scripts(), vec![Script::Devanagari], "never twice");
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn no_scripts_leave_no_trace() {
        let text = toml::to_string_pretty(&Config::default()).unwrap();
        assert!(!text.contains("scripts"), "{text}");
    }

    #[test]
    fn the_clipboard_folder_defaults_to_one_in_pictures() {
        let config = Config {
            clipboard: true,
            ..Config::default()
        };
        if let Some(pictures) = pictures_dir() {
            assert_eq!(config.clipboard_folder(), Some(pictures.join("Clipboard")));
        }
    }

    #[test]
    fn a_half_written_file_by_hand_still_loads() {
        let path = scratch("partial");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "theme = \"light\"\n").unwrap();
        let config = Config::load_from(&path).unwrap().unwrap();
        assert_eq!(config.theme, ThemeChoice::Light);
        // The default, which depends on the machine: GitHub's mac runners
        // have three cores.
        assert_eq!(config.threads, default_threads());
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn home_and_root_are_too_broad() {
        let home = directories::BaseDirs::new()
            .unwrap()
            .home_dir()
            .to_path_buf();
        assert!(too_broad(&home));
        assert!(too_broad(Path::new("/")));
        assert!(!too_broad(&home.join("Pictures")));
    }

    #[test]
    fn default_threads_fit_the_machine() {
        let cores = std::thread::available_parallelism().unwrap().get();
        assert!(default_threads() >= 1 && default_threads() <= cores.min(4));
    }

    #[test]
    fn home_is_shown_as_tilde() {
        let home = directories::BaseDirs::new()
            .unwrap()
            .home_dir()
            .to_path_buf();
        assert_eq!(tidy(&home.join("Pictures")), "~/Pictures");
        assert_eq!(tidy(&home), "~");
        assert_eq!(tidy(Path::new("/mnt/shots")), "/mnt/shots");
    }
}
