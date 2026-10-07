<p align="center">
  <img src="docs/banner.png" alt="gyotaku: search every screenshot by the text inside it" width="100%" />
</p>

<p align="center">
  Search every screenshot you have ever taken by the text inside it.<br>
  Native on Linux, macOS and Windows, fully offline, and fast on any hardware, with or without a GPU.
</p>

<h4 align="center">
  <a href="https://gyotaku.app">Website</a> |
  <a href="#installation">Installation</a> |
  <a href="docs/usage.md">Usage</a> |
  <a href="docs/troubleshooting.md">Troubleshooting</a> |
  <a href="CONTRIBUTING.md">Contributing</a>
</h4>

<p align="center">
  <a href="https://github.com/xevrion/gyotaku/actions/workflows/ci.yml"><img src="https://github.com/xevrion/gyotaku/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0-blue.svg" alt="GPL-3.0 licensed" /></a>
  <img src="https://img.shields.io/badge/platform-linux%20%7C%20windows%20%7C%20macos-blue.svg" alt="Linux, Windows and macOS" />
  <a href="CONTRIBUTING.md"><img src="https://img.shields.io/badge/PRs-welcome-brightgreen.svg" alt="PRs welcome" /></a>
</p>

gyotaku reads the text in every screenshot you take and makes it searchable. Press a shortcut, type a few letters of anything you remember seeing, and every matching screenshot appears with the matched words highlighted in place. Open one to copy its text, a selection of lines, or the image itself.

All processing happens locally. gyotaku does not take screenshots itself; it indexes the folders your existing screenshot tool saves to.

## Highlights

| Metric | Result |
|---|---|
| Search latency, 5,000+ screenshots | 1 to 10 ms per keystroke |
| Screenshot saved to searchable | 0.77 s |
| Window summon time | ~120 ms |
| Frame rate with a GPU | 144 fps (display refresh rate) |
| Memory while idle in the background | 37 MB |
| Disk usage | ~30 MB per 1,000 screenshots |

It also runs without a GPU, using software rendering. All figures were measured on real hardware; see [Performance](docs/performance.md) for the full results and methodology.

## Installation

### Linux

```sh
curl -fsSL https://raw.githubusercontent.com/xevrion/gyotaku/main/install.sh | sh
```

The script downloads the latest release for your machine, verifies its SHA-256 checksum and installs `gyotaku` and `gyotaku-app` to `~/.local/bin`, with an entry in your app launcher. It needs no root access and changes nothing outside your home directory. On a first install it opens gyotaku so you can choose your screenshot folders, then shows how to add a keyboard shortcut for your desktop.

Each release also has a `.deb` for Debian and Ubuntu and an `.rpm` for Fedora and openSUSE, which install to `/usr/bin` through your package manager:

```sh
# Debian, Ubuntu (replace x86_64 with aarch64 on ARM)
curl -fsSLO https://github.com/xevrion/gyotaku/releases/latest/download/gyotaku-x86_64-linux.deb
sudo apt install ./gyotaku-x86_64-linux.deb

# Fedora
sudo dnf install https://github.com/xevrion/gyotaku/releases/latest/download/gyotaku-x86_64-linux.rpm
```

These don't update on their own yet; install the newer one the same way. More packages are on the way, see [packaging](packaging).

