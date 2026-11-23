# Batch filename recommendation specification

This document specifies a batch filename recommendation and renaming system for gyotaku, designed to process large collections of screenshots and infographics (1,000+ images) using local OCR.

---

## Pre-implementation contract (mandatory gate)

Before writing any implementation code, this contract specifies the exact interfaces, invariants, and safety boundaries. Implementation will not proceed until this contract is explicitly reviewed and approved.

### 1. CLI and I/O contract
- **Command invocation**: `gyotaku name [OPTIONS] <PATHS>...`
- **Arguments and flags**:
  - `<PATHS>...`: One or more paths (file or directory). Mandatory.
  - `--dry-run`: Boolean. Default `true`. When neither `--dry-run` nor `--apply` is specified, dry-run is enforced.
  - `--apply`: Boolean. Default `false`. Requires interactive confirmation unless `--yes` is supplied.
  - `-y, --yes`: Boolean. Default `false`. Non-interactive confirmation override for automated pipelines.
  - `--format <FORMAT>`: Enum (`table`, `csv`, `json`, `script`). Default: `table`.
  - `--case <CASE>`: Enum (`snake`, `kebab`, `title`, `camel`). Default: `snake`.
  - `--max-length <N>`: Integer. Default: `50`. Valid range: `10..=200`.
  - `--prefix-date`: Boolean. Default: `false`.
  - `--threads <N>`: Integer. Default: `gyotaku_core::default_threads()`.
  - `--fallback <STRATEGY>`: Enum (`skip`, `keep`, `error`). Default: `skip`.
- **Precedence rules**:
  - `CLI flags` > `.env` / environment variables > `config.json` > Defaults.
  - `--apply` takes precedence over `--dry-run` when both are supplied; without `--yes`, `--apply` prompts interactively.
- **External configuration sources**:
  - `config.json` (or custom path via `--config`): sets `weight_height`, `weight_position`, `weight_centering`, `weight_confidence`, `header_zone_max_y`, `case_style`, `max_length`, `fallback`.
  - `.env` / Environment variables: `GYOTAKU_WEIGHT_HEIGHT`, `GYOTAKU_WEIGHT_POSITION`, `GYOTAKU_WEIGHT_CENTERING`, `GYOTAKU_WEIGHT_CONFIDENCE`, `GYOTAKU_HEADER_ZONE`, `GYOTAKU_CASE_STYLE`, `GYOTAKU_MAX_LENGTH`, `GYOTAKU_FALLBACK`.
- **Exit codes**:
  - `0`: Success (dry-run generated, script emitted, or all applied renames completed).
  - `1`: Fatal error (invalid input path, corrupt database, permission failure, unhandled IO error).
  - `2`: CLI usage / parse error (invalid flags or values emitted by `clap`).

### 2. Verified data schemas and path derivations
- **Confirmed system paths** (derived via `gyotaku_core`):
  - Database: `gyotaku_core::data_dir()?.join("index.db")`
    - Windows: `%APPDATA%\gyotaku\data\index.db`
    - Linux: `~/.local/share/gyotaku/index.db`
    - macOS: `~/Library/Application Support/gyotaku/index.db`
  - Models directory: `gyotaku_ocr::models_dir()?`
  - Cache directory: `gyotaku_core::dirs()?.cache_dir().join("thumbs")`
- **Existing database schema** (from `crates/core/src/index.rs`):
  - `shots (id INTEGER PRIMARY KEY, path TEXT UNIQUE, mtime INTEGER, width INTEGER, height INTEGER)`
  - `lines (id INTEGER PRIMARY KEY, shot_id INTEGER REFERENCES shots(id), text TEXT, x REAL, y REAL, w REAL, h REAL, score REAL)`
- **In-memory data structures**:
  - `Line { text: String, rect: Rect { x: f32, y: f32, w: f32, h: f32 }, score: f32 }`
  - `PlannedRename`:
    ```rust
    pub struct PlannedRename {
        pub source_path: PathBuf,
        pub recommended_filename: String,
        pub target_path: PathBuf,
        pub confidence: f32,
        pub nature: String,
        pub data_gaps: Vec<String>,
    }
    ```
