# Performance

All figures on this page are measurements. None are estimates or extrapolations.

- [Test environment](#test-environment)
- [Search window](#search-window)
- [Indexing](#indexing)
- [Other scripts](#other-scripts)
- [Search](#search)
- [macOS on Apple Silicon](#macos-on-apple-silicon)
- [Disk usage](#disk-usage)
- [Measuring on your system](#measuring-on-your-system)

## Test environment

| Component | Specification |
|---|---|
| CPU | Intel Core i7-14700HX, 28 threads |
| GPU | NVIDIA GeForce RTX 4060 Laptop GPU |
| OS | Fedora 43 |

"No GPU" configurations hide all GPU drivers so that rendering uses Mesa's software renderer, as on a machine without a GPU. "4 cores" and "2 cores" configurations pin the process to that many cores with `taskset`.

## Search window

Each configuration ran one scripted session: open the window, type a word, open and close a screenshot, then press Page Down 15 times.

| Metric | GPU | No GPU, 28 threads | No GPU, 4 cores | No GPU, 2 cores |
|---|---|---|---|---|
| First launch to visible window | 0.5 to 0.7 s | 0.4 s | 0.4 s | 0.4 s |
| Subsequent launches | ~120 ms | | | |
| Frame time while scrolling, median | 7 ms (144 fps) | 41 to 45 ms | 78 ms | |
| Frame time while scrolling, 95th percentile | 31 to 39 ms | 74 to 80 ms | 98 ms | 98 ms |
| Frame build time (application code only) | 0.04 ms | 0.05 ms | 0.04 ms | 0.03 ms |
| CPU time for the session | 3.1 s | 28 to 31 s | 17 to 20 s | 12 to 14 s |
| Private memory after the session | 123 MB | 145 MB | 142 to 160 MB | 140 to 164 MB |
| Memory including shared libraries | 245 MB | 405 MB | 410 MB | 400 MB |
| Memory while hidden | 37 MB | | | |
| Escape to window closed | | | 124 ms | 122 ms |

Empty cells were not measured.

The window renders only in response to changes, so an idle open window consumes no CPU in any configuration. After the first launch, the process remains resident while hidden. Most of a cold start is GPU driver initialization, which subsequent launches avoid.

## Indexing

Indexing 25 real screenshots with the process pinned to a given number of cores:

| Cores | 1 | 2 | 4 |
|---|---|---|---|
| Time per screenshot | 1.32 s | 1.21 s | 0.67 s |
| Peak memory | 278 MB | 311 MB | 289 MB |

| Metric | Result |
|---|---|
| Screenshot saved to searchable | 0.77 s |
| Indexer memory between screenshots | ~60 MB |
| Full index of 5,159 screenshots, 6 threads | 43 min, 0 failures |
| Recall against PaddleOCR's default configuration | 99.5% of words |

Recall is measured on 25 randomly selected screenshots against the larger detection model at PaddleOCR's default settings. The tuned configuration uses a detector 4.6x faster. See [Architecture](architecture.md#text-recognition) for details.

## Other scripts

Reading Devanagari as well (**Read Devanagari** in settings), on the same 25 screenshots, none of which contain Devanagari, 4 cores:

| | Default | With Devanagari |
|---|---|---|
| Median time per screenshot, idle machine | 600 ms | 743 ms |
| Total for the 25, idle machine | 14.4 s | 17.5 s (+21%) |
| Total for the 25, machine under load, runs interleaved | 17.6 s | 17.4 s |
| Lines read a second time | | 148 of 564 (26%) |
| Lines kept | 456 | 456 |

Under load the difference disappears into the noise. The extra recognizer is a third the size of the default one, and only reads lines the default one was unsure of or left gaps in. On these screenshots it changed no line and added none.

Accuracy on rendered Hindi and Marathi screenshots (chat, payment receipt, settings page, municipal notice, an order page mixing Hindi and English), Noto Sans and Noto Serif Devanagari at 15 to 18 pt: every line was read, at scores of 0.91 to 0.99, where the default reader returned nothing or stray Latin letters. Misreads were in conjuncts and nasal marks, for example "पुणे" read as "पुण" and "रास्ते" as "रासत".

Reading Bengali as well (**Read Bengali** in settings) costs far more, because its recognizer is a larger and older design than the others. On an Apple M5 with 4 cores, a development build, three runs each:

| Screenshot (1080 x 2400) | Default | With Bengali |
|---|---|---|
| Bengali Wikipedia article, 33 lines kept | 0.76 s | 5.3 to 5.8 s |
| Android settings page in English, 28 lines kept | 0.59 to 0.60 s | 1.5 s |

On the Wikipedia screenshot the default reader returned stray Latin letters for every Bangla line ("GERNAGTGGARANA" at 0.51); the Bengali reader returned the 24 whole lines as text at scores of 0.93 to 1.00 and two fragments (part of a logo, a line cut off by the screen edge) at 0.80 and 0.81, with misreads in punctuation and the odd letter ("টারশিয়ারি" read as "টারশিযারি", "।" as "|"). On the English screenshot it changed no line and added one stray one ("88ঃ", from an icon). These are two screenshots, not a test set.

Vertical text costs nothing extra unless a screenshot has some: only boxes at least 1.5 times taller than wide are turned, and on 100 real screenshots the text read was identical with and without it.

## Search

1 to 10 ms per keystroke across more than 5,000 screenshots, including fetching the matched lines for the visible results.

Stacking [similar screenshots](usage.md#similar-screenshots) into one tile adds 0.02 ms per keystroke (median; 0.3 ms at most), and 0.7 ms when browsing all 6,863 screenshots of the test library. Grouping a library indexed before groups existed is a one-time background job of about 2.4 ms per screenshot, 17 s for that library, at idle priority.

## macOS on Apple Silicon

Measured on a MacBook Air M2 (8 cores, 16 GB RAM, macOS 27.0.1) while the machine was in everyday use, load average about 7.5, with the reader running at idle priority. `footprint` counts the pages the process keeps resident; `ps` RSS also counts every mapped shared page, such as AppKit, Metal and the ONNX Runtime library, which is why it reads higher than the private-memory figures in the tables above.

| Metric | Result |
|---|---|
| Search window, fresh launch, footprint | 44 MB, of which 24 MB is the window's Metal surface |
| Search window, resident and hidden, `ps` RSS | 106 to 113 MB |
| Search window CPU while hidden, 60 s sample | 0%, cputime unchanged |
| Reader `ps` RSS between screenshots | 150 MB, 177 MB right after one, falling back on its own |
| Reader CPU between screenshots, 60 s sample | 0% |
| Model load, warm | 59 to 110 ms |
| `gyotaku ocr`, one screenshot with 21 lines | 560 to 711 ms, peak 163 to 170 MB |
| Screenshot saved to searchable | 6.17 s |

## Disk usage

| Item | Size |
|---|---|
| Text index | ~6.8 MB per 1,000 screenshots (measured on 6,887) |
| Thumbnails | ~23 MB per 1,000 screenshots (24 KB each; regenerated on demand) |
| OCR models | 22 MB, downloaded once |
| ONNX Runtime | 24 MB, downloaded once |
| Binaries | 8 MB (`gyotaku`) and 34 MB (`gyotaku-app`) |

## Measuring on your system

- `GYOTAKU_FRAME_STATS=1 gyotaku-app --once` prints frame timing statistics when the window closes.
- `gyotaku ocr <image>` prints model load, decode and OCR times for a single image.

Results from other hardware, particularly integrated GPUs and low-power laptops, are welcome as a [compatibility report](https://github.com/xevrion/gyotaku/issues/new?template=compatibility_report.yml).
