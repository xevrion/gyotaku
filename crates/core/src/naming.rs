//! Infographic and screenshot layout analysis, title extraction, and filename recommendation.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Line, Rect};

/// Case style for generated filenames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaseStyle {
    #[default]
    Snake,
    Kebab,
    Title,
    Camel,
}

impl CaseStyle {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "snake" => Some(Self::Snake),
            "kebab" => Some(Self::Kebab),
            "title" => Some(Self::Title),
            "camel" => Some(Self::Camel),
            _ => None,
        }
    }
}

/// Fallback strategy when no high-confidence title is extracted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FallbackStrategy {
    #[default]
    Skip,
    Keep,
    Error,
}

impl FallbackStrategy {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "skip" => Some(Self::Skip),
            "keep" => Some(Self::Keep),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

/// Structural archetype or topic nature of an infographic or screenshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InfographicNature {
    Comparison,
    CheatSheet,
    Timeline,
    Process,
    Architecture,
    Stats,
    #[default]
    Standard,
}

impl InfographicNature {
    pub fn prefix(self) -> Option<&'static str> {
        match self {
            Self::Comparison => Some("comparison"),
            Self::CheatSheet => Some("cheat_sheet"),
            Self::Timeline => Some("timeline"),
            Self::Process => Some("process"),
            Self::Architecture => Some("architecture"),
            Self::Stats => Some("stats"),
            Self::Standard => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Comparison => "comparison",
            Self::CheatSheet => "cheat_sheet",
            Self::Timeline => "timeline",
            Self::Process => "process",
            Self::Architecture => "architecture",
            Self::Stats => "stats",
            Self::Standard => "standard",
        }
    }
}

/// Options controlling title extraction and filename synthesis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NameOptions {
    pub case_style: CaseStyle,
    pub max_length: usize,
    pub prefix_date: bool,
    pub min_confidence: f32,
    pub fallback: FallbackStrategy,
    pub weight_height: f32,
    pub weight_position: f32,
    pub weight_centering: f32,
    pub weight_confidence: f32,
    pub header_zone_max_y: f32,
}

impl Default for NameOptions {
    fn default() -> Self {
        Self {
            case_style: CaseStyle::Snake,
            max_length: 50,
            prefix_date: false,
            min_confidence: 0.50,
            fallback: FallbackStrategy::Skip,
            weight_height: 0.45,
            weight_position: 0.30,
            weight_centering: 0.15,
            weight_confidence: 0.10,
            header_zone_max_y: 0.35,
        }
    }
}

/// JSON wrapper for config files that wrap settings under a `"naming"` key.
#[derive(Debug, Deserialize)]
struct ConfigFileWrapper {
    naming: Option<NameOptionsPartial>,
    #[serde(flatten)]
    flat: Option<NameOptionsPartial>,
}

#[derive(Debug, Default, Deserialize)]
struct NameOptionsPartial {
    case_style: Option<CaseStyle>,
    max_length: Option<usize>,
    prefix_date: Option<bool>,
    min_confidence: Option<f32>,
    fallback: Option<FallbackStrategy>,
    weight_height: Option<f32>,
    weight_position: Option<f32>,
    weight_centering: Option<f32>,
    weight_confidence: Option<f32>,
    header_zone_max_y: Option<f32>,
}

impl NameOptions {
    /// Loads configuration merging defaults with an optional JSON file and `.env` / environment variables.
    pub fn load(custom_json_path: Option<&Path>) -> Self {
        let mut opts = Self::default();

        // 1. Check custom JSON path, or fallback to ./config.json if it exists.
        let json_path = custom_json_path
            .map(|p| p.to_path_buf())
            .or_else(|| {
                let candidate = PathBuf::from("config.json");
                if candidate.exists() {
                    Some(candidate)
                } else {
                    None
                }
            });

        if let Some(path) = json_path
            && let Ok(contents) = std::fs::read_to_string(&path)
            && let Ok(parsed) = serde_json::from_str::<ConfigFileWrapper>(&contents)
        {
            let partial = parsed.naming.or(parsed.flat).unwrap_or_default();
            opts.apply_partial(&partial);
        }

        // 2. Check for .env file in current working directory and load key-value pairs.
        let env_file = Path::new(".env");
        if env_file.exists()
            && let Ok(lines) = std::fs::read_to_string(env_file)
        {
            for line in lines.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = trimmed.split_once('=') {
                    opts.apply_env_key(k.trim(), v.trim().trim_matches('"'));
                }
            }
        }

