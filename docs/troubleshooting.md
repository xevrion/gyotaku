# Troubleshooting

If your problem is not listed here, [open an issue](https://github.com/xevrion/gyotaku/issues/new/choose) and include your distribution, desktop environment, and the output of `RUST_LOG=warn gyotaku-app --once` run from a terminal.

- [Installing](#installing)
- [Building](#building)
- [Launching](#launching)
- [Windows](#windows)
- [Searching](#searching)
- [Background indexing](#background-indexing)
- [Clipboard](#clipboard)
- [Performance](#performance)

## Installing

### `couldn't download ... 404`

There is no release build for this machine, or GitHub could not be reached. Release builds exist for x86_64 and ARM64 Linux and x64 Windows. On anything else, [build from source](../README.md#build-from-source).

### `glibc ... is older than the 2.35 the release needs`, or `this system doesn't use glibc`

The release builds need Ubuntu 22.04, Debian 12, Fedora 36 or newer. Older and musl-based distributions (Alpine, Void musl) can [build from source](../README.md#build-from-source).

### `~/.local/bin isn't on your PATH`

The programs are installed but the shell can't find them by name. Most distributions add `~/.local/bin` to `PATH` at login once the folder exists, so logging out and back in usually fixes it; otherwise add the `export` line the installer printed to `~/.bashrc` or `~/.zshrc`. Keyboard shortcuts are unaffected as long as they use the full path, `~/.local/bin/gyotaku-app`.

### `Another gyotaku-app ... comes first on your PATH`

An earlier build from source in `~/.cargo/bin` is found before the installed release. Remove it with `cargo uninstall gyotaku gyotaku-app`, and update any keyboard shortcut that names `~/.cargo/bin/gyotaku-app`. Running the installer again afterwards points the background indexer at the new install.

### `The search window needs these libraries, which aren't installed`

This happens on minimal or server installs. Install the packages the installer lists, which every desktop environment already includes. `gyotaku` itself (the indexer and command line) works without them.

## Building

### Error mentioning `cold_path`, or requiring a newer `rustc`

The installed Rust version is older than 1.95. Install Rust with [rustup](https://rustup.rs) instead of the distribution package, and run `cargo install` from inside the cloned repository so that `rust-toolchain.toml` takes effect.

### Missing `fontconfig`, `xkbcommon`, `wayland` or `xcb`

A build dependency is missing. Re-run the command for your distribution from [step 1 of the installation](../README.md#1-install-build-dependencies).

### `linker 'cc' not found`

No C compiler is installed. It is included in step 1 (`build-essential`, `base-devel` or `gcc`).

### Bus error or "No space left on device"

The disk ran out of space during the build. About 3 GB of free space is required.

## Launching

### `gyotaku-app: command not found`

The install folder is not on your `PATH`: `~/.local/bin` for the installer (see [above](#localbin-isnt-on-your-path)), `~/.cargo/bin` for a build from source. Open a new terminal, or log out and back in.

### The keyboard shortcut does nothing

Run `RUST_LOG=warn gyotaku-app --once` in a terminal.

- If the window opens, the desktop environment cannot resolve the command. Use the absolute path from `which gyotaku-app` in the shortcut.
- If an error is printed, include it in an issue.

### The window opens as a regular window on GNOME

This is expected. GNOME does not implement the Wayland layer-shell protocol used for the overlay, so gyotaku falls back to a regular window.

### An old version opens after updating

The window process remains resident between uses. Stop it once with `pkill -x gyotaku-app`.

## Windows

### "Windows protected your PC"

The programs are not code-signed yet, so SmartScreen warns about an unknown publisher. Select **More info**, then **Run anyway**. The installer verifies each download against the checksum published with the release.

### Alt+Shift+S does nothing

Another program has claimed the key. Open gyotaku from the Start menu instead, or choose a different key in `%APPDATA%\gyotaku\config\config.toml`, then quit gyotaku (Ctrl+Q) and open it again:

```toml
[keys]
summon = "ctrl-alt-g"
```

### The window stays hidden after signing in

That is intended: when background indexing is on, gyotaku starts hidden at sign-in, ready for the summon key. It is listed in Task Manager under **Startup apps**, where it can also be turned off.

## Searching

### No results for anything

Run `gyotaku stats` to check the number of indexed screenshots. If it is zero:

1. Confirm that the folders in settings (Ctrl+,) are the folders your screenshots are saved to.
2. Confirm that background indexing is enabled and running: `systemctl --user status gyotaku-watch`.
3. Alternatively, index once manually with `gyotaku index`.

### A specific screenshot is not found

Run `gyotaku ocr path/to/screenshot.png` to see the text gyotaku extracts. If the text is missing or incorrect, open an issue with the `ocr-accuracy` label and, if it contains no private information, attach the screenshot.

### Text in Hindi, Russian, Korean, Arabic or other scripts is not found

For Hindi, Marathi, Nepali and other Devanagari text, turn on **Read Devanagari** in settings. It applies to screenshots taken from then on; ones read before keep their text as it was read. For Bangla and Assamese, turn on **Read Bengali** the same way. The other scripts are not supported yet. See [Compatibility](compatibility.md#language-support).

### Rotated or vertical text is not found

Horizontal lines and vertical columns are read; text at other angles is not. See [Compatibility](compatibility.md#language-support).

## Background indexing

### New screenshots are not indexed

```sh
systemctl --user status gyotaku-watch      # service state
journalctl --user -u gyotaku-watch -e      # most recent log output
```

On systems without systemd, confirm that `~/.config/autostart/gyotaku-watch.desktop` exists and that your desktop environment runs XDG autostart entries. The indexer can also be started manually with `gyotaku watch`.

### Indexing failed on first run without network access

The OCR models and ONNX Runtime could not be downloaded. Connect to the network; the systemd service retries every 30 seconds.

### Unsupported CPU architecture or musl-based distribution

Microsoft does not publish ONNX Runtime builds for these platforms. See [Compatibility](compatibility.md#cpu-architectures).

## Clipboard

### "couldn't copy the image, is wl-copy installed?"

Install `wl-clipboard` on Wayland or `xclip` on X11.

### Copied content disappears after the window closes

On Wayland, clipboard contents are owned by the application that set them. Install `wl-clipboard`; gyotaku then hands clipboard ownership to `wl-copy`, which persists after the window closes.

## Performance

### The window is slow or stutters

Compare against the figures in [Performance](performance.md). If no working GPU driver is available, rendering falls back to the CPU, which is slower. In that mode, animations are intentionally replaced with short fades.

### Indexing slows down other work

Indexing already runs at idle priority. On systems with few cores, reduce "Cores per screenshot" in settings.
