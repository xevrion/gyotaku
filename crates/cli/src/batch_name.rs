//! Batch filename recommendation and safe renaming execution.

use std::collections::HashSet;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use gyotaku_core::naming::{CaseStyle, FallbackStrategy, NameOptions, PlannedRename, plan_rename};
use gyotaku_core::{Config, Index, Line, Script};
use gyotaku_ocr::Ocr;
use serde::Serialize;

const EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum SortOrder {
    #[default]
    Newest,
    Oldest,
    Name,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    Table,
    Csv,
    Json,
    Script,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum CaseStyleArg {
    Snake,
    Kebab,
    Title,
    Camel,
}

impl From<CaseStyleArg> for CaseStyle {
    fn from(c: CaseStyleArg) -> Self {
        match c {
            CaseStyleArg::Snake => Self::Snake,
            CaseStyleArg::Kebab => Self::Kebab,
            CaseStyleArg::Title => Self::Title,
            CaseStyleArg::Camel => Self::Camel,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum FallbackArg {
    Skip,
    Keep,
    Error,
}

impl From<FallbackArg> for FallbackStrategy {
    fn from(f: FallbackArg) -> Self {
        match f {
            FallbackArg::Skip => Self::Skip,
            FallbackArg::Keep => Self::Keep,
            FallbackArg::Error => Self::Error,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum ApplyAction {
    /// Copy file to target path, leaving the original screenshot untouched (default, obeys AGENTS.md)
    #[default]
    Copy,
    /// Move file to target path (refused for screenshots per AGENTS.md)
    Move,
}

pub struct BatchRunOptions {
    pub paths: Vec<PathBuf>,
    pub limit: Option<usize>,
    pub sort: SortOrder,
    pub dry_run: bool,
    pub apply: bool,
    pub action: ApplyAction,
    pub yes: bool,
    pub format: OutputFormat,
    pub case: CaseStyleArg,
    pub max_length: usize,
    pub prefix_date: bool,
    pub threads: usize,
    pub fallback: FallbackArg,
    pub config_file: Option<PathBuf>,
}

#[derive(Serialize)]
struct JsonRenameItem<'a> {
    source_path: &'a str,
    recommended_filename: &'a str,
    confidence: f32,
    nature: &'a str,
    data_gaps: &'a [String],
}

/// Recursively gathers supported images from input paths.
fn discover_images(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut images = Vec::new();

    for p in paths {
        if !p.exists() {
            bail!("path does not exist: {}", p.display());
        }

        if p.is_file()
            && let Some(ext) = p.extension().and_then(|s| s.to_str())
            && EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
        {
            images.push(p.clone());
        } else if p.is_dir() {
            scan_dir(p, &mut images)?;
        }
    }

    Ok(images)
}

fn scan_dir(dir: &Path, results: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            scan_dir(&path, results)?;
        } else if path.is_file()
            && let Some(ext) = path.extension().and_then(|s| s.to_str())
            && EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
        {
            results.push(path);
        }
    }
    Ok(())
}

/// Extracts OCR lines for an image, using the SQLite cache when possible or running OCR directly.
fn get_lines_for_image(
    path: &Path,
    index: Option<&Index>,
    ocr: &mut Option<Ocr>,
    threads: usize,
    scripts: &[Script],
) -> Result<Vec<Line>> {
    let mtime = path
        .metadata()
        .and_then(|m| m.modified())
        .map(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64
        })
        .unwrap_or(0);

    // 1. Try reading cached lines from the database.
    if let Some(idx) = index
        && let Ok(Some(cached)) = idx.lines_for_path(path, mtime)
    {
        return Ok(cached);
    }

    // 2. Uncached image: load image and run local OCR.
    let img = image::open(path)
        .with_context(|| format!("opening {}", path.display()))?
        .into_rgb8();

    if ocr.is_none() {
        *ocr = Some(Ocr::new(threads, scripts)?);
    }

    if let Some(reader) = ocr {
        reader.read(&img)
    } else {
        Ok(Vec::new())
    }
}

/// Atomically copies `src` to `dst` failing closed if `dst` already exists,
/// eliminating TOCTOU race conditions where a target could be overwritten.
fn copy_no_overwrite(src: &Path, dst: &Path) -> io::Result<u64> {
    let mut reader = fs::File::open(src)?;
    let mut writer = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dst)?;
    io::copy(&mut reader, &mut writer)
}

pub fn run(opts: BatchRunOptions) -> Result<()> {
    let start_time = Instant::now();
    let mut images = discover_images(&opts.paths)?;
    match opts.sort {
        SortOrder::Name => images.sort_unstable(),
        order => {
            let mut dated: Vec<_> = images
                .into_iter()
                .map(|path| {
                    let modified = path
                        .metadata()
                        .and_then(|metadata| metadata.modified())
                        .ok();
                    (path, modified)
                })
                .collect();
            dated.sort_unstable_by(|(left_path, left_time), (right_path, right_time)| {
                let by_time = match (left_time, right_time) {
                    (Some(left), Some(right)) => match order {
                        SortOrder::Newest => right.cmp(left),
                        SortOrder::Oldest => left.cmp(right),
                        SortOrder::Name => unreachable!(),
                    },
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                };
                by_time.then_with(|| left_path.cmp(right_path))
            });
            images = dated.into_iter().map(|(path, _)| path).collect();
        }
    }
    if let Some(limit) = opts.limit {
        images.truncate(limit);
    }

    if images.is_empty() {
        eprintln!("No supported images found in specified paths.");
        return Ok(());
    }

    // Load base configuration and override with CLI options.
    let mut name_options = NameOptions::load(opts.config_file.as_deref());
    name_options.case_style = opts.case.into();
    name_options.max_length = opts.max_length;
    name_options.prefix_date = opts.prefix_date;
    name_options.fallback = opts.fallback.into();

    let config = Config::load_or_default();
    let scripts = config.scripts();
    let index = Index::open_default().ok();

    let mut plans: Vec<PlannedRename> = Vec::with_capacity(images.len());
    let mut used_names: HashSet<String> = HashSet::new();

    let mut ocr_instance: Option<Ocr> = None;

    for path in &images {
        let lines = match get_lines_for_image(
            path,
            index.as_ref(),
            &mut ocr_instance,
            opts.threads,
            &scripts,
        ) {
            Ok(lines) => lines,
            Err(e) => {
                if opts.fallback == FallbackArg::Error {
                    bail!("could not read {}: {:#}", path.display(), e);
                }
                eprintln!("warning: could not read {}: {:#}", path.display(), e);
                Vec::new()
            }
        };

        match plan_rename(path, &lines, &name_options, &mut used_names) {
            Some(plan) => {
                if opts.fallback == FallbackArg::Error && !plan.data_gaps.is_empty() {
                    bail!(
                        "failed to extract title for {}: {}",
                        path.display(),
                        plan.data_gaps.join(", ")
                    );
                }
                plans.push(plan);
            }
            None => {
                if opts.fallback == FallbackArg::Error {
                    bail!(
                        "failed to extract title for {} and --fallback error was specified",
                        path.display()
                    );
                }
            }
        }
    }

    // Render output according to requested format.
    match opts.format {
        OutputFormat::Json => {
            let json_items: Vec<JsonRenameItem> = plans
                .iter()
                .map(|p| JsonRenameItem {
                    source_path: p.source_path.to_str().unwrap_or(""),
                    recommended_filename: &p.recommended_filename,
                    confidence: p.confidence,
                    nature: &p.nature,
                    data_gaps: &p.data_gaps,
                })
                .collect();
            let json_str = serde_json::to_string_pretty(&json_items)?;
            println!("{json_str}");
        }
        OutputFormat::Csv => {
            println!("source_path,recommended_filename,confidence,nature,data_gaps");
            for p in &plans {
                let gaps = p.data_gaps.join(";");
                println!(
                    "\"{}\",\"{}\",{:.2},\"{}\",\"{}\"",
                    p.source_path.display(),
                    p.recommended_filename,
                    p.confidence,
                    p.nature,
                    gaps
                );
            }
        }
        OutputFormat::Script => {
            print_script(&plans);
        }
        OutputFormat::Table => {
            print_table(&plans, start_time.elapsed(), opts.dry_run);
        }
    }

    // Handle --apply mutation guard.
    if opts.apply {
        if !opts.yes {
            if !io::stdin().is_terminal() {
                bail!("--apply requires terminal confirmation or the -y/--yes flag");
            }
            let action_verb = match opts.action {
                ApplyAction::Copy => "copying",
                ApplyAction::Move => "renaming",
            };
            print!(
                "\nProceed with {} {} files? [y/N]: ",
                action_verb,
                plans.len()
            );
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Operation cancelled. No files modified.");
                return Ok(());
            }
        }

        let mut success_count = 0;
        for plan in &plans {
            if plan.source_path != plan.target_path {
                match opts.action {
                    ApplyAction::Copy => {
                        match copy_no_overwrite(&plan.source_path, &plan.target_path) {
                            Ok(_) => success_count += 1,
                            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                                eprintln!(
                                    "error: target already exists: {}; refusing to overwrite",
                                    plan.target_path.display()
                                );
                            }
                            Err(e) => {
                                eprintln!(
                                    "error copying {} to {}: {:#}",
                                    plan.source_path.display(),
                                    plan.target_path.display(),
                                    e
                                );
                            }
                        }
                    }
                    ApplyAction::Move => {
                        // Invariant per AGENTS.md: Never write to, move or delete a user's screenshots.
                        eprintln!(
                            "error: cannot move user screenshot {} (per AGENTS.md rule: user screenshots are read-only; use --action copy or system trash)",
                            plan.source_path.display()
                        );
                    }
                }
            }
        }
        let action_past = match opts.action {
            ApplyAction::Copy => "copied",
            ApplyAction::Move => "renamed",
        };
        println!("\nSuccessfully {action_past} {success_count} files.");
    }

    Ok(())
}

