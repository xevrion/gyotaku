use std::os::windows::fs::MetadataExt as _;
use std::sync::mpsc;
use std::time::Duration;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::DataExchange::{
    AddClipboardFormatListener, CloseClipboard, GetClipboardData, IsClipboardFormatAvailable,
    OpenClipboard, RegisterClipboardFormatW, RemoveClipboardFormatListener,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentThread, GetCurrentThreadId, IDLE_PRIORITY_CLASS, SetPriorityClass,
    SetThreadPriority, THREAD_PRIORITY_HIGHEST,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, HWND_MESSAGE,
    MSG, PostThreadMessageW, RegisterClassW, WM_CLIPBOARDUPDATE, WM_QUIT, WNDCLASSW,
};

/// Idle priority: the reader only gets cpu nobody else wants. Not
/// "background mode", which also caps the working set at a few tens of MB,
/// and reading a screenshot needs a few hundred.
pub fn become_idle() {
    unsafe {
        SetPriorityClass(GetCurrentProcess(), IDLE_PRIORITY_CLASS);
    }
}

/// The Windows heap gives freed pages back by itself.
pub fn release_memory() {}

/// ReadDirectoryChangesW overflows when a lot changes at once (a few hundred
/// photos copied in), and notify passes that on as nothing at all, or stops
/// watching the folder. So the reader also looks over its folders now and
/// then, the way it does on macOS.
pub const RESCAN_EVERY: Option<Duration> = Some(Duration::from_secs(180));

pub fn denied(folder: &str) -> String {
    format!("can't read {folder}, access denied")
}

/// The app starts the reader with its output going to watch.log already.
pub fn keep_a_log() {}

/// AC line status 0 means unplugged. Desktops report 1, or 255 (unknown)
/// when there's no battery at all, and neither counts.
pub fn on_battery() -> bool {
    let mut status: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
    (unsafe { GetSystemPowerStatus(&mut status) } != 0) && status.ACLineStatus == 0
}

const FILE_ATTRIBUTE_OFFLINE: u32 = 0x0000_1000;
const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x0004_0000;
const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;

pub fn only_in_the_cloud(meta: &std::fs::Metadata) -> bool {
    meta.file_attributes()
        & (FILE_ATTRIBUTE_OFFLINE
            | FILE_ATTRIBUTE_RECALL_ON_OPEN
            | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS)
        != 0
}

// The clipboard. Windows tells every window registered with
// AddClipboardFormatListener when the clipboard changes; a message-only
// window is enough, nothing is shown. The reader runs in the signed-in
// user's session (the app starts it), so it sees that user's clipboard.

pub struct ClipboardWatch {
    /// The thread that owns the window, told to quit when this is dropped.
    thread: u32,
}

impl Drop for ClipboardWatch {
    fn drop(&mut self) {
        unsafe { PostThreadMessageW(self.thread, WM_QUIT, 0, 0) };
    }
}

pub fn watch_clipboard(found: mpsc::Sender<Vec<u8>>) -> ClipboardWatch {
    let (started, thread) = mpsc::channel();
    let _ = std::thread::Builder::new()
        .name("clipboard watch".into())
        .spawn(move || {
            let _ = started.send(unsafe { GetCurrentThreadId() });
            if let Err(why) = unsafe { listen(found) } {
                eprintln!("clipboard: {why}, copied images aren't saved");
            }
        });
    ClipboardWatch {
        thread: thread.recv().unwrap_or(0),
    }
}

thread_local! {
    static FOUND: std::cell::RefCell<Option<mpsc::Sender<Vec<u8>>>> =
        const { std::cell::RefCell::new(None) };
}

/// Runs the window's message loop until the watch is dropped.
unsafe fn listen(found: mpsc::Sender<Vec<u8>>) -> Result<(), &'static str> {
    unsafe {
        FOUND.with(|f| *f.borrow_mut() = Some(found));
        let class: Vec<u16> = "gyotaku clipboard\0".encode_utf16().collect();
        let instance = GetModuleHandleW(std::ptr::null());
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(on_message);
        wc.hInstance = instance;
        wc.lpszClassName = class.as_ptr();
        // Fails harmlessly when a watch from before (turned off, then on
        // again) registered it already.
        RegisterClassW(&wc);
        let window = CreateWindowExW(
            0,
            class.as_ptr(),
            std::ptr::null(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if window.is_null() {
            return Err("couldn't make a window to listen with");
        }
        if AddClipboardFormatListener(window) == 0 {
            DestroyWindow(window);
            return Err("couldn't listen to the clipboard");
        }
        // The reader runs at idle priority. This thread only copies bytes
        // out when the clipboard changes, so it can afford to be the
        // quickest in the process, before the app that copied replaces or
        // drops what it put there. Saving happens on the idle thread.
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            DispatchMessageW(&msg);
        }
        RemoveClipboardFormatListener(window);
        DestroyWindow(window);
        Ok(())
    }
}

