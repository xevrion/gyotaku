# notes

learning log for gyotaku. messy on purpose. all numbers are from my laptop (i7-14700HX, 28 threads, RTX 4060 laptop, Fedora 43, niri), measured, not guessed.

### the core trick

a screenshot is a picture, so you can't search it. OCR it once, keep every line of text plus the box it came from, and put the text in a full text index. search is then a sqlite query, and the boxes let the ui point at exactly where the word is in the image.

### OCR bakeoff (python, before writing any rust)

four of my own screenshots (a github PR page, a youtube shorts page at 2568x1428, two others), CPU only.

| engine | time per shot | peak memory | how it read them |
|---|---|---|---|
| tesseract 5.5 | 0.4 to 0.9s | 40 to 75 MB | missed every small sidebar label (Home, Shorts, Like, Share), plus junk lines like `EERE` |
| ocrs 0.13 (pure rust) | about 0.4s | about 240 MB | fast but garbled: `Later inlthis video`, `Makina`, `Shar`, `emi` |
| PaddleOCR PP-OCRv6 small via RapidOCR | 2 to 3.4s | about 640 MB | read nearly everything, including the url bar and the tiny labels |

paddle wins, search is useless if it misses words.

### tuning it for screenshots (rust, 25 random shots from my library)

the reference for "recall" is what the small models found with paddle's own settings. substring recall is the one that matters, since search matches substrings: `nisargafem` still contains `nisarga`.

| config | det | rec | total (8 threads) | substring recall |
|---|---|---|---|---|
| small det + small rec, paddle sizing | 0.69s | 0.35s | 1.04s | 100% |
| detection capped at 1 MP | 0.38s | 0.34s | 0.71s | 99.9% |
| no upscaling | 0.36s | 0.33s | 0.69s | 100% |
| tiny det | 0.15s | 0.33s | 0.48s | 99.6% |
| tiny rec | 0.69s | 0.09s | 0.78s | 99.2% |
| tiny both | 0.15s | 0.09s | 0.24s | 99.1% |

tiny det's "misses" were word gaps (`inputgroup` became `input` `group`, which is better). tiny rec's were real misreads: `mock` became `meek`, `imo` became `hmo`. so: tiny det, small rec.

final setup (tiny det, no upscaling, 1 MP cap, width budgeted rec batches) on the same 25: **99.5% substring recall, 0.62s mean at 4 threads, worst peak 287 MB** (that one is a 5048x5544 image, 28 MP, where just decoding it is 200 MB). the 4 missing words: `award`, `get`, and two that were misreads in the reference itself.

### assumptions that turned out wrong

- **"OCR isn't cpu bound."** in python, pinning RapidOCR to 2 cores was exactly as fast as 28, so i wrote down that something serial must dominate. wrong, that was python. the rust version scales properly: 12s on 1 thread, 3s on 4, 2s on 8 for the big youtube shot.
- **"paddle's detection sizing is right."** it upscales anything under 736 px on the short side and never downscales. that's for photos of paper. for screenshots it made a 586x134 crop into a 3200x736 input (330 MB peak, for nothing) and left a 4K shot at full size (most of the time). only ever shrinking read the same words.
- **"arena allocator off will fix memory."** it helped (632 to 541 MB on the big shot) but the real hog was the input size above.
- **"my LRU cache bounds memory."** see the leak below.

### the bug that cost the most time: the app leaked ~9 MB per page of scrolling

symptom: paging down through the grid, anon memory went 110 MB, 260, 400, 550, 816, 850 and never came back. same at a 40 or 150 thumbnail cache. a clean `malloc_trim` did nothing, so it wasn't allocator slack.

