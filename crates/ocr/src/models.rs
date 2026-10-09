use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

pub struct Model {
    pub file: &'static str,
    /// Tried in order until one gives the file with the right hash.
    urls: &'static [&'static str],
    sha256: &'static str,
}

/// How far along a download is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Download {
    pub file: &'static str,
    pub done: u64,
    /// None when the server doesn't say how much is coming.
    pub total: Option<u64>,
}

type Watcher = Box<dyn Fn(Download) + Send>;

static WATCHER: Mutex<Option<Watcher>> = Mutex::new(None);

/// Has `f` called as a model or the runtime downloads, a few times a second
/// at most, so a window can show how far along it is instead of nothing for
/// a minute. One watcher at a time; a new one replaces the last.
pub fn watch_downloads(f: impl Fn(Download) + Send + 'static) {
    *WATCHER.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(f));
}

fn tell(download: Download) {
    if let Some(f) = WATCHER.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        f(download);
    }
}

// PP-OCRv6 exported to onnx by the RapidOCR folks, pinned by hash so a
// changed file fails loudly instead of quietly reading text differently.
// Fetched from this project's own GitHub release first (fast everywhere,
// rarely blocked), then from ModelScope, where RapidOCR publishes them (the
// same files their python package downloads).
macro_rules! urls {
    ($dir:literal, $file:literal) => {
        &[
            concat!(
                "https://github.com/xevrion/gyotaku/releases/download/models-v1/",
                $file
            ),
            concat!(
                "https://www.modelscope.cn/models/RapidAI/RapidOCR/resolve/v3.9.2/onnx/PP-OCRv6/",
                $dir,
                "/",
                $file
            ),
        ]
    };
}

// Tiny detector, small recognizer. On 25 of my own screenshots the tiny
// detector ran 4.6x faster than small and kept 99.6% of the words (its misses
// are mostly word gaps, which substring search doesn't care about). The tiny
// recognizer genuinely misreads things (mock -> meek), so rec stays small.
// Numbers are in notes.md.
pub const DET: Model = Model {
    file: "PP-OCRv6_det_tiny.onnx",
    urls: urls!("det", "PP-OCRv6_det_tiny.onnx"),
    sha256: "f42c0fbd294d95eac1a550e131b277dac97462c8025fa4b6c3cec1b7894bd3d5",
};

pub const REC: Model = Model {
    file: "PP-OCRv6_rec_small.onnx",
    urls: urls!("rec", "PP-OCRv6_rec_small.onnx"),
    sha256: "6f327246b50388f3c176ae304bd95767ea6dc0c9ae92153ef8cbe210b3c14884",
};

// PP-OCRv6 has no Devanagari recognizer yet, so this is PP-OCRv5's, the
// newest there is. Its alphabet is the Devanagari block plus digits, Latin
// letters and punctuation, so mixed Hindi and English lines read whole.
// Downloaded only once the script is turned on, from ModelScope alone until
// it's mirrored in this project's models release.
pub const DEVANAGARI: Model = Model {
    file: "devanagari_PP-OCRv5_rec_mobile.onnx",
    urls: &[
        "https://www.modelscope.cn/models/RapidAI/RapidOCR/resolve/v3.9.2/onnx/PP-OCRv5/rec/devanagari_PP-OCRv5_rec_mobile.onnx",
    ],
    sha256: "d6f0a906580e3fa6b324a318718f1f31f268b6ea8ef985f91c2012a37f52c91e",
};

/// ONNX Runtime itself: Microsoft's official build, fetched on first use like
/// the models. The Linux one is built against glibc 2.27 and GCC 5's
/// libstdc++, so it loads on anything from Ubuntu 18.04 and Debian 10 on. The
/// prebuilt that the ort crate links statically needs glibc 2.38, which CI
/// showed doesn't even link on Ubuntu 22.04 or Debian 12.
struct Runtime {
    url: &'static str,
    sha256: &'static str,
    /// Where the library sits inside Microsoft's archive.
    inner: &'static str,
}

