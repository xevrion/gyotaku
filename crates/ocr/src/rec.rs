//! Text recognition. Each detected line is cut out, squashed to 48 px tall,
//! and the model returns, for every thin vertical slice of it, a probability
//! for each of its ~18k characters. CTC decoding turns that into a string.

use anyhow::{Context, Result};
use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer};
use image::RgbImage;
use ort::{session::Session, value::Tensor};

use crate::det::Region;

const HEIGHT: u32 = 48;
/// Paddle never makes a batch narrower than 320 px, short words get padded,
/// and its models were trained that way. A model that wasn't can be given
/// short lines at their own width, which for one as heavy as the Bengali
/// reader is most of the cost of a screenshot full of short labels.
pub const PADDLE_MIN_WIDTH: u32 = 320;

// The output is batch * steps * 18710 floats, one step per 8 px of width. That
// vocabulary is what makes it big: 32 long lines at once is a quarter gigabyte
// just for the scores. So batches are capped by total width, which keeps that
// tensor around 15 MB, instead of by a line count. Going from 6400 down to
// this was no slower on my test set.
const BATCH_WIDTH: u32 = 1600;
const MAX_BATCH: usize = 32;

/// The model's own alphabet, stored in the onnx metadata. Index 0 is the CTC
/// blank and the space character is appended at the end, same as paddle.
pub fn alphabet(session: &Session) -> Result<Vec<String>> {
    let meta = session.metadata()?;
    let chars = meta
        .custom("character")
        .context("rec model has no character list")?;
    let mut out = vec![String::new()];
    out.extend(chars.lines().map(str::to_owned));
    out.push(" ".into());
    Ok(out)
}

/// One line as the model read it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Read {
    pub text: String,
    /// Mean confidence of the characters kept.
    pub score: f32,
    /// The longest stretch of the line, in model steps (8 px each at 48 px
    /// tall), where the model saw nothing it could read. Glyphs outside its
    /// alphabet come out as exactly that, so a long one inside a line that
    /// otherwise read fine means something there went unread.
    pub gap: usize,
}

pub fn recognize(
    session: &mut Session,
    alphabet: &[String],
    img: &RgbImage,
    regions: &[Region],
    min_width: u32,
) -> Result<Vec<Read>> {
    let mut results = read_lines(session, alphabet, img, regions, Turn::Left, min_width)?;

    // A column turned left reads vertical Japanese and Chinese, top to
    // bottom. A label turned on its side to read upwards, like a chart's
    // axis, needs turning the other way: "Revenue in thousands" read as
    // "Re nuds" at 0.78 one way and in full at 1.00 the other.
    let unsure: Vec<usize> = (0..regions.len())
        .filter(|&i| tall(&regions[i]) && results[i].score < SURE)
        .collect();
    if !unsure.is_empty() {
        let columns: Vec<Region> = unsure.iter().map(|&i| regions[i]).collect();
        let other = read_lines(session, alphabet, img, &columns, Turn::Right, min_width)?;
        for (&i, read) in unsure.iter().zip(other) {
            if read.score > results[i].score {
                results[i] = read;
            }
            // Neither way read it well: a narrow icon more often than a
            // column (a 17x30 px one read as "oo" at 0.59). Before columns
            // were turned, those read as nothing, and they still do.
            if results[i].score < COLUMN_MIN {
                results[i] = Read::default();
            }
        }
    }
    Ok(results)
}

// A column read at least this surely the first way isn't tried the other.
const SURE: f32 = 0.9;
// Real columns read at 1.00 both ways they were tried.
const COLUMN_MIN: f32 = 0.8;

/// Which way a column of text is turned to lie down as a line.
#[derive(Clone, Copy)]
enum Turn {
    /// Counterclockwise, so the top of the column is the start of the line.
    Left,
    /// Clockwise, so the bottom is.
    Right,
}