what i ruled out, in order:
- gpu atlas: gpui frees an atlas texture once its last tile is removed, and my cache did call `drop_image`
- wgpu staging buffers: `queue.submit` in wgpu-core 29 runs maintenance, so finished uploads are reclaimed
- my cache being recreated every frame: counted the instances, exactly one
- the images themselves: at eviction each `Arc<RenderImage>` had exactly 2 strong refs (mine and the cache entry's), so it was freed
- **painting at all**: with the thumbnails `invisible()` (still loaded, never drawn) it still leaked, 347 vs 401 MB. so it's in gpui's load path, somewhere i couldn't see into without patching gpui

what fixed it: stop using gpui's image loader. decode the thumbnail myself on a background thread, build the `RenderImage` myself, hand it to `img()` as `ImageSource::Render`. every reference is mine, and memory went 81, 191, 202, 214, 253 MB over 80 pages. then `mallopt(M_MMAP_THRESHOLD, 128K)` so glibc stops moving 0.5 MB image buffers onto the heap after the first few frees: 56 MB at start, 110 to 140 while paging hard, flat.

(pages were driven with `wtype`. watch out: the first key wtype sends after an app starts is decoded with the old keymap, where its keycode means escape. i spent a while thinking pagedown quit the app. prime it with a shift first.)

### smaller bugs

- **`pkill -f target/release/gyotaku-app` killed my own test script**, since the pattern was in its own command line. `pgrep -x` / `$!` instead.
- **the rename test re-OCR'd a file that only moved.** inotify sends `Name(From)` first and only pairs it with the new name in a later `Name(Both)`. forgetting on `From` threw the text away. now `From` waits one settle period for its partner.
- **highlight boxes landed in the wrong place on wide shots.** the backfill had made thumbnails with the old crop rule. the crop now lives in one function in core (`tile_crop`) that both the thumbnail writer and the ui call, and thumbnails can be redrawn without OCR (5136 in 76s).
- **the first build failed on `std::hint::cold_path`.** gpui from zed main needs rust 1.95, my default stable was 1.94. `rust-toolchain.toml` pins it.

### why a non obvious choice was made

- **gpui from git, not crates.io.** the 0.2.2 release renders through blade, vulkan only. zed main moved to wgpu with a GL fallback, so old integrated graphics still gets a window.
- **resident process.** our side of a cold start is ~60 ms. the gpu side is 300 to 800 ms (adapter enumeration probes the Nvidia GL driver too). so the first launch stays around after closing (37 MB) and later launches knock on a unix socket and exit. summon went from ~500 ms to ~120 ms end to end.
- **`wl-copy` for the clipboard.** a wayland clipboard is served by the app that set it. copy, escape, paste would paste nothing. wl-copy forks a tiny process that keeps serving it. checked: the copied png was still on the clipboard after the app exited.
- **highlight lines are fetched lazily.** a two letter query matches 2000 shots, and loading the matching lines for all of them took 60 to 90 ms on the ui thread. the grid shows ~30. per visible tile it's 1 to 10 ms per keystroke.
- **one fts row per screenshot, not per line**, so `invoice march` matches when the words are on different lines. the per line boxes live in a normal table.
- **every search term gets quoted before MATCH**, otherwise typing `c++` or `NOT` or a stray `"` is a syntax error. a test throws junk at it.
- **terms under 3 characters use LIKE**, the trigram index can't see them.
- **single character lines are dropped.** UI icons come back as one confident character: a bell reads as 白, a grid icon as 品, a hamburger as 三.
- **shu for found, ink for selected.** one accent colour, only ever meaning "the search found this".

### no gpu, and small laptops

tested by hiding every gpu driver (`VK_ICD_FILENAMES` pointing at Mesa's lavapipe only, or at nothing so it falls to llvmpipe opengl) and pinning to fewer cores with `taskset` and `LP_NUM_THREADS`. gpui picks the software adapter and says so (`gpu_specs().is_software_emulated`).

- our own render code is 0.03 to 0.05 ms a frame everywhere. all the cost is rasterising, which on the cpu is ~0.4 cpu-seconds a frame at this window size.
- so without a gpu the app goes calm: no flying tile, no press animation, short fades. and finished thumbnails repaint in batches every 20 ms instead of one frame each (paging brought in ~30 thumbnails, so ~30 frames). together: a session went from ~125 frames and 47 s of cpu to ~70 frames and 29 s.
- **the bug this found: animations ran slower on slow machines.** each frame stepped the spring by at most 1/30 s whatever time had really passed. at 150 ms a frame a 0.2 s fade took seconds, and an escape pressed during it got swallowed re-closing the view, so on 2 cores the window seemed to ignore escape. now it steps by real time (up to 0.25 s) and a second escape goes to the next step.

### testing through wtype

`wtype` makes a new virtual keyboard with its own keymap on every call, and the first key of a call gets decoded with the previous call's keymap. so a lone `wtype -k Page_Down` after a `wtype -k Escape` arrives as escape. this made the benchmarks flaky for a good while (the first run it quit the app, which looked like a crash). fix: start every call with `-k Shift_L`, keycodes are handed out in order so the stale slot is always shift.

### disk

- the dev profile built gpui, wgpu and friends with full debug info: 18 GB of target/debug, which ran the disk out and killed the linker with a bus error. dependencies now build without debug info, 1.6 GB.
- building in distro containers locally cost ~5 GB each, so that moved to github actions instead: ubuntu 22.04, debian 12, kali, arch, fedora, opensuse, each from a clean image with only the packages the readme lists.

### the backfill of my library

5159 files, 5136 searchable, 23 skipped (all 1x1 to 8x3 px junk), 0 failures. 43m21s at 6 threads, 288 MB peak. index 22 MB, thumbnails 130 MB.

### design direction

the screenshots are the colour, the chrome around them stays quiet: off white or near black, IBM Plex Sans (a nod to the name through its JP sibling, and not on the list of fonts already used). the one memorable move comes from the name: while you search, every print is inked dark and only the words you typed stay lit, pressed in with a quick fade. nothing animates on its own, motion only answers what you did.

### to try later

- a capture overlay of its own (or just call utsushot, 4x supersampled captures are the best OCR input there is)
- group bursts of near identical shots (i have 8 of the same page within 2 minutes in places)
- filters: `app:brave`, `yesterday`, `in:~/Pictures/Screenshots`
- fuzzy matching for OCR typos beyond what trigrams already forgive: fold look-alikes (0/O, 1/l/I, rn/m, 5/S) on both sides before matching, and match across spaces so a word the OCR split (`ord er`) still finds `order`. suggested on X by @sunsetsyntax, 2026-10-05. and from @ivzhukau the same day, for terminal screenshots: exact matches must rank first, near matches get a visible label (a different outline style, not shu, since shu means "found exactly this"), and the detail view shows the line exactly as ocr read it, so whether an error code had O or 0 is never hidden. folding only ever widens what's found, it never rewrites the stored text
- a vertical-text pass, and a model with devanagari so hindi screenshots work
- batch several screenshots through the detector at once during backfill
- mac and windows: the core and ui are portable, the watcher and overlay aren't yet
- search by what's in the picture, not just its text ("the screenshot with a cat"): a small clip-style image embedding per shot, stored next to the fts index, queried with the text side of the same model. has to stay local, opt-in (the model is a download of its own), and cheap on cpu, so a small model and the same idle-priority backfill. asked about on X by @jabr7_isaf, 2026-10-05. thread notes: rank as a hybrid of the fts (bm25) score and image similarity; pick a small, low-dimension model newer than original clip (mobileclip / siglip class) and keep it resident only while the window is. no hnsw needed at this scale: 10k shots x 256 dims x int8 is 2.5 MB, and a flat scan of that is well under a millisecond, so the 1 to 10 ms budget holds without an index that has to live in ram
- clipboard-only screenshots (asked on X by @pantharshit007, 2026-10-05: "more than half of my ss are in my clipboard"). opt-in, off by default: watch the clipboard for images and save each one into its own folder (`~/Pictures/gyotaku clipboard` or similar) that's indexed like any other. per platform: `wl-paste --watch` on wayland, xfixes selection events on x11, AddClipboardFormatListener on windows. skip anything a password manager marks as sensitive (`x-kde-passwordManagerHint`, windows' ExcludeClipboardContentFromMonitorProcessing), skip duplicates by hash, and cap the folder's size so it can't fill a disk
- handwritten notes (asked on X by @jacintosalz, 2026-10-06). PP-OCRv6 is trained on printed text, so neat print mostly reads and messy handwriting mostly doesn't. first measure it on real photos of notes and screenshots of handwriting, then if it's worth it, a handwriting recognizer as an optional second pass, only on images the first pass found little text in
- a shortcuts page in settings: every key listed, click one and press the new combo to rebind it, conflicts flagged, reset to default. saved to config.toml under `[keys]` so it can be edited by hand too
- sort shots by what they are (otp, receipt, chat, code) by handing the OCR'd text to jev, so one-time stuff like otps can be found and binned in one go. jev needs the internet, so strictly opt-in, off by default, and only the text leaves the machine, never the image
- cyrillic, what's left after it went in:
  - capitals made only of latin-looking letters are read by the default reader at 0.99 to 1.00 ("АВАТАР" as "ABATAP", "РАЗМЕР" as "PA3MEP"), so the cyrillic reader never gets a say. fix it on the search side: add the cyrillic/latin twins (а/a, в/b, е/e, к/k, м/m, н/h, о/o, р/p, с/c, т/t, х/x, у/y, з/3, ч/4) and ё/е to the near-match fold, so "верх" finds "BEPX" and "черные" finds "чёрные"
  - terms under 3 characters go through LIKE, which ignores case for ascii only: "на" doesn't find "На", and greek has the same problem. a unicode-aware lower() registered with rusqlite would do
  - the default reader writes a space where it couldn't read a word, so "(Франция 18 века)" comes back as "( 18 )" at 1.00 with no gap to give it away. counting space steps as blank in `ctc_decode` caught 2 more lines on 6 hard russian images and changed nothing else on 30 real screenshots, but it changes when devanagari reads a line again too, so it needs the devanagari numbers re-measured first
  - a line of greek capitals the default reader is unsure of (0.85) can come back from the cyrillic reader as cyrillic look-alikes (0.96)