#[cfg(windows)]
const RUNTIME_FILE: &str = "onnxruntime.dll";
// The macOS dylib ships under its versioned name, the same convention as
// the linux SONAME.
#[cfg(target_os = "macos")]
const RUNTIME_FILE: &str = "libonnxruntime.1.28.2.dylib";
#[cfg(not(any(windows, target_os = "macos")))]
const RUNTIME_FILE: &str = "libonnxruntime.so.1.28.2";

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const RUNTIME: Option<Runtime> = Some(Runtime {
    url: "https://github.com/microsoft/onnxruntime/releases/download/v1.28.2/onnxruntime-linux-x64-1.28.2.tgz",
    sha256: "d7209b8751b27b862b0c76332c2e20e203396edb5dab700ecf4bb485cf147415",
    inner: "onnxruntime-linux-x64-1.28.2/lib/libonnxruntime.so.1.28.2",
});

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const RUNTIME: Option<Runtime> = Some(Runtime {
    url: "https://github.com/microsoft/onnxruntime/releases/download/v1.28.2/onnxruntime-linux-aarch64-1.28.2.tgz",
    sha256: "f020b3d31106cc7db03889b4a5c21e7c38ce4a09ad26119c11d1ad6d3fa0ec04",
    inner: "onnxruntime-linux-aarch64-1.28.2/lib/libonnxruntime.so.1.28.2",
});

// Windows releases ship onnxruntime.dll next to the exe, so this download is
// only for builds from source. The archive is 78 MB for a 14 MB library, but
// it's once.
#[cfg(all(windows, target_arch = "x86_64"))]
const RUNTIME: Option<Runtime> = Some(Runtime {
    url: "https://github.com/microsoft/onnxruntime/releases/download/v1.28.2/onnxruntime-win-x64-1.28.2.zip",
    sha256: "c4eedd29489d5feca21866d054638416f3655bf6b18851b3b6b85c8313e95c35",
    inner: "onnxruntime-win-x64-1.28.2/lib/onnxruntime.dll",
});

#[cfg(all(windows, target_arch = "aarch64"))]
const RUNTIME: Option<Runtime> = Some(Runtime {
    url: "https://github.com/microsoft/onnxruntime/releases/download/v1.28.2/onnxruntime-win-arm64-1.28.2.zip",
    sha256: "a3ab2265e52d157ef1c4f4f82f66fc582ce12a780510aac240690ce194b58510",
    inner: "onnxruntime-win-arm64-1.28.2/lib/onnxruntime.dll",
});

// Microsoft's signed Apple Silicon build, fetched on first use like the
// Linux ones. Microsoft publishes no x86_64 macOS build (that url 404s), so
// on those Macs RUNTIME stays None and ORT_DYLIB_PATH covers them.
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const RUNTIME: Option<Runtime> = Some(Runtime {
    url: "https://github.com/microsoft/onnxruntime/releases/download/v1.28.2/onnxruntime-osx-arm64-1.28.2.tgz",
    sha256: "c4fceacfc53765d0869dc9180c31ec91054d149017a99d1e80ffe28dc79596de",
    inner: "onnxruntime-osx-arm64-1.28.2/lib/libonnxruntime.1.28.2.dylib",
});

#[cfg(not(any(
    all(any(target_os = "linux", windows), target_arch = "x86_64"),
    all(any(target_os = "linux", windows), target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "aarch64"),
)))]
const RUNTIME: Option<Runtime> = None;

pub fn models_dir() -> Result<PathBuf> {
    Ok(gyotaku_core::data_dir()?.join("models"))
}

/// Returns the path to the model, downloading it first if it isn't there yet.
pub fn ensure(model: &Model) -> Result<PathBuf> {
    let dir = models_dir()?;
    let path = dir.join(model.file);
    if path.exists() {
        return Ok(path);
    }
    eprintln!("downloading {} (first run only)", model.file);
    let mut failures = Vec::new();
    for url in model.urls {
        match fetch(model.file, url, model.sha256) {
            Ok(bytes) => {
                write_atomically(&path, &bytes)?;
                return Ok(path);
            }
            Err(e) => {
                eprintln!("{e:#}");
                failures.push(format!("{e:#}"));
            }
        }
    }
    bail!("couldn't download {}: {}", model.file, failures.join("; "))
}