unsafe extern "system" fn on_message(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_CLIPBOARDUPDATE {
        if let Some(bytes) = unsafe { clipboard_image() } {
            FOUND.with(|f| {
                if let Some(found) = &*f.borrow() {
                    let _ = found.send(bytes);
                }
            });
        }
        return 0;
    }
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

const CF_DIB: u32 = 8;
const CF_HDROP: u32 = 15;

fn format(name: &str) -> u32 {
    let name: Vec<u16> = name.encode_utf16().chain([0]).collect();
    unsafe { RegisterClipboardFormatW(name.as_ptr()) }
}

/// The image on the clipboard, as PNG when the app that copied offered one
/// (browsers, the Snipping Tool) and as a BMP made from its bitmap
/// otherwise. Files copied in Explorer are files already, and anything an
/// app asked clipboard history and monitors to skip (password managers do)
/// is skipped here too.
unsafe fn clipboard_image() -> Option<Vec<u8>> {
    unsafe {
        let private = format("ExcludeClipboardContentFromMonitorProcessing");
        if IsClipboardFormatAvailable(CF_HDROP) != 0 || IsClipboardFormatAvailable(private) != 0 {
            return None;
        }
        let png = format("PNG");
        let wanted = if IsClipboardFormatAvailable(png) != 0 {
            png
        } else if IsClipboardFormatAvailable(CF_DIB) != 0 {
            CF_DIB
        } else {
            return None;
        };
        // Whoever copied can still be holding it open for a moment.
        let opened = (0..10).any(|_| {
            let ok = OpenClipboard(std::ptr::null_mut()) != 0;
            if !ok {
                std::thread::sleep(Duration::from_millis(30));
            }
            ok
        });
        if !opened {
            return None;
        }
        let bytes = read(wanted);
        CloseClipboard();
        let bytes = bytes?;
        if wanted == CF_DIB {
            bmp_from_dib(&bytes)
        } else {
            Some(bytes)
        }
    }
}

/// Copies one format out. The clipboard keeps owning the memory.
unsafe fn read(format: u32) -> Option<Vec<u8>> {
    unsafe {
        let handle = GetClipboardData(format);
        if handle.is_null() {
            return None;
        }
        let size = GlobalSize(handle);
        let at = GlobalLock(handle) as *const u8;
        if at.is_null() || size == 0 {
            return None;
        }
        let bytes = std::slice::from_raw_parts(at, size).to_vec();
        GlobalUnlock(handle);
        Some(bytes)
    }
}

/// A bitmap on the clipboard is a BMP file without its 14 byte file header,
/// so putting one in front makes it something any decoder reads. The header
/// says where the pixels start: after the info header, the colour masks
/// when there are any, and the palette.
fn bmp_from_dib(dib: &[u8]) -> Option<Vec<u8>> {
    const BI_BITFIELDS: u32 = 3;
    const BI_ALPHABITFIELDS: u32 = 6;
    let u32_at = |at: usize| Some(u32::from_le_bytes(dib.get(at..at + 4)?.try_into().ok()?));
    let header = u32_at(0)?;
    let bits = u16::from_le_bytes(dib.get(14..16)?.try_into().ok()?);
    let compression = u32_at(16)?;
    let used = u32_at(32)?;
    let masks = match (header, compression) {
        (40, BI_BITFIELDS) => 12,
        (40, BI_ALPHABITFIELDS) => 16,
        _ => 0,
    };
    let palette = match (used, bits) {
        (0, 1..=8) => 1 << bits,
        (0, _) => 0,
        _ => used,
    };
    let offset = 14 + header + masks + palette * 4;
    let size = 14 + u32::try_from(dib.len()).ok()?;
    let mut bmp = Vec::with_capacity(size as usize);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&size.to_le_bytes());
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&offset.to_le_bytes());
    bmp.extend_from_slice(dib);
    Some(bmp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clipboard_bitmap_becomes_a_readable_bmp() {
        // 3 x 2, 32 bit, bottom-up, the way the window itself copies one.
        let (w, h) = (3u32, 2u32);
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&(w as i32).to_le_bytes());
        dib.extend_from_slice(&(h as i32).to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&32u16.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());
        dib.extend_from_slice(&(w * h * 4).to_le_bytes());
        dib.extend_from_slice(&[0; 16]);
        // Bottom row red, top row blue, as blue green red alpha.
        dib.extend_from_slice(&[0, 0, 255, 255].repeat(3));
        dib.extend_from_slice(&[255, 0, 0, 255].repeat(3));

        let image = image::load_from_memory(&bmp_from_dib(&dib).unwrap())
            .unwrap()
            .to_rgb8();
        assert_eq!(image.dimensions(), (3, 2));
        assert_eq!(image.get_pixel(0, 0).0, [0, 0, 255]);
        assert_eq!(image.get_pixel(2, 1).0, [255, 0, 0]);
        assert_eq!(bmp_from_dib(&dib[..10]), None);
    }
}