        // 3. Environment variables (highest file/system precedence before explicit CLI flags).
        for (k, v) in std::env::vars() {
            opts.apply_env_key(&k, &v);
        }

        opts
    }

    fn apply_partial(&mut self, partial: &NameOptionsPartial) {
        if let Some(val) = partial.case_style {
            self.case_style = val;
        }
        if let Some(val) = partial.max_length {
            self.max_length = val;
        }
        if let Some(val) = partial.prefix_date {
            self.prefix_date = val;
        }
        if let Some(val) = partial.min_confidence {
            self.min_confidence = val;
        }
        if let Some(val) = partial.fallback {
            self.fallback = val;
        }
        if let Some(val) = partial.weight_height {
            self.weight_height = val;
        }
        if let Some(val) = partial.weight_position {
            self.weight_position = val;
        }
        if let Some(val) = partial.weight_centering {
            self.weight_centering = val;
        }
        if let Some(val) = partial.weight_confidence {
            self.weight_confidence = val;
        }
        if let Some(val) = partial.header_zone_max_y {
            self.header_zone_max_y = val;
        }
    }

    fn apply_env_key(&mut self, k: &str, v: &str) {
        match k {
            "GYOTAKU_WEIGHT_HEIGHT" | "GYOTAKU_NAMING_WEIGHT_HEIGHT" => {
                if let Ok(val) = v.parse::<f32>() {
                    self.weight_height = val;
                }
            }
            "GYOTAKU_WEIGHT_POSITION" | "GYOTAKU_NAMING_WEIGHT_POSITION" => {
                if let Ok(val) = v.parse::<f32>() {
                    self.weight_position = val;
                }
            }
            "GYOTAKU_WEIGHT_CENTERING" | "GYOTAKU_NAMING_WEIGHT_CENTERING" => {
                if let Ok(val) = v.parse::<f32>() {
                    self.weight_centering = val;
                }
            }
            "GYOTAKU_WEIGHT_CONFIDENCE" | "GYOTAKU_NAMING_WEIGHT_CONFIDENCE" => {
                if let Ok(val) = v.parse::<f32>() {
                    self.weight_confidence = val;
                }
            }
            "GYOTAKU_HEADER_ZONE" | "GYOTAKU_NAMING_HEADER_ZONE" => {
                if let Ok(val) = v.parse::<f32>() {
                    self.header_zone_max_y = val;
                }
            }
            "GYOTAKU_MIN_CONFIDENCE" | "GYOTAKU_NAMING_MIN_CONFIDENCE" => {
                if let Ok(val) = v.parse::<f32>() {
                    self.min_confidence = val;
                }
            }
            "GYOTAKU_MAX_LENGTH" | "GYOTAKU_NAMING_MAX_LENGTH" => {
                if let Ok(val) = v.parse::<usize>() {
                    self.max_length = val;
                }
            }
            "GYOTAKU_CASE_STYLE" | "GYOTAKU_NAMING_CASE_STYLE" => {
                if let Some(val) = CaseStyle::from_name(v) {
                    self.case_style = val;
                }
            }
            "GYOTAKU_FALLBACK" | "GYOTAKU_NAMING_FALLBACK" => {
                if let Some(val) = FallbackStrategy::from_name(v) {
                    self.fallback = val;
                }
            }
            "GYOTAKU_PREFIX_DATE" | "GYOTAKU_NAMING_PREFIX_DATE" => {
                self.prefix_date = v.eq_ignore_ascii_case("true") || v == "1";
            }
            _ => {}
        }
    }
}

