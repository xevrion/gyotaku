//! Every keyboard shortcut in one place: the commands that can be rebound in
//! settings (saved in config.toml under `[keys]`), and the keys that can't.
//! Moving around stays fixed on purpose, so no setting can leave someone
//! unable to get back out.

use std::collections::{BTreeMap, HashMap};

use gpui::{App, Global, KeyBinding, Keystroke, SharedString};

use crate::app::*;
use crate::input::*;

pub struct Shortcut {
    /// The name in config.toml.
    pub name: &'static str,
    pub label: &'static str,
    pub default: &'static str,
    bind: fn(&str) -> KeyBinding,
}

pub const SHORTCUTS: [Shortcut; 10] = [
    Shortcut {
        name: "copy_text",
        label: "copy text",
        default: "ctrl-c",
        bind: |k| KeyBinding::new(k, CopyText, Some("Gyotaku")),
    },
    Shortcut {
        name: "copy_image",
        label: "copy image",
        default: "ctrl-shift-c",
        bind: |k| KeyBinding::new(k, CopyImage, Some("Gyotaku")),
    },
    Shortcut {
        name: "open",
        label: "open in the image viewer",
        default: "ctrl-o",
        bind: |k| KeyBinding::new(k, OpenExternal, Some("Gyotaku")),
    },
    Shortcut {
        name: "reveal",
        label: "show in its folder",
        default: "ctrl-shift-o",
        bind: |k| KeyBinding::new(k, Reveal, Some("Gyotaku")),
    },
    Shortcut {
        name: "similar",
        label: "show or hide similar screenshots",
        default: "ctrl-e",
        bind: |k| KeyBinding::new(k, Similar, Some("Gyotaku")),
    },
    Shortcut {
        name: "mark_all",
        label: "mark every result",
        default: "ctrl-shift-a",
        bind: |k| KeyBinding::new(k, MarkAll, Some("Gyotaku")),
    },
    Shortcut {
        name: "trash",
        label: "move to the trash",
        default: "ctrl-delete",
        bind: |k| KeyBinding::new(k, Trash, Some("Gyotaku")),
    },
    Shortcut {
        name: "undo",
        label: "undo moving to the trash",
        default: "ctrl-z",
        bind: |k| KeyBinding::new(k, Undo, Some("Gyotaku")),
    },
    Shortcut {
        name: "settings",
        label: "settings",
        default: "ctrl-,",
        bind: |k| KeyBinding::new(k, OpenSettings, Some("Gyotaku")),
    },
    Shortcut {
        name: "quit",
        label: "quit",
        default: "ctrl-q",
        bind: |k| KeyBinding::new(k, Quit, Some("Gyotaku")),
    },
];

/// The default key for a shortcut on this system: the platform override
/// when it has one (macOS today), the shared table above everywhere else.
/// Overrides in config.toml are compared against this, so reset, clash
/// checks and "changed from" agree with what a fresh install binds.
pub fn default(s: &Shortcut) -> &str {
    crate::platform::default_key(s.name).unwrap_or(s.default)
}