Release builds run on x86_64 and ARM64 with glibc 2.35 or newer: Ubuntu 22.04, Debian 12, Fedora 36, Linux Mint 21, Pop!_OS 22.04 and later, as well as Kali, Arch Linux and openSUSE Tumbleweed. Anything else can [build from source](#build-from-source).

### Windows

Windows support is a preview. In PowerShell:

```powershell
irm https://raw.githubusercontent.com/xevrion/gyotaku/main/install.ps1 | iex
```

This installs to `%LOCALAPPDATA%\Programs\gyotaku`, adds gyotaku to the Start menu and opens it. Press **Alt+Shift+S** anywhere to open or close the search window. No administrator rights are needed. Windows 10 and 11 on x64 are supported; ARM64 runs the x64 build through emulation. While it waits, gyotaku sits in the notification area: click its icon to open the window, right-click it for settings or to quit. The window never takes a taskbar button, and Alt+F4 just puts it away.

If you prefer a regular installer, download [`gyotaku-setup-x86_64.exe`](https://github.com/xevrion/gyotaku/releases/latest/download/gyotaku-setup-x86_64.exe) from the latest release and run it. It installs the same files to the same folder, also without administrator rights, and lists gyotaku in Settings > Apps so it can be uninstalled from there. The setup is not code-signed yet, so SmartScreen may warn about an unknown publisher; choose **More info** and then **Run anyway**. The setup and the PowerShell command update each other's installs.

### macOS

In a terminal:

```sh
curl -fsSL https://raw.githubusercontent.com/xevrion/gyotaku/main/install.sh | sh
```

The same installer as on Linux: it downloads the release for Apple Silicon Macs, verifies its SHA-256 checksum and installs `gyotaku` and `gyotaku-app` to `~/.local/bin`, with ONNX Runtime beside them. Apple Silicon (arm64) is supported. Microsoft publishes no ONNX Runtime build for Intel macs, so those need a [build from source](#build-from-source) with `ORT_DYLIB_PATH` pointing at an `onnxruntime` library.

It also adds `gyotaku.app` to `~/Applications`, so Spotlight and Launchpad can open it. Like Raycast, it runs without a Dock icon: while it waits it sits in the menu bar, whose menu opens the window or settings and is where it's quit. Cmd+W and Cmd+Q in the window just put it away.

Config, index and models live in `~/Library/Application Support/gyotaku`, thumbnails in `~/Library/Caches/gyotaku`.

### Keyboard shortcut (Linux and macOS)

On macOS the app registers the shortcut itself: `Alt+Shift+S` opens the search window, and pressing it again closes it. To use another key, set `summon` under `[keys]` in `config.toml` (for example `summon = "cmd-shift-space"`) and restart gyotaku; it isn't in the settings yet.

On Linux, `gyotaku-app` opens the search window, and pressing the same shortcut again closes it. Bind it in your desktop's keyboard settings, using the full path `~/.local/bin/gyotaku-app`, since some desktops do not use your shell's `PATH`:

| Environment | Configuration |
|---|---|
| GNOME | Settings > Keyboard > View and Customize Shortcuts > Custom Shortcuts > Add |
| KDE Plasma | System Settings > Keyboard > Shortcuts > Add New > Command or Script |
| Xfce | Settings > Keyboard > Application Shortcuts > Add |
| Cinnamon | System Settings > Keyboard > Shortcuts > Custom Shortcuts > Add custom shortcut |
| sway, i3 | `bindsym $mod+s exec ~/.local/bin/gyotaku-app` |
| Hyprland | `bind = SUPER, S, exec, ~/.local/bin/gyotaku-app` |
| niri | `Mod+S { spawn "~/.local/bin/gyotaku-app"; }` in the `binds` block |
| Other | Any "run a command" shortcut. `gyotaku-app` can also be run from a terminal. |

### First run

On first launch, gyotaku asks:

1. **Which folders contain your screenshots.** It suggests the save locations of common screenshot tools (Flameshot, Spectacle, ksnip, grim, Hyprshot, niri) along with `~/Pictures/Screenshots`, `~/Pictures` and `~/Desktop`, with the number of images in each. On Windows it suggests `Pictures\Screenshots`, where Win+PrtScn and the Snipping Tool save.
2. **Whether to index new screenshots in the background.** On Linux this installs a systemd user service, or an XDG autostart entry on systems without systemd. On Windows it starts gyotaku when you sign in. On macOS the settings toggle "start at login" installs a launchd agent that reads new screenshots in the background.

Indexing starts immediately, newest screenshots first, at idle CPU and I/O priority. Search is available while it runs.

Before the first screenshot is read, gyotaku downloads the OCR models once (22 MB), and on Linux and macOS ONNX Runtime as well (24 MB on Linux). No network access is needed after that.

### Updating

Run the install command again, or on Windows the newest setup. It replaces the programs and restarts the background indexer; the index, settings and thumbnails are kept.

### Uninstalling

Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/xevrion/gyotaku/main/install.sh | sh -s -- --uninstall
```

Windows, in PowerShell:

```powershell
$env:GYOTAKU_UNINSTALL = 1; irm https://raw.githubusercontent.com/xevrion/gyotaku/main/install.ps1 | iex
```

Installed with the setup on Windows, gyotaku can also be uninstalled from Settings > Apps, which asks whether to delete the index and settings too.

macOS:

```sh
curl -fsSL https://raw.githubusercontent.com/xevrion/gyotaku/main/install.sh | sh -s -- --uninstall
```

These commands stop the background indexer and remove the programs, and print how to also remove the index and settings. On Linux, remove the keyboard shortcut yourself. gyotaku never modifies or deletes your screenshots unless you move them to the trash yourself.

### Build from source

Building needs Rust and a C compiler. The steps below are verified in [CI](https://github.com/xevrion/gyotaku/actions/workflows/ci.yml) from a clean image of Ubuntu 22.04, Debian 12, Kali, Arch Linux, Fedora and openSUSE Tumbleweed.

#### 1. Install build dependencies

**Ubuntu, Debian, Kali, Linux Mint, Pop!_OS**

```sh
sudo apt install git curl build-essential pkg-config libfontconfig-dev libxkbcommon-x11-dev libwayland-dev libx11-xcb-dev
```

**Arch Linux, Manjaro, EndeavourOS**

```sh
sudo pacman -S --needed git curl base-devel fontconfig libxkbcommon-x11 wayland libxcb
```

**Fedora**

```sh
sudo dnf install git curl gcc gcc-c++ make pkgconf-pkg-config fontconfig-devel libxkbcommon-x11-devel wayland-devel libxcb-devel
```

**openSUSE**

```sh
sudo zypper install git curl gcc gcc-c++ make pkg-config fontconfig-devel libxkbcommon-x11-devel wayland-devel libxcb-devel
```

Optional: install `wl-clipboard` (Wayland) or `xclip` (X11) to enable copying images, and saving copied images (off by default). Copying text works without them.

**macOS**: the indexer (`gyotaku`) builds with the Xcode Command Line Tools alone, which provide the C compiler that the bundled SQLite compiles with:

```sh
xcode-select --install
```

The search window (`gyotaku-app`) additionally needs full Xcode, free from the App Store, because the UI toolkit compiles Metal shaders at build time. With only the Command Line Tools installed that build fails with `cannot execute tool 'metal'`; if a fresh Xcode still lacks it, run `xcodebuild -downloadComponent MetalToolchain` once.

On an Intel mac, ONNX Runtime also has to be provided separately, since Microsoft publishes no macOS x86_64 build of it: install or build `onnxruntime` and set `ORT_DYLIB_PATH` to its library (see [Compatibility](docs/compatibility.md)). Apple Silicon needs nothing beyond Xcode; the runtime is downloaded on first use.

#### 2. Install Rust

gyotaku requires Rust 1.95 or newer. Install it with [rustup](https://rustup.rs) rather than your distribution's package manager, whose version is usually older:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Open a new terminal afterwards so that `cargo` is on your `PATH`. The repository's `rust-toolchain.toml` selects the correct version automatically.

#### 3. Build and install

```sh
git clone https://github.com/xevrion/gyotaku && cd gyotaku
cargo install --locked --path crates/cli     # gyotaku: the indexer and command-line interface
cargo install --locked --path crates/app     # gyotaku-app: the search window
```

Both binaries are installed to `~/.cargo/bin`; use `~/.cargo/bin/gyotaku-app` for the [keyboard shortcut](#keyboard-shortcut-linux-and-macos). The first build compiles the UI toolkit from source and takes several minutes. It requires about 3 GB of free disk space while running; the build directory is removed afterwards.

To update a source build, `git pull` and run both `cargo install` commands again, then `systemctl --user restart gyotaku-watch` and `pkill -x gyotaku-app` so the new version is used. To remove it, run the uninstall command above, then `cargo uninstall gyotaku gyotaku-app`.

If anything does not work as described, see [Troubleshooting](docs/troubleshooting.md).

## Usage

| Key | Action |
|---|---|
| Type | Search. Partial words match: `nutsmp` finds `donutsmp.net`. |
| Arrow keys | Move between results |
| Enter | Open the selected screenshot |
| Ctrl+C | Copy the screenshot's text, or the selected lines |
| Ctrl+Shift+C | Copy the image |
| Ctrl+, | Open settings |
| Ctrl+E | Show the similar screenshots folded behind the selected one |
| Ctrl+Shift+A, Ctrl+Delete | Mark every result, then move them to the trash. Ctrl+Z puts them back. |
| Escape | Clear the search, then close |

On macOS each Ctrl shortcut above is Cmd instead.

Every word in a query must appear somewhere in the screenshot, not necessarily on the same line. Filters narrow it down by folder and date: `otp in:discord date:week`. On an open screenshot, drag a box to copy only the lines inside it.

A command-line interface is also available:

```sh
gyotaku search invoice march
```

See the [usage guide](docs/usage.md) for all keys, settings and commands.

## Privacy

gyotaku runs entirely on your machine. It has no telemetry, accounts or update checks. Its only network access is the one-time download of the OCR models (from this repository's releases, with ModelScope as a fallback) and ONNX Runtime (Microsoft's official build, from GitHub), each verified against a pinned SHA-256 checksum before use.

## Roadmap

- [x] Move screenshots to the trash in bulk: search, mark the results, move them to the system trash, with undo
- [x] Simple installation on every supported OS: prebuilt releases and a one-command install, no Rust toolchain needed
- [x] A Windows setup `.exe`: Start menu entry, uninstall from Apps and features
- [ ] A macOS `.dmg` with a signed, notarized app
- [ ] Linux packages for every family, each a separate piece of the release pipeline: Flatpak on Flathub, the AUR, a Fedora COPR, an apt repository for Debian and Ubuntu, and openSUSE's OBS
- [x] A landing page with a demo, the measured numbers and the install commands
- [x] Windows support
- [x] macOS support on Apple Silicon (thanks to [@saurav-codes](https://github.com/saurav-codes))
- [x] macOS: shortcuts shown and bound with ⌘ instead of Ctrl, the way Mac apps do (thanks to [@Chiroyce1](https://github.com/Chiroyce1))
- [ ] macOS: confirm the summon key works after a real reboot, with the launch agent starting the app hidden
- [ ] macOS on Intel Macs
- [ ] Optional classification of screenshots (one-time codes, receipts, chats) with Jev, to find and clear out the throwaway ones. Opt-in and off by default; only the recognized text is sent, never the image
- [x] Typo-tolerant search for OCR misreads: look-alike characters (`0` and `O`, `rn` and `m`, `l` and `1`). Exact matches always rank first, and near matches are labelled as such, with the text exactly as it was read, so an error code is never silently "corrected"
- [ ] Words the OCR split apart (`ord er` for `order`), without matching words that really are apart: joining them naively found "HOURS IN VOICE" for `invoice`
- [ ] Search by what a screenshot shows, not only the text in it ("the one with a cat"), using a small local image embedding model such as CLIP. Optional, offline, and fast enough without a GPU. The most requested feature so far
- [x] A keyboard shortcuts page in settings that lists every shortcut and lets each one be rebound by pressing the new keys
- [ ] Screenshots that only ever go to the clipboard: an opt-in setting that saves images copied to the clipboard into a folder of their own, so they become searchable like any other screenshot. Works on Linux (Wayland and X11); the Windows side is written but not yet tried on a real machine; macOS needs a pasteboard watcher (saved screenshots already work there)
- [x] Group bursts of near-identical screenshots into one tile, with the rest a key away
- [x] Search filters for the folder (`in:discord`) and the date (`date:yesterday`, `before:aug`, `after:2026-08-01`)
- [ ] An `app:` filter, which first needs a way to know which app a screenshot was taken in: the file itself doesn't say
- [ ] A short title for every screenshot, generated from what it shows and says ("Order confirmation from Fern & Co"), shown on the tile and searchable, and optional auto-organizing into groups by those titles. Offline, like everything else
- [x] Devanagari (Hindi, Marathi, Nepali), opt-in in settings, and vertical text
- [ ] More scripts: Cyrillic, Hangul, Arabic, Thai and the rest
- [ ] Handwriting: measure how well neat and messy handwritten notes read, and add an optional handwriting pass if it's worth it

Suggestions are welcome as [feature requests](https://github.com/xevrion/gyotaku/issues/new?template=feature_request.yml).

## Documentation

| Document | Contents |
|---|---|
| [Usage](docs/usage.md) | Keys, mouse controls, settings, the command-line interface, file locations |
| [Troubleshooting](docs/troubleshooting.md) | Build, launch, search, indexing and clipboard problems |
| [Compatibility](docs/compatibility.md) | Supported desktops, GPUs, distributions and CPUs, and known limitations |
| [Performance](docs/performance.md) | Measured CPU, memory, frame time and disk usage |
| [Architecture](docs/architecture.md) | How OCR, indexing and the search window work, and why |
| [Development log](notes.md) | Measurements, rejected approaches and notable bugs from development |

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for the development setup and pull request guidelines. Questions belong in [Discussions](https://github.com/xevrion/gyotaku/discussions). Report security issues as described in [SECURITY.md](SECURITY.md), not in public issues.

Testing on untested configurations is particularly valuable. GNOME, KDE Plasma, integrated GPUs, ARM and non-systemd distributions have not been verified yet. Please [submit a compatibility report](https://github.com/xevrion/gyotaku/issues/new?template=compatibility_report.yml) whether it works or not.

## Built with

- [PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR) PP-OCRv6 models, run on [ONNX Runtime](https://onnxruntime.ai) via [`ort`](https://crates.io/crates/ort)
- SQLite FTS5 with the trigram tokenizer, via [`rusqlite`](https://crates.io/crates/rusqlite)
- [GPUI](https://gpui.rs), the UI framework behind the Zed editor

Text detection post-processing, line extraction, batching and decoding are implemented in this repository.

## About the name

Gyotaku (魚拓) is a traditional Japanese method of recording a catch: the fish is inked and pressed onto paper to make a lasting print. The search window borrows the idea. While you search, every screenshot is inked dark and only the matching words remain lit.

## Star history

<a href="https://star-history.com/#xevrion/gyotaku&Date">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=xevrion/gyotaku&type=Date&theme=dark" />
    <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=xevrion/gyotaku&type=Date" />
    <img alt="Star history for xevrion/gyotaku" src="https://api.star-history.com/svg?repos=xevrion/gyotaku&type=Date" />
  </picture>
</a>

## License

gyotaku is licensed under the [GNU General Public License v3.0 or later](LICENSE).

IBM Plex Sans is bundled under the SIL Open Font License 1.1 ([`crates/app/fonts/OFL.txt`](crates/app/fonts/OFL.txt)). The PaddleOCR models are licensed under Apache-2.0; they are downloaded at runtime and not distributed with this repository.