- **Output JSON contract**:
  ```json
  [
    {
      "source_path": "path/to/image.png",
      "recommended_filename": "comparison_docker_vs_podman.png",
      "confidence": 0.88,
      "nature": "comparison",
      "data_gaps": []
    }
  ]
  ```

### 3. Happy-path flow
1. User invokes `gyotaku name --dry-run --format table ./infographics/`.
2. CLI validates that target folder exists without modifying filesystem state.
3. Scanner gathers all images with extensions `.png`, `.jpg`, `.jpeg`, `.webp`.
4. Index resolver queries `index.db` for cached OCR lines by `path` and `mtime`.
5. Uncached images are fed to worker thread pool running `gyotaku_ocr::Ocr::read`.
6. Lines enter heuristic engine: spatial zoning, nature classification, hero title extraction, multi-line clustering, slugification.
7. Collision resolver deduplicates target names across the batch (`_2`, `_3`).
8. Formatter outputs dry-run report to stdout or writes script file. Process exits with code `0`.

### 4. Edge cases and error matrix

| Input / Condition | Behavior | Exit Code |
|---|---|---|
| Target directory does not exist | Fail-closed: print error message to stderr | `1` |
| Image contains zero text / blank | Record `"no_text_detected"` in `data_gaps`; apply `--fallback` policy (`skip`/`keep`) | `0` |
| Low OCR confidence (`< 0.50`) | Record `"low_confidence_title"` in `data_gaps`; include proposal flagged with warning | `0` |
| SQLite `index.db` missing or locked | Fallback to live parallel OCR; log non-fatal warning to stderr | `0` |
| SQLite `index.db` corrupted | Fail-closed with explicit error message to stderr | `1` |
| Colliding target filename on disk | Append numeric counter `_2`, `_3` before collision occurs | `0` |
| Windows reserved filename (`CON.png`, `NUL.png`) | Sanitize by prefixing `file_con.png` | `0` |
| `--apply` invoked in non-interactive shell without `-y` | Fail-closed: refuse to mutate without explicit confirmation | `1` |
| Unreadable/corrupted image file | Record `"corrupt_image_file"` in `data_gaps`; skip file; report summary | `0` |