/// The ONNX Runtime library to load. ORT_DYLIB_PATH wins if it's set, which
/// is how a distro package can use its own onnxruntime instead.
pub fn runtime() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("ORT_DYLIB_PATH") {
        return Ok(path.into());
    }
    // A release that ships the library next to the program (the Windows zip
    // does) needs no download at all.
    if let Some(beside) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(RUNTIME_FILE)))
        .filter(|p| p.is_file())
    {
        return Ok(beside);
    }
    let path = gyotaku_core::data_dir()?.join("runtime").join(RUNTIME_FILE);
    if path.exists() {
        return Ok(path);
    }
    let Some(runtime) = RUNTIME else {
        bail!(
            "there's no official ONNX Runtime build for this system, install onnxruntime \
             and point ORT_DYLIB_PATH at the library"
        );
    };
    eprintln!("downloading ONNX Runtime 1.28.2 (first run only)");
    let archive = fetch(RUNTIME_FILE, runtime.url, runtime.sha256)?;
    let lib = extract(&archive, runtime.inner)?
        .with_context(|| format!("{} wasn't in the ONNX Runtime archive", runtime.inner))?;
    write_atomically(&path, &lib)?;
    Ok(path)
}

#[cfg(not(windows))]
fn extract(archive: &[u8], inner: &str) -> Result<Option<Vec<u8>>> {
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    for entry in tar.entries()? {
        let mut entry = entry?;
        if entry.path()?.as_ref() == Path::new(inner) {
            let mut lib = Vec::new();
            entry.read_to_end(&mut lib)?;
            return Ok(Some(lib));
        }
    }
    Ok(None)
}

#[cfg(windows)]
fn extract(archive: &[u8], inner: &str) -> Result<Option<Vec<u8>>> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive))?;
    let Ok(mut file) = zip.by_name(inner) else {
        return Ok(None);
    };
    let mut lib = Vec::new();
    file.read_to_end(&mut lib)?;
    Ok(Some(lib))
}

// How many bytes between two reports to the watcher.
const REPORT_EVERY: usize = 256 << 10;

/// Downloads into memory and checks the hash before anything touches disk, so
/// a changed or truncated file upstream fails loudly instead of quietly
/// reading text differently.
fn fetch(file: &'static str, url: &str, sha256: &str) -> Result<Vec<u8>> {
    // With no limits a stalled connection waits forever, and never gets to
    // try the next source. Generous ones: a 78 MB archive on a slow line.
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_recv_body(Some(Duration::from_secs(600)))
        .build()
        .into();
    let response = agent
        .get(url)
        .call()
        .with_context(|| format!("downloading {url}"))?;
    let total = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    let mut body = response.into_body().into_reader();

    // Read in pieces rather than in one go, only so there's something to
    // report along the way.
    let mut bytes = Vec::with_capacity(total.unwrap_or(0).min(256 << 20) as usize);
    let mut piece = vec![0u8; 64 << 10];
    let mut told = 0;
    tell(Download {
        file,
        done: 0,
        total,
    });
    loop {
        let n = body
            .read(&mut piece)
            .with_context(|| format!("downloading {url}"))?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&piece[..n]);
        if bytes.len() - told >= REPORT_EVERY {
            told = bytes.len();
            tell(Download {
                file,
                done: told as u64,
                total,
            });
        }
    }
    tell(Download {
        file,
        done: bytes.len() as u64,
        total,
    });
    let got: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if got != sha256 {
        bail!("{url} doesn't match its checksum, expected {sha256} got {got}");
    }
    Ok(bytes)
}

/// Written next to it and renamed over, so a half written file never looks
/// like a real one.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::create_dir_all(path.parent().context("no parent folder")?)?;
    // Named per process, so two first runs at once (the app and the watcher,
    // say) can't write into the same half-finished file.
    let mut part = path.as_os_str().to_owned();
    part.push(format!(".{}.part", std::process::id()));
    let part = PathBuf::from(part);
    fs::write(&part, bytes)?;
    fs::rename(&part, path)?;
    Ok(())
}
