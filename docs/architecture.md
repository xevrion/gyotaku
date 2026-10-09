# Architecture

- [Overview](#overview)
- [Text recognition](#text-recognition)
- [Index and search](#index-and-search)
- [Search window](#search-window)
- [Operating systems](#operating-systems)
- [Repository layout](#repository-layout)

## Overview

```
Screenshot saved to a watched folder
  1. The indexer is notified through inotify and runs at idle priority
  2. OCR extracts each line of text and its bounding box
  3. Lines are stored in SQLite, with a full-text index using the trigram tokenizer
  4. The search window queries the index on every keystroke
```

gyotaku does not capture screenshots. It indexes the folders that existing screenshot tools write to. An existing backlog is indexed newest first, since recent screenshots are the most likely to be searched for.

## Text recognition

gyotaku uses [PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR)'s PP-OCRv6 models (tiny detector, small recognizer), executed on the CPU by [ONNX Runtime](https://onnxruntime.ai) through the [`ort`](https://crates.io/crates/ort) crate. The pipeline around the models is implemented in `crates/ocr`: extracting text boxes from the detector's probability map, cropping each line, batching lines through the recognizer, and CTC decoding.

### ONNX Runtime loading

ONNX Runtime is loaded dynamically at runtime from Microsoft's official release rather than linked at build time. The prebuilt binaries commonly linked by Rust projects require a recent glibc and fail to link on Ubuntu 22.04 and Debian 12. Microsoft's build requires glibc 2.27, which covers Ubuntu 18.04 and Debian 10 onward. The download is verified against a SHA-256 checksum pinned in the source.

### Engine selection

Tesseract and [ocrs](https://github.com/robertknight/ocrs) were evaluated first on real screenshots. Both missed small interface text such as sidebar labels, buttons and tab names, which is the text users most often search for. PP-OCRv6 recognized nearly all of it. The full comparison is in the [development log](../notes.md#ocr-bakeoff-python-before-writing-any-rust).

### Tuning for screenshots

PaddleOCR's defaults target photographs of documents. gyotaku changes them for screen content:

| Parameter | PaddleOCR default | gyotaku | Rationale |
|---|---|---|---|
| Detection model | Small | Tiny | 4.6x faster, retains 99.6% of words |
| Detection input size | Upscale to 736 px short side; never downscale | Downscale only, to ~1 megapixel | Screen text is already legible at native size. Upscaling a small crop cost 330 MB of memory with no accuracy gain. |
| Box geometry | Rotated rectangles | Axis-aligned rectangles | Screenshot text is horizontal or vertical, so connected components are sufficient |
| Vertical text | Columns turned counterclockwise | Turned counterclockwise, and clockwise when that reads poorly | Counterclockwise reads vertical Japanese and Chinese; a sideways label reading upwards, such as a chart axis, needs the other turn |
| Recognition batching | 6 lines per batch | Batched by total width | The recognizer scores ~18,000 characters per step, so the output tensor dominates memory |

Single-character lines are discarded, because interface icons are frequently recognized as a single high-confidence character. A box at least 1.5 times taller than wide is a column: it is turned upright before recognition, and dropped if neither turn reads it with a score of 0.8 or more, since narrow icons outnumber real columns.

### Other scripts

The default recognizer covers Latin, Chinese, Japanese and Greek. Other scripts are opt-in, each with a recognizer of its own that is downloaded the first time it is enabled. Devanagari uses PaddleOCR's PP-OCRv5 Devanagari model (7.9 MB), whose alphabet also includes Latin letters, digits and punctuation. PaddleOCR has no Bengali recognizer, so Bengali uses EasyOCR's (54 MB after quantizing its weights to 8 bits), exported by `contrib/export-bengali-model.py` to take the same input as the PaddleOCR models.

Detection runs once and the default recognizer reads every line. A line is read again by the extra recognizer when the default one was unsure of it (score under 0.9), or when it contains a long unread stretch: CTC decoding reports the longest run of blank steps, and a glyph outside the model's alphabet produces exactly that. A line that is half English and half Hindi reads the English confidently and leaves a gap of 24 steps where the Hindi was; fully read lines have gaps of 2 to 5. The second reading replaces the first only if it scores higher and contains the script it is for, so enabling a script never changes how English or Chinese text is read.

## Index and search

The index is a single SQLite database using FTS5 with the trigram tokenizer. Trigram indexing enables substring matching: `nutsmp` matches `donutsmp.net`, and partially misrecognized words remain findable.

- **One full-text row per screenshot.** Multi-word queries match across lines. Per-line text and bounding boxes are stored in a separate table.
- **Query escaping.** Every term is quoted before it reaches `MATCH`, so operators and punctuation in user input are treated as literal text.
- **Short terms.** Terms under three characters cannot use the trigram index and fall back to `LIKE`, applied to rows already narrowed by the other terms.
- **Near matches.** A second trigram index holds each screenshot's text with OCR look-alikes folded together (`0` and `o`; `1`, `l`, `i` and `|`; `5`, `s` and `$`; `8` and `b`; `m` and `rn`; `w` and `vv`; `d` and `cl`). It is only searched when the exact matches don't fill the result limit, and each candidate is then checked against the text as it was read: one misread letter is allowed in a word of four to seven letters, two from eight letters up, none under four. Without that check, folding finds ordinary words in other words, such as `email` in `internally`. Spaces are not folded, since on a real index that mostly matched words that were apart all along (`in voice` for `invoice`). Near matches always follow every exact match and are labelled in the window.
- **Filters.** `crates/core/src/query.rs` splits a query into words and filters (`in:`, `date:`, `before:`, `after:`). Filters become plain `WHERE` clauses on the stored path and modification time, applied to both the exact and the near search, so they need no extra index; dates are resolved to local midnights when the query is parsed. `in:` matches a path component by prefix with `LIKE`, followed by a later separator so a file name can't match it. On 6,887 screenshots a filtered keystroke takes the same 1 to 12 ms as an unfiltered one, and a query of only filters takes 3 to 7 ms.
- **Bursts.** Near-identical screenshots taken close together are grouped when they're stored, not when they're searched (`crates/core/src/burst.rs`). Each new screenshot is compared with those taken within 10 minutes of it, by its text (the share of lines in both, and whether the shared ones moved, which is what scrolling does) and, for screenshots with little text, by a 64-bit difference hash of its thumbnail. Groups chain, and each one is named by its lowest id in a `burst` column, so the window only has to collapse results that share a name, which takes under a millisecond for 20,000 results. Grouping at query time would have meant comparing the text of every result on every keystroke. The rules were tuned on 6,863 real screenshots by reading what each grouped pair said; different pages of one app share their sidebar but not the rest, and are kept apart. Screenshots indexed before version 3 of the schema are grouped by the reader in the background, 50 at a time once there is nothing new to read, with their hashes taken from the thumbnails (about 2.4 ms each).
- **Lazy highlighting.** Matched lines are fetched only for thumbnails currently on screen. A two-letter query can match 2,000 screenshots; loading lines for all of them took 60 to 90 ms, compared with 1 to 10 ms for the ~30 visible thumbnails.

## Search window

The window is built with [GPUI](https://gpui.rs), the UI framework behind the Zed editor, rendering through wgpu on Vulkan or OpenGL on Linux, and Direct3D 11 on Windows.

- **Resident process.** GPU driver initialization accounts for 300 to 800 ms of a cold start. After the first launch, the process remains resident while hidden (37 MB) and later launches signal it (see [Operating systems](#operating-systems)), reducing summon time to ~120 ms. The window reopens in its previous state.
- **Search highlighting.** While a query is active, thumbnails are dimmed and matched lines are outlined in the accent color, vermilion. The accent color is reserved for search matches; selection uses the foreground color.
- **Interruptible motion.** Opening a screenshot animates it from its thumbnail with a spring. Springs can be interrupted, so pressing Escape mid-animation reverses from the current position. Animations advance by elapsed time, not per frame, so they run at the correct speed on slow machines.
- **Software rendering.** When no GPU is available, animated transitions are replaced by short fades, because each frame rendered on the CPU has a measurable power cost.
- **Image loading.** GPUI's built-in image loader leaked roughly 9 MB per page of scrolling. Thumbnails are instead decoded on a background thread and passed to GPUI as prepared render images, which keeps memory flat.
- **Clipboard.** On Wayland, clipboard contents are served by the application that set them. gyotaku delegates to `wl-copy` when available, so copied content persists after the window closes.

The interface uses IBM Plex Sans and a neutral palette so that the screenshots themselves carry the color. Motion occurs only in response to user input.

## Operating systems

Everything that differs between operating systems is isolated in `platform` modules, one file per system implementing the same interface. The rest of the code never checks which system it runs on.

| Module | Linux | Windows |
|---|---|---|
| `crates/app/src/platform` | | |
| Launcher window | Layer-shell overlay on Wayland; a regular window elsewhere | Borderless, always-on-top popup that hides when it loses focus |
| Summoning | A shortcut configured in the desktop runs `gyotaku-app` | A global hotkey registered by the app (Alt+Shift+S) |
| Signalling the resident process | Unix socket | Loopback TCP port, with a lock file against races |
| Background indexing | systemd user service, or an XDG autostart entry | The app starts the indexer whenever it runs; the `Run` registry key starts the app at sign-in |
| Clipboard | `wl-copy` or `xclip`, so copies persist after the window closes | The system clipboard |
| Screenshot folder detection | Configuration of Flameshot, Spectacle, ksnip, niri, grim, Hyprshot | Game Bar captures and ShareX, plus `Pictures\Screenshots` |
| `crates/cli/src/platform` | | |
| Idle priority | `SCHED_IDLE` and idle I/O priority | Process background mode |
| Battery detection | `/sys/class/power_supply` | `GetSystemPowerStatus` |
| Watching the clipboard, when saving copied images is on | `wl-paste --watch` on Wayland; XFixes events and `xclip` on X11 | `AddClipboardFormatListener` on a message-only window |
| `crates/core/src/trash` | | |
| Moving to the trash | freedesktop.org trash specification, per drive | Recycle Bin, fixed drives only |

Each `platform/mod.rs` fails the build with a `compile_error!` on an unsupported system, so a port starts by adding one file per module and lets the compiler list what remains.

Windows releases link the C runtime statically and ship Microsoft's Visual C++ runtime DLLs beside ONNX Runtime, so they run on a fresh installation. CI builds the Windows version on every change, opens the window on a Windows runner (rendering with WARP, a software renderer), and stores screenshots of it as a build artifact.

## Repository layout

| Path | Contents |
|---|---|
| `crates/core` | Index, configuration, search, thumbnail crop rules, moving to the trash |
| `crates/ocr` | PP-OCRv6 on ONNX Runtime: detection, recognition, decoding, model download |
| `crates/cli` | The `gyotaku` binary: `index`, `watch`, `search`, `stats`, `ocr` |
| `crates/app` | The `gyotaku-app` binary: search window, onboarding and settings |
| `install.sh`, `install.ps1` | One-command installers for Linux and Windows, tested in CI |
| `contrib/` | systemd user unit for manual installation, the Windows release readme |
| `tests/fixtures/` | Test images used in CI |

`core` and `ocr` have no dependency on the UI, so the command-line interface and CI exercise them on machines without a display.

See [CONTRIBUTING.md](../CONTRIBUTING.md) for build and test instructions, and the [development log](../notes.md) for the measurements and bugs behind these decisions.