/// Default for fixed keys outside the rebindable table (panel keys): the
/// platform override when it has one, otherwise the given fallback.
pub fn fixed(name: &str, fallback: &'static str) -> &'static str {
    crate::platform::default_key(name).unwrap_or(fallback)
}

/// Shown in settings next to the ones that can be changed.
pub const FIXED: [(&str, &str); 6] = [
    ("esc", "back"),
    ("enter", "open"),
    ("arrows", "move"),
    ("pgup pgdn", "jump"),
    ("shift arrows", "mark a run"),
    ("ctrl click", "mark one"),
];

// The search field edits text with these, and it sits closer to the focus
// than any shortcut, so a shortcut bound to one would never fire.
const TEXT_EDITING: [&str; 4] = ["ctrl-a", "ctrl-v", "ctrl-x", "ctrl-backspace"];

/// What each shortcut is bound to right now, ready to show, so hints on
/// screen always name the keys that actually work.
struct Bound(HashMap<&'static str, SharedString>);

impl Global for Bound {}

/// The keys for a shortcut, as shown on screen: `ctrl shift c`.
pub fn shown(name: &str, cx: &App) -> SharedString {
    cx.try_global::<Bound>()
        .and_then(|b| b.0.get(name).cloned())
        .unwrap_or_default()
}

/// The key each shortcut ends up on, never the same key twice. Shortcuts
/// left at their default keep it. A changed one gets its new key unless a
/// shortcut before it already has that key; otherwise it falls back to its
/// default, and if even that is taken it's left without a key rather than
/// silently shadowing another. A hand-edited config with a typo or a clash
/// can't break the others.
fn resolve(overrides: &BTreeMap<String, String>) -> Vec<String> {
    let changed = |s: &Shortcut| overrides.get(s.name).filter(|k| usable(k).is_ok()).cloned();
    let mut taken: Vec<String> = SHORTCUTS
        .iter()
        .filter(|s| changed(s).is_none())
        .map(|s| default(s).to_string())
        .collect();
    SHORTCUTS
        .iter()
        .map(|s| match changed(s) {
            None => default(s).to_string(),
            Some(key) => {
                let key = [key, default(s).to_string()]
                    .into_iter()
                    .find(|k| !taken.contains(k))
                    .unwrap_or_default();
                taken.push(key.clone());
                key
            }
        })
        .collect()
}

/// Why a key can't be a shortcut, if it can't.
pub fn usable(key: &str) -> Result<(), &'static str> {
    let Ok(k) = Keystroke::parse(key) else {
        return Err("that key can't be used");
    };
    let m = k.modifiers;
    let function_key =
        k.key.len() > 1 && k.key.starts_with('f') && k.key[1..].parse::<u8>().is_ok();
    if !(m.control || m.alt || m.platform || function_key) {
        return Err("use it with ctrl, alt or super, it would type otherwise");
    }
    if TEXT_EDITING.contains(&key) {
        return Err("the search field uses that one for editing");
    }
    Ok(())
}

/// The keys bound to a shortcut, by its place in SHORTCUTS.
pub fn current(i: usize, overrides: &BTreeMap<String, String>) -> String {
    resolve(overrides).swap_remove(i)
}

/// Which other shortcut already has this key, if any.
pub fn taken_by(key: &str, except: usize, overrides: &BTreeMap<String, String>) -> Option<usize> {
    resolve(overrides)
        .iter()
        .enumerate()
        .find(|(i, k)| *i != except && k.as_str() == key)
        .map(|(i, _)| i)
}

/// Throws out every binding and binds them all again, so a change in
/// settings applies straight away.
pub fn bind_all(cx: &mut App, overrides: &BTreeMap<String, String>) {
    cx.clear_key_bindings();
    cx.bind_keys([
        KeyBinding::new("escape", Back, Some("Gyotaku")),
        KeyBinding::new("enter", Open, Some("Gyotaku")),
        KeyBinding::new("up", Up, Some("Gyotaku")),
        KeyBinding::new("down", Down, Some("Gyotaku")),
        KeyBinding::new("left", Left, Some("Gyotaku")),
        KeyBinding::new("right", Right, Some("Gyotaku")),
        KeyBinding::new("pageup", PageUp, Some("Gyotaku")),
        KeyBinding::new("pagedown", PageDown, Some("Gyotaku")),
        KeyBinding::new("shift-up", MarkUp, Some("Gyotaku")),
        KeyBinding::new("shift-down", MarkDown, Some("Gyotaku")),
        KeyBinding::new("shift-left", MarkLeft, Some("Gyotaku")),
        KeyBinding::new("shift-right", MarkRight, Some("Gyotaku")),
        KeyBinding::new("space", Toggle, Some("Panel")),
        KeyBinding::new("delete", Remove, Some("Panel")),
        KeyBinding::new("backspace", Remove, Some("Panel")),
        KeyBinding::new(fixed("add_folder", "ctrl-o"), AddFolder, Some("Panel")),
        KeyBinding::new("backspace", Backspace, Some("TextInput")),
        KeyBinding::new("ctrl-backspace", DeleteWord, Some("TextInput")),
        KeyBinding::new("delete", Delete, Some("TextInput")),
        KeyBinding::new("ctrl-a", SelectAll, Some("TextInput")),
        KeyBinding::new("home", Home, Some("TextInput")),
        KeyBinding::new("end", End, Some("TextInput")),
        KeyBinding::new("ctrl-v", Paste, Some("TextInput")),
        KeyBinding::new("ctrl-x", Cut, Some("TextInput")),
    ]);
    // The system's own close keys, where it has any, put the window away
    // from anywhere in it, the text field included.
    for key in crate::platform::hide_keys() {
        cx.bind_keys([KeyBinding::new(key, crate::HideWindow, None)]);
    }
    let keys = resolve(overrides);
    let mut bound = HashMap::new();
    for (s, key) in SHORTCUTS.iter().zip(&keys) {
        if !key.is_empty() {
            cx.bind_keys([(s.bind)(key)]);
        }
        bound.insert(s.name, pretty(key));
    }
    cx.set_global(Bound(bound));
}

/// `ctrl-shift-c` as `ctrl shift c`, the way every hint in the app reads.
/// The platform modifier reads per OS: `super` on Linux, the command mark
/// on macOS.
pub fn pretty(key: &str) -> SharedString {
    if key.is_empty() {
        return "no key".into();
    }
    let Ok(k) = Keystroke::parse(key) else {
        return key.to_string().into();
    };
    let m = k.modifiers;
    let mut parts: Vec<&str> = Vec::new();
    for (on, name) in [
        (m.control, "ctrl"),
        (m.alt, "alt"),
        (m.platform, crate::platform::MODIFIER_NAME),
        (m.shift, "shift"),
    ] {
        if on {
            parts.push(name);
        }
    }
    let key = match k.key.as_str() {
        "delete" => "del",
        "escape" => "esc",
        "backspace" => "bksp",
        "pageup" => "pgup",
        "pagedown" => "pgdn",
        other => other,
    };
    parts.push(key);
    parts.join(" ").into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_keys_and_editing_keys_are_refused() {
        assert!(usable("a").is_err());
        assert!(usable("shift-a").is_err());
        assert!(usable("delete").is_err());
        assert!(usable("ctrl-v").is_err());
        assert!(usable("ctrl-backspace").is_err());
        assert!(usable("ctrl-k").is_ok());
        assert!(usable("alt-shift-d").is_ok());
        assert!(usable("super-t").is_ok());
        assert!(usable("f5").is_ok());
    }

    #[test]
    fn overrides_apply_and_bad_ones_fall_back() {
        let ix = |name| SHORTCUTS.iter().position(|s| s.name == name).unwrap();
        let trash_default = default(&SHORTCUTS[ix("trash")]).to_string();
        let copy_default = default(&SHORTCUTS[ix("copy_text")]).to_string();
        let open_default = default(&SHORTCUTS[ix("open")]).to_string();
        let mut o = BTreeMap::new();
        o.insert("trash".to_string(), "ctrl-k".to_string());
        assert_eq!(current(ix("trash"), &o), "ctrl-k");
        o.insert("trash".to_string(), "x".to_string());
        assert_eq!(current(ix("trash"), &o), trash_default);
        // A clash with an earlier shortcut keeps the default instead.
        o.insert("trash".to_string(), copy_default);
        assert_eq!(current(ix("trash"), &o), trash_default);
        assert_eq!(taken_by(&open_default, ix("trash"), &o), Some(ix("open")));
        assert_eq!(taken_by("ctrl-k", ix("trash"), &o), None);
    }

    #[test]
    fn no_key_is_ever_bound_twice() {
        let ix = |name| SHORTCUTS.iter().position(|s| s.name == name).unwrap();
        let trash_default = default(&SHORTCUTS[ix("trash")]).to_string();
        let copy_default = default(&SHORTCUTS[ix("copy_text")]).to_string();
        // copy text took trash's default while trash was on ctrl k; then the
        // trash override was deleted by hand.
        let mut o = BTreeMap::new();
        o.insert("copy_text".to_string(), trash_default.clone());
        assert_eq!(current(ix("trash"), &o), trash_default);
        assert_eq!(current(ix("copy_text"), &o), copy_default);
        // Two changed shortcuts after the same key: the first keeps it.
        o.insert("trash".to_string(), "ctrl-k".to_string());
        o.insert("undo".to_string(), "ctrl-k".to_string());
        let keys = resolve(&o);
        let mut seen = std::collections::HashSet::new();
        assert!(
            keys.iter()
                .filter(|k| !k.is_empty())
                .all(|k| seen.insert(k))
        );
    }

    #[test]
    fn defaults_are_all_usable_and_distinct() {
        for s in &SHORTCUTS {
            assert!(usable(s.default).is_ok(), "{}", s.default);
            // An empty default leaves the shortcut unbound (quit on macOS,
            // where quitting stays in the menu bar so the summon key lives).
            let d = default(s);
            assert!(d.is_empty() || usable(d).is_ok(), "{d}");
        }
        assert!(usable(fixed("add_folder", "ctrl-o")).is_ok());
        let none = BTreeMap::new();
        let keys = resolve(&none);
        for (i, s) in SHORTCUTS.iter().enumerate() {
            assert_eq!(keys[i], default(s));
        }
    }

    #[test]
    fn keys_read_like_the_hints() {
        assert_eq!(pretty("ctrl-shift-c"), "ctrl shift c");
        assert_eq!(pretty("ctrl-delete"), "ctrl del");
        assert_eq!(pretty("ctrl-,"), "ctrl ,");
        assert_eq!(
            pretty("cmd-c").to_string(),
            format!("{} c", crate::platform::MODIFIER_NAME)
        );
    }
}
