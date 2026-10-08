//! OCR for screenshots: PP-OCRv6 detection and recognition models running on
//! ONNX Runtime, with the pre and post processing written for this project.

mod det;
mod models;
mod rec;

use std::path::Path;
use std::sync::OnceLock;

use anyhow::{Context, Result};
use gyotaku_core::{Line, Rect, Script};
use image::RgbImage;
use ort::session::{Session, builder::GraphOptimizationLevel};

pub use models::models_dir;

// Lines the recognizer isn't at least this sure about are mostly icons read as
// letters. Paddle's default.
const MIN_SCORE: f32 = 0.5;

pub struct Ocr {
    det: Session,
    rec: Session,
    alphabet: Vec<String>,
    /// Recognizers for the scripts turned on in settings, on top of the
    /// default one.
    extra: Vec<Recognizer>,
}

struct Recognizer {
    script: Script,
    session: Session,
    alphabet: Vec<String>,
}

impl Ocr {
    /// Loads the models, downloading them on first use. `threads` is how many
    /// cores one screenshot may use, `scripts` the extra writing systems to
    /// read.
    pub fn new(threads: usize, scripts: &[Script]) -> Result<Self> {
        load_runtime()?;
        let det = load(&models::DET, threads)?;
        let rec = load(&models::REC, threads)?;
        let alphabet = rec::alphabet(&rec)?;
        let extra = scripts
            .iter()
            .map(|&script| {
                let session = load(model_for(script), threads)?;
                let alphabet = rec::alphabet(&session)?;
                Ok(Recognizer {
                    script,
                    session,
                    alphabet,
                })
            })
            .collect::<Result<_>>()?;
        Ok(Self {
            det,
            rec,
            alphabet,
            extra,
        })
    }

    pub fn read(&mut self, img: &RgbImage) -> Result<Vec<Line>> {
        // Nothing readable fits in a sliver, and the models would choke on it.
        if img.width() < 8 || img.height() < 8 {
            return Ok(Vec::new());
        }
        let regions = det::detect(&mut self.det, img)?;
        let mut texts = rec::recognize(&mut self.rec, &self.alphabet, img, &regions)?;

        // Only the lines the default reader stumbled on get a second read,
        // which keeps it to about a fifth more time on a typical screenshot
        // instead of nearly double.
        for extra in &mut self.extra {
            let unsure: Vec<usize> = (0..regions.len())
                .filter(|&i| second_read(extra.script, &texts[i]))
                .collect();
            if unsure.is_empty() {
                continue;
            }
            let picked: Vec<det::Region> = unsure.iter().map(|&i| regions[i]).collect();
            let again = rec::recognize(&mut extra.session, &extra.alphabet, img, &picked)?;
            for (&i, read) in unsure.iter().zip(again) {
                if better(extra.script, &read, &texts[i]) {
                    texts[i] = read;
                }
            }
        }

        let (w, h) = (img.width() as f32, img.height() as f32);
        Ok(regions
            .iter()
            .zip(texts)
            .filter(|(_, read)| read.score >= MIN_SCORE && worth_keeping(&read.text))
            .map(|(r, read)| Line {
                text: read.text.trim().to_owned(),
                rect: Rect {
                    x: r.x0 as f32 / w,
                    y: r.y0 as f32 / h,
                    w: r.width() as f32 / w,
                    h: r.height() as f32 / h,
                },
                score: read.score,
            })
            .collect())
    }
}

fn model_for(script: Script) -> &'static models::Model {
    match script {
        Script::Devanagari => &models::DEVANAGARI,
        Script::Bengali => &models::BENGALI,
    }
}

fn in_script(script: Script, c: char) -> bool {
    match script {
        Script::Devanagari => ('\u{0900}'..='\u{097F}').contains(&c),
        Script::Bengali => ('\u{0980}'..='\u{09FF}').contains(&c),
    }
}

// Hindi read by the default reader comes back as a few confident-looking Latin
// letters (पुणे महानगरपालिका as "yut HETATRUTOT", 0.66) or as nothing. On
// screenshots with no Hindi at all, about a fifth of lines fall under this,
// mostly icons.
const RECHECK_BELOW: f32 = 0.9;

// A line that's half English and half Hindi reads the English confidently and
// leaves the Hindi out, "Order #4021 का स्टेटस" as "Order #4021" at 0.95. What
// gives it away is the gap: 24 steps of nothing where the Hindi was. Lines
// that read in full have gaps of 2 to 5, and on 441 confident lines from real
// screenshots only 25 had one of 8 or more.
const GAP_STEPS: usize = 8;

/// Whether a line read by the default reader is worth reading again as
/// `script`.
fn second_read(script: Script, read: &rec::Read) -> bool {
    read.score < RECHECK_BELOW
        || read.gap >= GAP_STEPS
        || read.text.chars().any(|c| in_script(script, c))
}

/// Whether the second read should replace the first: only when it's surer
/// and found the script it's for. Its alphabet has Latin letters too, but the
/// default reader stays in charge of everything else, so turning a script on
/// never changes how an English or Chinese line reads. Without this, a price
/// "₹1,250.00" came back as "ऱ 1,250.00", and Chinese as stray letters.
fn better(script: Script, new: &rec::Read, old: &rec::Read) -> bool {
    new.score > old.score && new.text.chars().any(|c| in_script(script, c))
}