### 5. Security and defensive invariants
- **Secrets and scoping**: No network transport, no cloud APIs, no secret credentials. All execution is offline and local. Non-interactive execution is guaranteed when `-y` is provided.
- **Type and schema validation**: Verify line coordinates are finite numbers clamped to `[0.0, 1.0]`. Handle empty line lists without division by zero. Validate all deserialized options before execution.
- **Output sanitization**: Escape delimiter characters in generated CSV (`"`, commas) and Markdown tables (`|`). When generating shell scripts (`.ps1`, `.sh`), properly quote all file paths to prevent shell injection.
- **Path and slug safety**: Enforce slug regex `^[a-z0-9_]+$` (or chosen case style). Prevent directory traversal (`..`, `/`, `\`). Reject or sanitize Windows reserved device names (`CON`, `PRN`, `AUX`, `NUL`, `COM1-9`, `LPT1-9`).
- **Resource lifecycle (Windows-safe)**: Verify file existence before opening handles to prevent creation of 0-byte orphan files. Ensure all file handles, transactions, and SQLite connections are wrapped in RAII/explicit cleanup to prevent file locking on Windows.

### 6. Acceptance criteria
- **Batch capability**: Successfully process 1,000+ images in dry-run mode without memory exhaustion or process hangs.
- **Negative test 1 (Missing path)**: Executing against a non-existent path exits with code `1`.
- **Negative test 2 (Zero-text graphic)**: Abstract images with zero text record `"no_text_detected"` in `data_gaps` and obey `--fallback` without panic.
- **Negative test 3 (Path traversal & reserved names)**: Input text attempting `../../` or reserved device name `CON` produces sanitized safe filename without escaping directory.
- **Negative test 4 (Dry-run safety)**: Running `--dry-run` guarantees zero file modifications, moves, or deletions on disk.
- **Unit test coverage**: Unit tests verifying spatial scoring, multi-line clustering, nature classifier, and slugifier.

### 7. Out of scope
- Live GUI batch renaming inside `crates/app` (reserved for future iteration).
- Automatic file deletion or unconfirmed destructive moves.
- Cloud-based LLM/multimodal re-ranking models.
- Modifications to underlying PP-OCRv6 neural network weights.

---

## Architectural spec

### High-level architecture

```
                     Input Files / Folders
                              |
                              v
                   [File Scanner & Filter]
                              |
              +---------------+---------------+
              |                               |
       Already Indexed?                Not in Index
              |                               |
              v                               v
     [SQLite Index Cache]             [Parallel OCR Pool]
     (Instant: cached lines)        (PP-OCRv6 via ONNX Runtime)
              |                               |
              +---------------+---------------+
                              |
                              v
                 [Line Aggregation & Filter]
                              |
                              v
               [Title Heuristic Scoring Engine]
              (Height, position, width, score)
                              |
                              v
                 [Title Boundary Assembler]
                 (Merges multi-line headings)
                              |
                              v
               [Slugifier & Name Sanitizer]
                 (Casing, length, safe chars)
                              |
                              v
               [Collision Resolver & Planner]
                (Deduplicates target names)
                              |
              +---------------+---------------+
              |                               |
       Dry Run / Report             Execution Mode
    (Terminal table, CSV, JSON)   (--apply or shell script)
```

### System boundaries and component breakdown

1. `crates/core::naming`:
   - Contains pure business logic and algorithms.
   - Input: collection of lines with normalized coordinates (`Rect`) and confidence scores.
   - Output: candidate title, confidence score, and sanitized slug.
   - No filesystem side effects, no GUI dependencies.

2. `crates/cli::batch_name`:
   - Subcommand `gyotaku name` (or `gyotaku batch-name`).
   - Handles file discovery, thread pooling, index cache checks, progress reporting, and output generation.
   - Enforces safe defaults: defaults to dry-run output (table/CSV/JSON/script) before any file alteration.

3. `crates/app` (Future UI integration):
   - Reuses `crates/core::naming` to display recommended titles in the detail view or trigger single/batch name exports.

### Performance and concurrency model for 1,000+ images

- **Indexed images**: If images were already watched or indexed by gyotaku, lines and bounding boxes already reside in `index.db`. In this state, processing 1,000 images requires only SQLite queries, executing in under 200 ms total.
- **Unindexed images**: Uncached images require PP-OCRv6 inference. At ~0.77 s per image on a single thread:
  - Sequential: ~13 minutes for 1,000 images.
  - Multi-threaded worker pool (`rayon` or multi-instance channel, utilizing CPU cores up to system thread budget): 4 to 8 workers process 1,000 images in approximately 90 to 180 seconds.
- **Memory safety**: Avoid loading all full-resolution images simultaneously. Process through a streaming bounded channel (`crossbeam` or `std::sync::mpsc`) with a work buffer proportional to worker count.

---

## Implementation spec

### CLI interface

```sh
gyotaku name [OPTIONS] <PATHS>...
```

#### Arguments and options

- `<PATHS>...`: One or more image files or directories to scan recursively.
- `--dry-run`: Default behavior. Prints proposed renames without touching files.
- `--apply`: Executes renames on disk after explicit confirmation.
- `--format <FORMAT>`: Output format: `table` (default), `csv`, `json`, or `script` (generates `.sh` or `.ps1`).
- `--threads <N>`: Thread count for unindexed OCR (default: system logical cores).
- `--case <CASE>`: Naming style: `snake` (default, e.g. `quarterly_revenue_growth.png`), `kebab` (`quarterly-revenue-growth.png`), `title` (`Quarterly Revenue Growth.png`), or `camel`.
- `--max-length <N>`: Maximum slug character length before truncating at word boundaries (default: 50).
- `--prefix-date`: Optional flag to prefix with date extracted from image metadata/mtime (default: off; e.g. `2026-11-23_quarterly_revenue_growth.png`).
- `--include-existing-id`: Appends short hash or original index to prevent collisions.
- `--fallback <STRATEGY>`: Action when no high-confidence title is found: `skip`, `keep`, or `prompt`.

### Core data structures

```rust
pub struct NameOptions {
    pub case_style: CaseStyle,
    pub max_length: usize,
    pub prefix_date: bool,
    pub min_confidence: f32,
    pub fallback: FallbackStrategy,
}

pub struct TitleCandidate {
    pub raw_text: String,
    pub confidence: f32,
    pub bounding_box: Rect,
    pub is_multi_line: bool,
}

pub struct PlannedRename {
    pub source_path: PathBuf,
    pub recommended_filename: String,
    pub target_path: PathBuf,
    pub confidence: f32,
    pub reason: String,
}
```

### Pipeline steps

1. **Discovery**:
   - Walk inputs, collect files matching supported extensions (`png`, `jpg`, `jpeg`, `webp`).
2. **Cache resolution**:
   - Query `Index::open_default()` for matching `path` and valid `mtime`.
   - If present, fetch `lines` directly from SQLite.
   - If absent, queue for background OCR.
3. **OCR inference**:
   - Distribute unindexed images across `Ocr` worker instances using Rayon or thread workers.
4. **Candidate evaluation**:
   - Feed lines into `crates/core::naming::extract_title()`.
5. **Slug generation and conflict resolution**:
   - Convert extracted title to target casing, strip illegal characters, truncate gracefully.
   - Track planned names in a hash set. Append numeric suffixes (`-2`, `-3`) for duplicates.
6. **Output generation**:
   - Render dry-run table, generate script file, or perform atomic renames if `--apply` is specified.

---

## Design spec: infographic visual analysis and title extraction

Infographics present unique challenges compared to standard document scans or terminal screenshots:
- They use non-linear canvas layouts (grids, columns, radial maps, branching flows).
- They combine decorative kickers (category pills), large hero headlines, secondary explanatory subheadings, and dense metric labels.
- They feature distinct graphic archetypes (comparisons, timelines, cheat sheets, hierarchical flowcharts).

This section details how gyotaku analyzes infographic layouts to classify their "nature" and pull clean, representative titles.

### 1. Infographic layout anatomy and spatial zoning

An infographic is segmented into four primary vertical and functional zones:

```
+-------------------------------------------------------------+
| 1. META / BRANDING ZONE (Top 0% - 8%)                       |
|    - Category badge/pill: "CHEAT SHEET", "FIELD GUIDE"      |
|    - Publication logo / series marker (discarded as noise)  |
+-------------------------------------------------------------+
| 2. HERO HEADER ZONE (Top 5% - 30%)                          |
|    - Main Title: largest font, highest prominence           |
|    - Subtitle / Clarifier: medium font, immediately below   |
+-------------------------------------------------------------+
| 3. CONTENT BODY ZONE (30% - 88%)                            |
|    - Archetype layout: timelines, columns, flow steps       |
|    - Section headers, data callouts, metric badges          |
+-------------------------------------------------------------+
| 4. ATTRIBUTION FOOTER ZONE (88% - 100%)                     |
|    - Data sources, author credits, copyright, URLs          |
+-------------------------------------------------------------+
```

#### Distinguishing kickers, titles, and subtitles
Infographics often place a category tag (e.g. `DEV SEC OPS` or `BEGINNER'S GUIDE`) directly above or below the title:
- **Kicker / Overline**: Box height is small to medium (`h < 0.6 * max_h`), positioned directly above the title (`y_kicker < y_title`). Extracted as infographic "nature" or context tag rather than the main title.
- **Hero Title**: Bounding box with maximum normalized height (`max(rect.h)`) in the top 35% of the canvas.
- **Subtitle**: Vertically adjacent under the hero title (`dist_y < 1.2 * title.h`), smaller height (`0.4 * title.h <= h <= 0.7 * title.h`). Used as a tie-breaker when the title alone is too brief or ambiguous (e.g. title is simply `ARCHITECTURE` and subtitle is `Clean Hexagonal Patterns`).

---

### 2. Infographic "nature" (archetype) classification

Understanding the structural "nature" of an infographic enables descriptive filenames (e.g. `cheat_sheet_git_commands.png` or `comparison_postgresql_vs_mysql.png`).

The classifier identifies the archetype using structural geometry and semantic trigger tokens:

| Infographic nature | Geometric patterns | Keyword indicators | Filename affix |
|---|---|---|---|
| **Comparison** | Bimodal horizontal distribution (two symmetric columns split down the center `x ~ 0.5`) | `vs`, `versus`, `comparison`, `pros and cons`, `difference between`, `before / after` | `comparison_` or `_vs_` |
| **Cheat sheet / Guide** | High-density grid of small, regularly spaced boxes across 3 to 4 columns | `cheat sheet`, `quick reference`, `pocket guide`, `handbook`, `commands` | `cheat_sheet_` |
| **Timeline / History** | Linear chain of dates/numbers along a vertical or horizontal axis | `timeline`, `history of`, `roadmap`, `evolution`, year sequences (`1990...2025`) | `timeline_` or `roadmap_` |
| **Process / Workflow** | Numbered sequence badges (`1, 2, 3...` or `Step A, Step B`) with directional flow | `how to`, `steps to`, `process`, `workflow`, `pipeline`, `lifecycle` | `process_` or `workflow_` |
| **Architecture / Stack** | Layered horizontal tiers or hierarchical tree structure | `architecture`, `stack`, `layers`, `system design`, `internals`, `framework` | `architecture_` |
| **Statistical / Metrics** | Large isolated numeric figures (`>30%` box height) paired with lowercase labels | `survey`, `statistics`, `market share`, `growth`, `%`, `benchmarks` | `stats_` |
| **Standard / Topic** | Standard hero title and narrative body | Specific domain keywords without structural archetypes | (Direct title slug) |

---

### 3. Title extraction algorithm

The extraction pipeline isolates the true subject of the graphic through three stages:

#### Stage A: Candidate filtering and noise suppression
1. **Logo and watermark elimination**:
   - Filter single tokens residing in the extreme corners: `x < 0.08` or `x > 0.92` with `y < 0.08` or `y > 0.92`.
   - Strip domain names, URLs (`.com`, `.io`, `http`), social handles (`@`), and legal notices (`copyright`, `all rights reserved`, `source:`).
2. **Icon and symbol pruning**:
   - Drop boxes where OCR score is low (`< 0.55`) and text consists of single or doubled non-alphanumeric punctuation.
3. **Body noise exclusion**:
   - Exclude any line in the lower 65% of the image unless no text exists in the upper third.

#### Stage B: Hero title identification
For all remaining candidates in the header zone (`y < 0.35`):
1. **Dominant text sizing**:
   Find the primary headline height `H_max = max(line.rect.h)`.
2. **Prominence threshold**:
   Candidate title lines must satisfy `line.rect.h >= 0.70 * H_max`.
3. **Spatial weight**:
   Score candidate lines by combining height, top proximity, and central alignment:
   ```
   Prominence = (rect.h / H_max) * 0.50
              + (1.0 - rect.y / 0.35) * 0.30
              + (1.0 - 2.0 * abs(rect.x + rect.w/2.0 - 0.5)) * 0.20
   ```

#### Stage C: Multi-line title clustering
Infographic titles often span 2 to 3 lines with creative line wrapping.
1. Starting with the highest-scoring anchor line:
   - Identify lines directly above or below (`dist_vertical < 1.3 * anchor.rect.h`).
   - Confirm font size similarity (`abs(line.rect.h - anchor.rect.h) / anchor.rect.h < 0.25`).
   - Confirm horizontal overlap (`overlap_x > 0.4 * min(w1, w2)`).
2. Sort clustered title lines strictly by `y` coordinate (top to bottom).
3. Concatenate text with normalized single spacing.

---

### 4. Filename synthesis and formatting

The final filename synthesizes the detected nature, the extracted subject, and optional metadata:

```
[optional_nature_prefix_][extracted_subject_slug][_optional_date].png
```

#### Examples of input vs generated filename

| Graphic content | Detected title | Detected nature | Generated filename (snake_case) |
|---|---|---|---|
| "Cheat Sheet: Modern Git CLI Commands" | `Modern Git CLI Commands` | `Cheat sheet` | `cheat_sheet_modern_git_cli_commands.png` |
| "Docker vs Podman: Container Engines Compared" | `Docker vs Podman` | `Comparison` | `comparison_docker_vs_podman.png` |
| "A Visual History of Computer Memory (1950 - 2025)" | `Visual History of Computer Memory` | `Timeline` | `timeline_visual_history_computer_memory.png` |
| "Clean Architecture in Rust: Core Layers" | `Clean Architecture in Rust` | `Architecture` | `architecture_clean_architecture_in_rust.png` |
| "The Life Cycle of a Star" | `Life Cycle of a Star` | `Process` | `lifecycle_of_a_star.png` |
| "Global Coffee Production by Country" | `Global Coffee Production by Country` | `Standard` | `global_coffee_production_by_country.png` |

#### Sanitization and boundary limits
1. **Character sanitization**:
   - Convert to lowercase ASCII.
   - Replace spaces, dashes, slashes, and periods with underscores (`_`).
   - Remove forbidden characters (`\ / : * ? " < > |`).
   - Collapse consecutive underscores (`___` to `_`).
2. **Length limits**:
   - Limit slug length to 50 characters (configurable via `--max-length`).
   - Truncate cleanly at word boundaries; avoid chopping mid-word.
3. **Collision safety**:
   - If `cheat_sheet_git_commands.png` already exists in target destination, automatically append numeric counter: `cheat_sheet_git_commands_2.png`.

---

## Error handling and edge cases

1. **Unreadable or zero-text images**:
   - If OCR extracts zero lines (e.g. abstract art, corrupt file, photo without text), fallback to `--fallback` strategy:
     - `skip`: File is ignored in output plan.
     - `keep`: Filename remains unchanged.
     - `prompt`: Interactive prompt asking the user for a label.
2. **Ambiguous or low-confidence headings**:
   - If top candidate score is below threshold (`< 0.50`), mark as `[LOW_CONFIDENCE]` in output table and export script for manual review.
3. **Cross-platform filename restrictions**:
   - Windows reserved names (`CON`, `PRN`, `AUX`, `NUL`, `COM1-9`, `LPT1-9`) are sanitized with a prefix (e.g. `file_con.png`).
   - Trailing periods and spaces stripped for Windows compatibility.
   - Long paths exceeding 260 characters sanitized and clamped.
4. **Safety invariant enforcement**:
   - The `--apply` flag must require explicit terminal confirmation (`Proceed with renaming 1,024 files? [y/N]`).
   - Default output is non-destructive (`--dry-run`), generating `.ps1` (Windows) or `.sh` (Linux/macOS) rename scripts.

---

## Verification and test plan

1. **Core heuristic unit tests (`crates/core/src/naming/tests.rs`)**:
   - Spatial scoring formula: assert top-centered large boxes outscore small body boxes.
   - Multi-line clustering: verify 2-line title `"Clean Architecture" + "in Rust"` clusters and orders correctly.
   - Archetype classifier: verify keyword detection for `vs`, `cheat sheet`, `timeline`, `steps`.
   - Slugifier: verify strip punctuation, accent normalization, whitespace collapse, max-length word-boundary clamping.
   - Collision resolution: verify duplicate name sequence `foo.png`, `foo_2.png`, `foo_3.png`.
2. **CLI dry-run integration test (`tests/test_batch_name.rs`)**:
   - Run `gyotaku name --dry-run --format json <fixtures_dir>`.
   - Verify non-zero exit code on missing files.
   - Verify JSON output structure contains `source_path`, `recommended_filename`, `confidence`.
   - Ensure original fixture files remain completely untouched on disk.