/// Candidate headline identified during visual analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct TitleCandidate {
    pub raw_text: String,
    pub confidence: f32,
    pub bounding_box: Rect,
    pub is_multi_line: bool,
}

/// Recommendation result for a single image in a batch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlannedRename {
    pub source_path: PathBuf,
    pub recommended_filename: String,
    pub target_path: PathBuf,
    pub confidence: f32,
    pub nature: String,
    pub data_gaps: Vec<String>,
}

/// Windows reserved device names that cannot be used as base filenames.
const WINDOWS_RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Known keyword patterns identifying infographic archetypes.
const COMPARISON_KEYWORDS: [&str; 6] = [
    " vs ",
    " versus ",
    "comparison",
    "pros and cons",
    "difference between",
    "before / after",
];
const CHEAT_SHEET_KEYWORDS: [&str; 5] = [
    "cheat sheet",
    "quick reference",
    "pocket guide",
    "handbook",
    "commands",
];
const TIMELINE_KEYWORDS: [&str; 5] = [
    "timeline",
    "history of",
    "roadmap",
    "evolution",
    "milestones",
];
const PROCESS_KEYWORDS: [&str; 6] = [
    "how to",
    "steps to",
    "process",
    "workflow",
    "pipeline",
    "lifecycle",
];
const ARCHITECTURE_KEYWORDS: [&str; 6] = [
    "architecture",
    "stack",
    "layers",
    "system design",
    "internals",
    "framework",
];
const STATS_KEYWORDS: [&str; 6] = [
    "survey",
    "statistics",
    "market share",
    "growth rate",
    "benchmark",
    "overview",
];

/// Identifies the infographic nature based on structural geometry and keyword cues.
pub fn classify_nature(lines: &[Line]) -> InfographicNature {
    let mut combined_text = String::with_capacity(512);
    for line in lines.iter().take(30) {
        combined_text.push(' ');
        combined_text.push_str(&line.text.to_lowercase());
    }

    if COMPARISON_KEYWORDS.iter().any(|k| combined_text.contains(k)) {
        return InfographicNature::Comparison;
    }
    if CHEAT_SHEET_KEYWORDS.iter().any(|k| combined_text.contains(k)) {
        return InfographicNature::CheatSheet;
    }
    if TIMELINE_KEYWORDS.iter().any(|k| combined_text.contains(k)) {
        return InfographicNature::Timeline;
    }
    if PROCESS_KEYWORDS.iter().any(|k| combined_text.contains(k)) {
        return InfographicNature::Process;
    }
    if ARCHITECTURE_KEYWORDS
        .iter()
        .any(|k| combined_text.contains(k))
    {
        return InfographicNature::Architecture;
    }
    if STATS_KEYWORDS.iter().any(|k| combined_text.contains(k)) {
        return InfographicNature::Stats;
    }

    // Geometric heuristic: bimodal column distribution around x = 0.5 (two side-by-side columns).
    if lines.len() >= 8 {
        let left_count = lines
            .iter()
            .filter(|l| l.rect.x + l.rect.w / 2.0 < 0.45)
            .count();
        let right_count = lines
            .iter()
            .filter(|l| l.rect.x + l.rect.w / 2.0 > 0.55)
            .count();
        let center_count = lines.len() - left_count - right_count;
        if left_count >= 3 && right_count >= 3 && center_count <= lines.len() / 4 {
            return InfographicNature::Comparison;
        }
    }

    InfographicNature::Standard
}