/// UI icons get detected as text and come back as one confident character: a
/// bell reads as 白, a grid as 品, a hamburger menu as 三. A lone character is
/// never something anyone searches for, so those go.
fn worth_keeping(text: &str) -> bool {
    text.trim().chars().count() > 1
}

/// ONNX Runtime is loaded at run time rather than linked in, see
/// `models::runtime` for why. Once per process.
fn load_runtime() -> Result<()> {
    static LOADED: OnceLock<()> = OnceLock::new();
    if LOADED.get().is_some() {
        return Ok(());
    }
    let lib = models::runtime()?;
    ort::init_from(&lib)
        .map_err(|e| anyhow::anyhow!("loading {}: {e}", lib.display()))?
        .commit();
    let _ = LOADED.set(());
    Ok(())
}

/// A model file that won't load is damaged (a disk filling up mid-write, a
/// file someone edited): it's deleted, so the next start downloads it again
/// instead of failing the same way forever.
fn load(model: &models::Model, threads: usize) -> Result<Session> {
    let path = models::ensure(model)?;
    session(&path, threads).inspect_err(|_| {
        let _ = std::fs::remove_file(&path);
    })
}

fn session(model: &Path, threads: usize) -> Result<Session> {
    let build = || -> ort::Result<Session> {
        // No arena and no memory pattern: every screenshot is a different size,
        // so both just hold on to the peak of the biggest one forever.
        Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(threads)?
            .with_execution_providers([ort::ep::CPU::default()
                .with_arena_allocator(false)
                .build()])?
            .with_memory_pattern(false)?
            .commit_from_file(model)
    };
    build().with_context(|| format!("loading {}", model.display()))
}

pub fn load_image(path: &Path) -> Result<RgbImage> {
    let img = open_image(path).with_context(|| format!("decoding {}", path.display()))?;
    Ok(img.into_rgb8())
}

/// Anything that would need more than this to decode is refused rather than
/// loaded. 256 MB is a 64 megapixel image, far past any screen, and a stray
/// panorama or giant scan shouldn't be able to push a small laptop into swap.
const MAX_DECODE_BYTES: u64 = 256 * 1024 * 1024;

/// Opens an image by what's in the file, not its extension (a `.png` that's
/// really a jpeg still works), with a cap on how much memory decoding it may
/// take.
pub fn open_image(path: &Path) -> Result<image::DynamicImage> {
    let mut reader = image::ImageReader::open(path)?.with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    Ok(reader.decode()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rec::Read;

    fn read(text: &str, score: f32, gap: usize) -> Read {
        Read {
            text: text.into(),
            score,
            gap,
        }
    }

    const HINDI: Script = Script::Devanagari;
    const BANGLA: Script = Script::Bengali;

    // The readings below are what the two models gave on real test images.
    #[test]
    fn confident_whole_lines_are_left_alone() {
        assert!(!second_read(
            HINDI,
            &read("Your invoice from Fern & Co", 0.981, 2)
        ));
        assert!(!second_read(HINDI, &read("请输入验证码 482913", 0.99, 4)));
    }

    #[test]
    fn unsure_lines_and_lines_with_holes_get_a_second_read() {
        assert!(second_read(HINDI, &read("yut HETATRUTOT", 0.655, 4)));
        assert!(second_read(HINDI, &read("Order #4021 ", 0.945, 24)));
        assert!(second_read(HINDI, &read("", 0.0, 48)));
    }

    #[test]
    fn hindi_replaces_what_the_default_reader_made_of_it() {
        let old = read("Order #4021 ", 0.945, 24);
        let new = read("Order #4021 का स्टेटस", 0.975, 2);
        assert!(better(HINDI, &new, &old));
    }

    #[test]
    fn the_default_reader_keeps_everything_else() {
        // Surer, but no Devanagari in it: an English line stays as read.
        let old = read("Gate 14· Seat 22A·PNR K7Q2ZD", 0.960, 3);
        assert!(!better(
            HINDI,
            &read("Gate 14 · Seat 22A · PNR K7Q2ZD", 0.983, 3),
            &old
        ));
        // A stray Devanagari letter, but less sure: the rupee sign stays.
        let old = read("₹1,250.00", 0.943, 5);
        assert!(!better(HINDI, &read("ऱ 1,250.00", 0.888, 5), &old));
    }

    // What the two readers gave for one line of a screenshot of Bengali
    // Wikipedia. The scores are theirs; the gaps weren't noted and don't
    // matter here, the first read is unsure enough on its own.
    #[test]
    fn bangla_replaces_what_the_default_reader_made_of_it() {
        let old = read("GERNAGTGGARANA", 0.51, 0);
        assert!(second_read(BANGLA, &old));
        let new = read("অংশগ্রহণ করুন ও জিতে নিন আকর্ষণীয় পুরস্কার|", 0.98, 0);
        assert!(better(BANGLA, &new, &old));
        // Each script only answers for its own letters.
        assert!(!better(HINDI, &new, &old));
    }
}
