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

pub struct BatchRunOptions {
    pub paths: Vec<PathBuf>,
    pub dry_run: bool,
    pub apply: bool,
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

pub fn run(opts: BatchRunOptions) -> Result<()> {
    let start_time = Instant::now();
    let images = discover_images(&opts.paths)?;

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
                eprintln!("warning: could not read {}: {:#}", path.display(), e);
                Vec::new()
            }
        };

        if let Some(plan) = plan_rename(path, &lines, &name_options, &mut used_names) {
            plans.push(plan);
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
            print!(
                "\nProceed with renaming {} files? [y/N]: ",
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
                if let Err(e) = fs::rename(&plan.source_path, &plan.target_path) {
                    eprintln!(
                        "error renaming {} to {}: {:#}",
                        plan.source_path.display(),
                        plan.target_path.display(),
                        e
                    );
                } else {
                    success_count += 1;
                }
            }
        }
        println!("\nSuccessfully renamed {success_count} files.");
    }

    Ok(())
}

fn print_table(plans: &[PlannedRename], elapsed: std::time::Duration, dry_run: bool) {
    println!("{:<45} {:<42} {:<12} {:<14}", "Source", "Recommended Name", "Confidence", "Nature");
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
    #[cfg(windows)]
    {
        println!("# PowerShell Batch Rename Script generated by gyotaku");
        println!("$ErrorActionPreference = 'Stop'\n");
        for p in plans {
            if p.source_path != p.target_path {
                println!(
                    "Rename-Item -LiteralPath \"{}\" -NewName \"{}\"",
                    p.source_path.display(),
                    p.recommended_filename
                );
            }
        }
    }

    #[cfg(not(windows))]
    {
        println!("#!/bin/sh");
        println!("# Shell Batch Rename Script generated by gyotaku");
        println!("set -e\n");
        for p in plans {
            if p.source_path != p.target_path {
                println!(
                    "mv -- \"{}\" \"{}\"",
                    p.source_path.display(),
                    p.target_path.display()
                );
            }
        }
    }
}