/// Determines whether a line represents branding, logo noise, or footer attribution.
fn is_noise_or_attribution(line: &Line) -> bool {
    let lower = line.text.to_lowercase();
    let text = lower.trim();

    // Corner watermarks or corner badges (x and y both extreme).
    let is_corner =
        (line.rect.x < 0.08 || line.rect.x > 0.92) && (line.rect.y < 0.08 || line.rect.y > 0.92);
    if is_corner && text.len() < 25 {
        return true;
    }

    // URLs, email, web addresses, social handles.
    if text.starts_with("http://")
        || text.starts_with("https://")
        || text.starts_with("www.")
        || text.contains(".com")
        || text.contains(".org")
        || text.contains(".io")
        || text.contains(".net")
        || text.starts_with('@')
    {
        return true;
    }

    // Footers / copyrights / licenses.
    if text.contains("copyright")
        || text.contains("all rights reserved")
        || text.starts_with("source:")
        || text.starts_with("sources:")
        || text.contains("license:")
    {
        return true;
    }

    // Pure short punctuation or noise.
    if text.len() < 3 && !text.chars().any(|c| c.is_alphanumeric()) {
        return true;
    }

    false
}

/// Scores and extracts the primary heading candidate from OCR lines using default options.
pub fn extract_title(lines: &[Line]) -> Option<TitleCandidate> {
    extract_title_with_options(lines, &NameOptions::default())
}