fn print_table(plans: &[PlannedRename], elapsed: std::time::Duration, dry_run: bool) {
    println!(
        "{:<45} {:<42} {:<12} {:<14}",
        "Source", "Recommended Name", "Confidence", "Nature"
    );
    println!("{}", "-".repeat(115));

    for p in plans {
        let src_name = p
            .source_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let truncated_src = if src_name.len() > 42 {
            format!("{}...", &src_name[..39])
        } else {
            src_name.to_string()
        };

        let conf_str = if p.confidence > 0.0 {
            format!("{:.0}%", p.confidence * 100.0)
        } else {
            "N/A".to_string()
        };

        let gap_marker = if !p.data_gaps.is_empty() {
            format!(" ({})", p.data_gaps.join(", "))
        } else {
            String::new()
        };

        println!(
            "{:<45} {:<42} {:<12} {}{}",
            truncated_src, p.recommended_filename, conf_str, p.nature, gap_marker
        );
    }

    println!("{}", "-".repeat(115));
    if dry_run {
        println!(
            "Processed {} images in {:.2}s. (Dry run: run with --apply to execute renames)",
            plans.len(),
            elapsed.as_secs_f32()
        );
    } else {
        println!(
            "Processed {} images in {:.2}s.",
            plans.len(),
            elapsed.as_secs_f32()
        );
    }
}

fn print_script(plans: &[PlannedRename]) {
    let commands: Vec<(&Path, &Path, &str)> = plans
        .iter()
        .filter(|p| p.source_path != p.target_path)
        .map(|p| {
            (
                p.source_path.as_path(),
                p.target_path.as_path(),
                p.recommended_filename.as_str(),
            )
        })
        .collect();
    crate::platform::print_rename_script(&commands);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copy_no_overwrite_refuses_to_clobber_existing_target() {
        let temp_dir = std::env::temp_dir().join("gyotaku_atomic_copy_unit_test");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let src = temp_dir.join("source.png");
        let dst = temp_dir.join("target.png");
        fs::write(&src, b"source image bytes").unwrap();
        fs::write(&dst, b"original existing content").unwrap();

        let result = copy_no_overwrite(&src, &dst);
        assert!(result.is_err(), "must fail when target exists");
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::AlreadyExists);

        // Verify dst content was untouched
        let dst_content = fs::read(&dst).unwrap();
        assert_eq!(dst_content, b"original existing content");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
