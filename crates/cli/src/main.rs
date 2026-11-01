mod clipboard;
mod indexer;
mod platform;
mod watch;

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::Result;
use clap::{Parser, Subcommand};
use gyotaku_core::{Config, Index, Script};
use gyotaku_ocr::Ocr;

use indexer::{Indexer, Outcome};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Find screenshots containing every word of the query
    Search {
        #[arg(required = true)]
        query: Vec<String>,
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
    },
    /// Index every image in these folders (default: the ones in your config) and exit
    Index {
        dirs: Vec<PathBuf>,
        /// Cores to use per screenshot (default: from your config)
        #[arg(long)]
        threads: Option<usize>,
    },
    /// Index everything, then keep watching for new screenshots, at idle
    /// priority. Without folders it follows your config as it changes.
    Watch {
        dirs: Vec<PathBuf>,
        #[arg(long)]
        threads: Option<usize>,
    },
    /// Show where the index lives and how much is in it
    Stats,
    /// Read the text out of one image and print it, without indexing anything
    Ocr {
        image: PathBuf,
        /// Also print each line's box (x y w h, as fractions of the image) and score
        #[arg(long)]
        boxes: bool,
        #[arg(long, default_value_t = gyotaku_core::default_threads())]
        threads: usize,
        /// Also read this script, `--script devanagari` or `bengali` (default: the
        /// ones in your config)
        #[arg(long = "script")]
        scripts: Vec<String>,
    },
    /// Run by the clipboard watcher for each image copied, see clipboard::hand_over
    #[command(hide = true)]
    ClipboardIncoming { dir: PathBuf },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Search { query, limit } => {
            for hit in Index::open_default()?.search(&query.join(" "), limit)? {
                if hit.near {
                    println!("{} (near match)", hit.path.display());
                } else {
                    println!("{}", hit.path.display());
                }
                for line in &hit.lines {
                    println!("    {}", line.text);
                }
            }
        }
        Command::Index { dirs, threads } => {
            let config = Config::load_or_default();
            let dirs = if dirs.is_empty() {
                config.reading_folders()
            } else {
                dirs
            };
            index(&dirs, threads.unwrap_or(config.threads))?
        }
        Command::Watch { dirs, threads } => {
            if let Err(e) = watch::run((!dirs.is_empty()).then_some(dirs), threads) {
                // The reader usually runs hidden, so the window is where this
                // gets seen.
                gyotaku_core::status::set(&format!("stopped reading: {e:#}"));
                return Err(e);
            }
        }
        Command::Stats => {
            let index = Index::open_default()?;
            println!(
                "index   {}",
                gyotaku_core::data_dir()?.join("index.db").display()
            );
            println!("models  {}", gyotaku_ocr::models_dir()?.display());
            println!(
                "shots   {} searchable, {} looked at",
                index.visible_len()?,
                index.len()?
            );
        }
        Command::Ocr {
            image,
            boxes,
            threads,
            scripts,
        } => {
            let scripts = if scripts.is_empty() {
                Config::load_or_default().scripts()
            } else {
                scripts
                    .iter()
                    .map(|n| {
                        Script::from_name(n).ok_or_else(|| {
                            let known: Vec<&str> = Script::ALL.iter().map(|s| s.name()).collect();
                            anyhow::anyhow!("no script called {n}, there's {}", known.join(", "))
                        })
                    })
                    .collect::<Result<_>>()?
            };
            ocr(&image, boxes, threads, &scripts)?
        }
        Command::ClipboardIncoming { dir } => clipboard::hand_over(&dir)?,
    }
    Ok(())
}

fn index(dirs: &[PathBuf], threads: usize) -> Result<()> {
    let mut indexer = Indexer::new(threads, &Config::load_or_default().scripts())?;
    let files = indexer::scan(dirs);
    let total = files.len();
    let started = Instant::now();
    let mut done = 0;

    for (i, path) in files.iter().enumerate() {
        match indexer.index_file(path) {
            Ok(Outcome::Unchanged | Outcome::Thumbnail) => continue,
            Ok(Outcome::NotYet) => {
                eprintln!(
                    "[{:>5}/{total}] still being written, run again later: {}",
                    i + 1,
                    path.display()
                )
            }
            Ok(Outcome::Indexed { lines, took }) => {
                done += 1;
                let eta = started.elapsed() / done * (total - i - 1) as u32;
                eprintln!(
                    "[{:>5}/{total}] {lines:>3} lines {took:>6.1?}  eta {}  {}",
                    i + 1,
                    human(eta.as_secs()),
                    path.display()
                );
            }
            Ok(Outcome::Hidden(why)) => {
                eprintln!("[{:>5}/{total}] skipped {}: {why}", i + 1, path.display())
            }
            Err(e) => eprintln!("[{:>5}/{total}] failed {}: {e:#}", i + 1, path.display()),
        }
    }
    eprintln!(
        "done, {} searchable, {done} new in {}",
        indexer.index.visible_len()?,
        human(started.elapsed().as_secs())
    );
    Ok(())
}

fn human(secs: u64) -> String {
    match secs {
        0..60 => format!("{secs}s"),
        60..3600 => format!("{}m{:02}s", secs / 60, secs % 60),
        _ => format!("{}h{:02}m", secs / 3600, secs % 3600 / 60),
    }
}

fn ocr(path: &Path, boxes: bool, threads: usize, scripts: &[Script]) -> Result<()> {
    let t = Instant::now();
    let mut ocr = Ocr::new(threads, scripts)?;
    let load = t.elapsed();

    let t = Instant::now();
    let img = gyotaku_ocr::load_image(path)?;
    let decode = t.elapsed();

    let t = Instant::now();
    let lines = ocr.read(&img)?;
    let read = t.elapsed();

    for l in &lines {
        if boxes {
            let r = l.rect;
            println!(
                "{:.3} {:.3} {:.3} {:.3}  {:.2}  {}",
                r.x, r.y, r.w, r.h, l.score, l.text
            );
        } else {
            println!("{}", l.text);
        }
    }
    eprintln!(
        "{} lines, models {load:.0?}, decode {decode:.0?}, ocr {read:.0?}",
        lines.len()
    );
    Ok(())
}
