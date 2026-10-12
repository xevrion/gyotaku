//! The icon gyotaku keeps in the macOS menu bar and the Windows notification
//! area while it waits in the background, the way Raycast does: the one
//! visible sign that it's running, a way back to the window without the
//! shortcut, and the only place to quit it for good.

use futures::channel::mpsc::UnboundedSender;
use gpui::{App, Global};
use tray_icon::menu::accelerator::Accelerator;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use super::TrayCommand;

/// Kept for as long as the app runs; dropping it takes the icon away.
struct Tray {
    _icon: TrayIcon,
    open: MenuItem,
}

impl Global for Tray {}

pub struct Look {
    /// A builder with the icon already on it, set the way each system wants
    /// (a template image on macOS, as drawn on Windows).
    pub icon: TrayIconBuilder,
    /// Windows opens the window on a plain click and keeps the menu for a
    /// right click; the macOS menu bar opens the menu on any click.
    pub click_opens: bool,
}

/// Puts the icon up with its menu. Picking an item, or clicking the icon
/// where that opens the window, sends a command to the app.
pub fn show(look: Look, summon_key: &str, commands: UnboundedSender<TrayCommand>, cx: &mut App) {
    // Shown beside the item the way each system draws shortcuts. A menu that
    // isn't a window's menu bar never acts on it, so the key stays the
    // global hotkey's alone.
    let summon = summon_key.replace('-', "+").parse::<Accelerator>().ok();
    let open = MenuItem::with_id("open", "Open gyotaku", true, summon);
    let settings = MenuItem::with_id("settings", "Settings…", true, None);
    let quit = MenuItem::with_id("quit", "Quit gyotaku", true, None);
    let Ok(menu) = Menu::with_items(&[&open, &settings, &PredefinedMenuItem::separator(), &quit])
    else {
        return;
    };
    let built = look
        .icon
        .with_tooltip("gyotaku")
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(!look.click_opens)
        .build();
    let tray = match built {
        Ok(tray) => tray,
        Err(e) => {
            log::warn!("couldn't show the tray icon: {e}");
            return;
        }
    };

    let menu_commands = commands.clone();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        let command = match e.id.as_ref() {
            "open" => TrayCommand::Open,
            "settings" => TrayCommand::Settings,
            "quit" => TrayCommand::Quit,
            _ => return,
        };
        let _ = menu_commands.unbounded_send(command);
    }));
    if look.click_opens {
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = e
            {
                let _ = commands.unbounded_send(TrayCommand::Open);
            }
        }));
    }
    cx.set_global(Tray { _icon: tray, open });
}

/// Updates the shortcut shown next to "Open gyotaku" in the menu.
pub fn update_summon(summon_key: &str, cx: &mut App) {
    if let Some(tray) = cx.try_global::<Tray>() {
        let summon = summon_key.replace('-', "+").parse::<Accelerator>().ok();
        let _ = tray.open.set_accelerator(summon);
    }
}

/// A PNG made into an icon, decoded once at start. Bundled, so failing to
/// decode is a build mistake; it's logged rather than taking the app down.
pub fn icon(png: &[u8]) -> Option<Icon> {
    let image = image::load_from_memory(png).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    let icon = Icon::from_rgba(image.into_raw(), width, height).ok();
    if icon.is_none() {
        log::warn!("couldn't decode the tray icon");
    }
    icon
}