fn read_lines(
    session: &mut Session,
    alphabet: &[String],
    img: &RgbImage,
    regions: &[Region],
    turn: Turn,
    min_width: u32,
) -> Result<Vec<Read>> {
    let mut results = vec![Read::default(); regions.len()];

    // Similar widths go in the same batch so little of it is padding.
    let ratio = aspect;
    let mut order: Vec<usize> = (0..regions.len()).collect();
    order.sort_by(|&a, &b| ratio(&regions[a]).total_cmp(&ratio(&regions[b])));

    let mut resizer = Resizer::new();
    let mut start = 0;
    while start < order.len() {
        // Sorted ascending, so the last line in a batch sets its width.
        let mut end = start + 1;
        while end < order.len()
            && end - start < MAX_BATCH
            && (end - start + 1) as u32 * batch_width(ratio(&regions[order[end]]), min_width)
                <= BATCH_WIDTH
        {
            end += 1;
        }
        let batch = &order[start..end];
        let width = batch_width(ratio(&regions[batch[batch.len() - 1]]), min_width);

        let input = batch_tensor(&mut resizer, img, regions, batch, width, turn)?;
        let outputs = session.run(ort::inputs![input])?;
        let (shape, probs) = outputs[0].try_extract_tensor::<f32>()?;
        let (steps, classes) = (shape[1] as usize, shape[2] as usize);

        for (n, &i) in batch.iter().enumerate() {
            let slice = &probs[n * steps * classes..(n + 1) * steps * classes];
            // Only the steps over the line itself, not the padding after it.
            // Not every model steps a whole number of pixels: paddle's take
            // 8, the Bengali one a little under 3.
            let used = (line_width(&regions[i], width) as usize * steps)
                .div_ceil(width as usize)
                .min(steps);
            results[i] = ctc_decode(&slice[..used * classes], classes, alphabet);
        }
        start = end;
    }
    Ok(results)
}

fn batch_width(max_ratio: f32, min_width: u32) -> u32 {
    ((HEIGHT as f32 * max_ratio).ceil() as u32).max(min_width)
}

/// How wide a line is once scaled to the model's height, within its batch.
fn line_width(r: &Region, batch_width: u32) -> u32 {
    ((HEIGHT as f32 * aspect(r)).ceil() as u32).clamp(1, batch_width)
}

// A box this much taller than wide is a column of text: vertical Japanese or
// Chinese, or a label turned on its side. Paddle's threshold.
const TALL: f32 = 1.5;

fn tall(r: &Region) -> bool {
    r.height() as f32 >= TALL * r.width() as f32
}

/// Length over thickness of the line as the model will see it, turned
/// upright first if it's a column.
fn aspect(r: &Region) -> f32 {
    let (w, h) = (r.width() as f32, r.height() as f32);
    if tall(r) { h / w } else { w / h }
}

fn batch_tensor(
    resizer: &mut Resizer,
    img: &RgbImage,
    regions: &[Region],
    batch: &[usize],
    width: u32,
    turn: Turn,
) -> Result<Tensor<f32>> {
    let plane = (HEIGHT * width) as usize;
    // Padding stays 0.0, which is mid grey after normalisation, like paddle.
    let mut data = vec![0f32; batch.len() * 3 * plane];

    for (n, &i) in batch.iter().enumerate() {
        let r = regions[i];
        let w = line_width(&r, width);
        let mut line = RgbImage::new(w, HEIGHT);
        let opts = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Bilinear));
        if tall(&r) {
            let column = image::imageops::crop_imm(img, r.x0, r.y0, r.width(), r.height());
            let turned = match turn {
                Turn::Left => image::imageops::rotate270(&*column),
                Turn::Right => image::imageops::rotate90(&*column),
            };
            resizer.resize(&turned, &mut line, &opts)?;
        } else {
            let opts = opts.crop(
                r.x0 as f64,
                r.y0 as f64,
                r.width() as f64,
                r.height() as f64,
            );
            resizer.resize(img, &mut line, &opts)?;
        }

        let base = n * 3 * plane;
        for (x, y, px) in line.enumerate_pixels() {
            let at = (y * width + x) as usize;
            for c in 0..3 {
                data[base + c * plane + at] = px[c] as f32 / 127.5 - 1.0;
            }
        }
    }
    Ok(Tensor::from_array((
        [batch.len(), 3, HEIGHT as usize, width as usize],
        data,
    ))?)
}