/// Scores and extracts the primary heading candidate using customizable weights and zone thresholds.
pub fn extract_title_with_options(lines: &[Line], options: &NameOptions) -> Option<TitleCandidate> {
    if lines.is_empty() {
        return None;
    }

    // Filter noise and clamp rect bounds.
    let valid_lines: Vec<&Line> = lines
        .iter()
        .filter(|l| {
            !is_noise_or_attribution(l)
                && l.rect.h > 0.0
                && l.rect.w > 0.0
                && l.text.trim().len() >= 2
        })
        .collect();

    if valid_lines.is_empty() {
        return None;
    }

    // Zone 1: Look primarily in configured header zone (e.g. top 35% of canvas).
    let in_header_zone: Vec<&Line> = valid_lines
        .iter()
        .copied()
        .filter(|l| l.rect.y <= options.header_zone_max_y)
        .collect();

    let candidates = if !in_header_zone.is_empty() {
        in_header_zone
    } else {
        // Fallback: If no lines in header zone, consider upper 60%.
        let in_mid_zone: Vec<&Line> = valid_lines
            .iter()
            .copied()
            .filter(|l| l.rect.y <= (options.header_zone_max_y * 1.7).min(0.85))
            .collect();
        if !in_mid_zone.is_empty() {
            in_mid_zone
        } else {
            valid_lines
        }
    };

    let max_h = candidates
        .iter()
        .map(|l| l.rect.h)
        .fold(0.0f32, |acc, h| acc.max(h));

    if max_h <= 0.0 {
        return None;
    }

    // Score candidates based on height, top proximity, horizontal centering, and OCR confidence.
    let mut best_line: Option<&Line> = None;
    let mut best_score = -1.0f32;

    for line in &candidates {
        let h_score = (line.rect.h / max_h).clamp(0.0, 1.0);
        let pos_score = (1.0 - (line.rect.y / options.header_zone_max_y).min(1.0)).clamp(0.0, 1.0);
        let center_x = line.rect.x + (line.rect.w / 2.0);
        let center_score = (1.0 - 2.0 * (center_x - 0.5).abs()).clamp(0.0, 1.0);
        let conf_score = line.score.clamp(0.0, 1.0);

        let composite = (h_score * options.weight_height)
            + (pos_score * options.weight_position)
            + (center_score * options.weight_centering)
            + (conf_score * options.weight_confidence);

        if composite > best_score {
            best_score = composite;
            best_line = Some(line);
        }
    }

    let anchor = best_line?;

    // Multi-line title assembly: check for lines vertically adjacent to the anchor.
    let mut title_lines: Vec<&Line> = vec![anchor];

    for line in &candidates {
        if std::ptr::eq(*line, anchor) {
            continue;
        }

        let dist_y = (line.rect.y - anchor.rect.y).abs();
        let height_ratio = (line.rect.h - anchor.rect.h).abs() / anchor.rect.h.max(1e-4);

        // Check horizontal overlap.
        let left = line.rect.x.max(anchor.rect.x);
        let right = (line.rect.x + line.rect.w).min(anchor.rect.x + anchor.rect.w);
        let overlap_w = (right - left).max(0.0);
        let min_w = line.rect.w.min(anchor.rect.w);
        let has_overlap = min_w > 0.0 && (overlap_w / min_w) > 0.35;

        if dist_y <= 1.4 * anchor.rect.h && height_ratio <= 0.35 && has_overlap {
            title_lines.push(line);
        }
    }

    // Sort clustered title lines top-to-bottom.
    title_lines.sort_by(|a, b| {
        a.rect
            .y
            .partial_cmp(&b.rect.y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let is_multi_line = title_lines.len() > 1;
    let mut assembled = String::with_capacity(64);
    let mut total_score = 0.0f32;

    for (i, line) in title_lines.iter().enumerate() {
        if i > 0 {
            assembled.push(' ');
        }
        assembled.push_str(line.text.trim());
        total_score += line.score;
    }

    let avg_confidence = total_score / title_lines.len() as f32;

    Some(TitleCandidate {
        raw_text: assembled,
        confidence: avg_confidence,
        bounding_box: anchor.rect,
        is_multi_line,
    })
}

/// Sanitizes a string into a clean, safe filename slug according to case style and length limits.
pub fn slugify(
    nature: InfographicNature,
    title: &str,
    date_prefix: Option<&str>,
    case_style: CaseStyle,
    max_length: usize,
) -> String {
    let mut words: Vec<String> = Vec::new();

    // Add nature prefix word if applicable.
    if let Some(prefix) = nature.prefix() {
        for part in prefix.split('_') {
            if !part.is_empty() {
                words.push(part.to_ascii_lowercase());
            }
        }
    }

    // Split title into clean alphanumeric tokens.
    let mut current_word = String::new();
    for ch in title.chars() {
        if ch.is_alphanumeric() {
            current_word.push(ch.to_ascii_lowercase());
        } else if !current_word.is_empty() {
            words.push(std::mem::take(&mut current_word));
        }
    }
    if !current_word.is_empty() {
        words.push(current_word);
    }

    // Truncate words to respect max_length.
    let delim_len = 1;
    let mut budget = max_length;
    let mut kept_words = Vec::new();

    for w in words {
        let needed = if kept_words.is_empty() {
            w.len()
        } else {
            w.len() + delim_len
        };
        if needed <= budget {
            budget -= needed;
            kept_words.push(w);
        } else if kept_words.is_empty() {
            // If even the first word exceeds budget, clamp it safely.
            let clamped: String = w.chars().take(max_length.max(3)).collect();
            kept_words.push(clamped);
            break;
        } else {
            break;
        }
    }

    if kept_words.is_empty() {
        kept_words.push("untitled".to_string());
    }

    // Format words according to case style.
    let formatted_slug = match case_style {
        CaseStyle::Snake => kept_words.join("_"),
        CaseStyle::Kebab => kept_words.join("-"),
        CaseStyle::Title => kept_words
            .iter()
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
        CaseStyle::Camel => kept_words
            .iter()
            .enumerate()
            .map(|(i, w)| {
                if i == 0 {
                    w.clone()
                } else {
                    let mut c = w.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(""),
    };

    // Prepend date if provided.
    let mut final_base = if let Some(date) = date_prefix {
        match case_style {
            CaseStyle::Snake => format!("{date}_{formatted_slug}"),
            CaseStyle::Kebab => format!("{date}-{formatted_slug}"),
            CaseStyle::Title => format!("{date} {formatted_slug}"),
            CaseStyle::Camel => format!("{date}_{formatted_slug}"),
        }
    } else {
        formatted_slug
    };

    // Windows safety: sanitize reserved device names (CON, PRN, AUX, NUL, COM1-9, LPT1-9).
    let lower_base = final_base.to_ascii_lowercase();
    if WINDOWS_RESERVED.contains(&lower_base.as_str()) {
        final_base = format!("file_{final_base}");
    }

    final_base
}

/// Resolves duplicate recommended filenames in a batch by appending sequential counters.
pub fn resolve_collision(
    base_name: &str,
    ext: &str,
    case_style: CaseStyle,
    used_names: &mut HashSet<String>,
) -> String {
    let delim = match case_style {
        CaseStyle::Snake => "_",
        CaseStyle::Kebab => "-",
        CaseStyle::Title => " ",
        CaseStyle::Camel => "_",
    };

    let initial = if ext.is_empty() {
        base_name.to_string()
    } else {
        format!("{base_name}.{ext}")
    };

    if !used_names.contains(&initial.to_ascii_lowercase()) {
        used_names.insert(initial.to_ascii_lowercase());
        return initial;
    }

    let mut counter = 2;
    loop {
        let candidate = if ext.is_empty() {
            format!("{base_name}{delim}{counter}")
        } else {
            format!("{base_name}{delim}{counter}.{ext}")
        };

        if !used_names.contains(&candidate.to_ascii_lowercase()) {
            used_names.insert(candidate.to_ascii_lowercase());
            return candidate;
        }
        counter += 1;
    }
}

/// Generates a complete PlannedRename recommendation for an image.
pub fn plan_rename(
    path: &Path,
    lines: &[Line],
    options: &NameOptions,
    used_names: &mut HashSet<String>,
) -> Option<PlannedRename> {
    let mut data_gaps = Vec::new();
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if lines.is_empty() {
        data_gaps.push("no_text_detected".to_string());
        match options.fallback {
            FallbackStrategy::Skip => return None,
            FallbackStrategy::Error => {
                return Some(PlannedRename {
                    source_path: path.to_path_buf(),
                    recommended_filename: String::new(),
                    target_path: path.to_path_buf(),
                    confidence: 0.0,
                    nature: InfographicNature::Standard.as_str().to_string(),
                    data_gaps,
                });
            }
            FallbackStrategy::Keep => {
                let filename = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("image")
                    .to_string();
                return Some(PlannedRename {
                    source_path: path.to_path_buf(),
                    recommended_filename: filename.clone(),
                    target_path: path.with_file_name(filename),
                    confidence: 0.0,
                    nature: InfographicNature::Standard.as_str().to_string(),
                    data_gaps,
                });
            }
        }
    }

    let candidate = match extract_title_with_options(lines, options) {
        Some(c) => c,
        None => {
            data_gaps.push("no_high_confidence_title".to_string());
            match options.fallback {
                FallbackStrategy::Skip => return None,
                _ => TitleCandidate {
                    raw_text: "untitled".to_string(),
                    confidence: 0.0,
                    bounding_box: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 1.0,
                        h: 1.0,
                    },
                    is_multi_line: false,
                },
            }
        }
    };

    if candidate.confidence < options.min_confidence {
        data_gaps.push("low_confidence_title".to_string());
    }

    let nature = classify_nature(lines);

    // Optional date prefix from mtime if requested.
    let date_prefix = if options.prefix_date {
        path.metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| {
                let dur = t
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                jiff::Timestamp::from_second(dur as i64)
                    .ok()
                    .map(|ts| ts.to_zoned(jiff::tz::TimeZone::system()).strftime("%Y-%m-%d").to_string())
            })
    } else {
        None
    };

    let base_slug = slugify(
        nature,
        &candidate.raw_text,
        date_prefix.as_deref(),
        options.case_style,
        options.max_length,
    );

    let final_filename = resolve_collision(&base_slug, &ext, options.case_style, used_names);
    let target_path = path.with_file_name(&final_filename);

    Some(PlannedRename {
        source_path: path.to_path_buf(),
        recommended_filename: final_filename,
        target_path,
        confidence: candidate.confidence,
        nature: nature.as_str().to_string(),
        data_gaps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify_case_styles() {
        let title = "Docker vs Podman: Fast Container Engines";
        let nature = InfographicNature::Comparison;

        let snake = slugify(nature, title, None, CaseStyle::Snake, 50);
        assert_eq!(
            snake,
            "comparison_docker_vs_podman_fast_container_engines"
        );

        let kebab = slugify(nature, title, None, CaseStyle::Kebab, 50);
        assert_eq!(
            kebab,
            "comparison-docker-vs-podman-fast-container-engines"
        );

        let title_case = slugify(nature, title, None, CaseStyle::Title, 50);
        assert_eq!(
            title_case,
            "Comparison Docker Vs Podman Fast Container Engines"
        );
    }

    #[test]
    fn test_windows_reserved_device_names() {
        let nature = InfographicNature::Standard;
        let slug = slugify(nature, "con", None, CaseStyle::Snake, 50);
        assert_eq!(slug, "file_con");

        let slug_nul = slugify(nature, "NUL", None, CaseStyle::Snake, 50);
        assert_eq!(slug_nul, "file_nul");
    }

    #[test]
    fn test_nature_classification() {
        let lines = vec![
            Line {
                text: "Modern Git Commands Cheat Sheet".to_string(),
                rect: Rect {
                    x: 0.1,
                    y: 0.05,
                    w: 0.8,
                    h: 0.08,
                },
                score: 0.95,
            },
            Line {
                text: "git checkout -b new-branch".to_string(),
                rect: Rect {
                    x: 0.1,
                    y: 0.20,
                    w: 0.4,
                    h: 0.03,
                },
                score: 0.90,
            },
        ];

        let nature = classify_nature(&lines);
        assert_eq!(nature, InfographicNature::CheatSheet);
    }

    #[test]
    fn test_title_extraction_multi_line_clustering() {
        let lines = vec![
            Line {
                text: "Clean Architecture".to_string(),
                rect: Rect {
                    x: 0.2,
                    y: 0.05,
                    w: 0.6,
                    h: 0.08,
                },
                score: 0.96,
            },
            Line {
                text: "in Rust Systems".to_string(),
                rect: Rect {
                    x: 0.25,
                    y: 0.14,
                    w: 0.5,
                    h: 0.075,
                },
                score: 0.94,
            },
            Line {
                text: "https://example.com/blog".to_string(),
                rect: Rect {
                    x: 0.05,
                    y: 0.95,
                    w: 0.4,
                    h: 0.02,
                },
                score: 0.99,
            },
        ];

        let candidate = extract_title(&lines).expect("should extract clustered title");
        assert!(candidate.is_multi_line);
        assert_eq!(candidate.raw_text, "Clean Architecture in Rust Systems");
    }

    #[test]
    fn test_collision_resolution() {
        let mut used = HashSet::new();
        let name1 = resolve_collision("overview", "png", CaseStyle::Snake, &mut used);
        assert_eq!(name1, "overview.png");

        let name2 = resolve_collision("overview", "png", CaseStyle::Snake, &mut used);
        assert_eq!(name2, "overview_2.png");

        let name3 = resolve_collision("overview", "png", CaseStyle::Snake, &mut used);
        assert_eq!(name3, "overview_3.png");
    }

    #[test]
    fn test_load_config_from_json() {
        let json_content = r#"{
            "naming": {
                "weight_height": 0.55,
                "weight_position": 0.25,
                "case_style": "kebab",
                "max_length": 65
            }
        }"#;
        let temp_dir = std::env::temp_dir();
        let config_file = temp_dir.join("gyotaku_test_config.json");
        std::fs::write(&config_file, json_content).unwrap();

        let opts = NameOptions::load(Some(&config_file));
        assert_eq!(opts.weight_height, 0.55);
        assert_eq!(opts.weight_position, 0.25);
        assert_eq!(opts.case_style, CaseStyle::Kebab);
        assert_eq!(opts.max_length, 65);

        let _ = std::fs::remove_file(config_file);
    }
}