/// Greedy CTC: take the likeliest class at every step, collapse runs of the
/// same class, drop blanks. "hh-e-ll-ll-o" becomes "hello", and the blank
/// between the two "ll" runs is what keeps the double l.
fn ctc_decode(probs: &[f32], classes: usize, alphabet: &[String]) -> Read {
    let mut text = String::new();
    let (mut sum, mut kept) = (0.0, 0);
    let mut prev = 0;
    let (mut blanks, mut gap) = (0, 0);

    for step in probs.chunks_exact(classes) {
        let (best, p) = step
            .iter()
            .copied()
            .enumerate()
            .fold(
                (0, f32::MIN),
                |acc, (i, p)| if p > acc.1 { (i, p) } else { acc },
            );
        if best == 0 {
            blanks += 1;
            gap = gap.max(blanks);
        } else {
            blanks = 0;
        }
        if best != 0
            && best != prev
            && let Some(ch) = alphabet.get(best)
        {
            text.push_str(ch);
            sum += p;
            kept += 1;
        }
        prev = best;
    }

    let score = if kept == 0 { 0.0 } else { sum / kept as f32 };
    Read { text, score, gap }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alphabet() -> Vec<String> {
        ["", "h", "e", "l", "o", " "]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    // one row per step, a 1.0 on the winning class
    fn steps(winners: &[usize]) -> Vec<f32> {
        let mut out = vec![0.0; winners.len() * 6];
        for (t, &w) in winners.iter().enumerate() {
            out[t * 6 + w] = 1.0;
        }
        out
    }

    #[test]
    fn collapses_runs_and_keeps_doubled_letters() {
        let probs = steps(&[1, 1, 0, 2, 0, 3, 3, 0, 3, 0, 4, 4]);
        let read = ctc_decode(&probs, 6, &alphabet());
        assert_eq!((read.text.as_str(), read.score), ("hello", 1.0));
    }

    #[test]
    fn all_blank_is_empty_with_zero_score() {
        let read = ctc_decode(&steps(&[0, 0, 0]), 6, &alphabet());
        assert_eq!((read.text.as_str(), read.score, read.gap), ("", 0.0, 3));
    }

    #[test]
    fn score_is_the_mean_of_kept_steps() {
        let mut probs = steps(&[1, 2]);
        probs[1] = 0.5;
        probs[6 + 2] = 0.7;
        let read = ctc_decode(&probs, 6, &alphabet());
        assert_eq!(read.text, "he");
        assert!((read.score - 0.6).abs() < 1e-6);
    }

    #[test]
    fn the_gap_is_the_longest_unread_stretch() {
        // "he", four steps of nothing, "lo": the four are the gap, not the
        // single blanks between letters.
        let read = ctc_decode(&steps(&[1, 0, 2, 0, 0, 0, 0, 3, 0, 4]), 6, &alphabet());
        assert_eq!((read.text.as_str(), read.gap), ("helo", 4));
    }

    #[test]
    fn columns_are_measured_lying_down() {
        let line = Region {
            x0: 0,
            y0: 0,
            x1: 300,
            y1: 30,
        };
        let column = Region {
            x0: 0,
            y0: 0,
            x1: 30,
            y1: 300,
        };
        // A digit or two is taller than wide, but not by this much.
        let digits = Region {
            x0: 0,
            y0: 0,
            x1: 24,
            y1: 30,
        };
        assert!(!tall(&line) && tall(&column) && !tall(&digits));
        assert_eq!(aspect(&line), 10.0);
        assert_eq!(aspect(&column), 10.0);
        assert_eq!(line_width(&column, 1600), 480);
    }

    #[test]
    fn batch_width_never_goes_under_paddles_minimum() {
        assert_eq!(batch_width(0.5, PADDLE_MIN_WIDTH), 320);
        assert_eq!(batch_width(10.0, PADDLE_MIN_WIDTH), 480);
        // A model that takes short lines as they are.
        assert_eq!(batch_width(0.5, 96), 96);
    }
}
