# Usage

- [Searching](#searching)
- [Keyboard shortcuts](#keyboard-shortcuts)
- [Moving screenshots to the trash](#moving-screenshots-to-the-trash)
- [Mouse](#mouse)
- [Settings](#settings)
- [Background indexing](#background-indexing)
- [Copied images](#copied-images)
- [Other scripts](#other-scripts)
- [Command-line interface](#command-line-interface)
- [Flags and environment variables](#flags-and-environment-variables)
- [File locations](#file-locations)

## Searching

Open the search window with your shortcut and start typing. Results update on every keystroke.

The window opens on the monitor being worked on wherever the system can say which one that is: on Windows, the monitor holding the window that was in front; on Wayland, the output the compositor treats as in use. Elsewhere it opens on the primary monitor.

| Behavior | Detail |
|---|---|
| Partial matching | Any substring matches. `nutsmp` finds `donutsmp.net`, and `invoi` finds `invoice`. Words the OCR misread slightly remain findable from their correct parts. |
| Misread words | After the exact matches come near matches, screenshots where a word was read with one look-alike character in place of another: `0RDER` for `order`, `rnodern` for `modern`, `E0425` for `EO425`. They are labelled `near match`, and the text is shown exactly as it was read. Words under four characters only match exactly. |
| Multiple words | All words must appear in the screenshot, in any order and on any line. `invoice march` matches a screenshot containing both. |
| Case | Matching is case-insensitive. |
| Special characters | Input is always treated as literal text. `c++`, `NOT` and unbalanced quotes are valid queries. |
| Short words | Words of one or two characters are supported. The index is built from three-character sequences, so these are matched by scanning the results of the other terms. |
| Ordering | Results are ranked by match quality, then by modification time, newest first. An empty query lists all screenshots, newest first. |

While a query is active, each thumbnail is dimmed and the matched lines are highlighted in place.

### Filters

A word of the form `key:value` with one of the keys below narrows the results by where or when a screenshot was taken. Filters combine with words and with each other, and are shown dimmed in the search box once they are complete. A query made only of filters, such as `date:today`, lists every screenshot they allow, newest first.

| Filter | Matches |
|---|---|
| `in:<name>` | Screenshots inside a folder whose name starts with `<name>`, at any depth and ignoring case: `in:disc` for a `Discord` folder. Quote names with spaces: `in:"my shots"`. |
| `date:<when>` | Screenshots taken during that time. |
| `before:<when>` | Screenshots taken before it starts. |
| `after:<when>` | Screenshots taken from the time it starts onward. `after:2026-08` includes August. |

`<when>` is one of:

| Value | Means |
|---|---|
| `today`, `yesterday` | That day, in your local time. |
| `week`, `month` | The last 7 or 30 days, including today. |
| `2026-08-01`, `2026-08`, `2026` | That day, month or year. |
| `aug`, `august` | The most recent August: this year's if it has started, otherwise last year's. Any three or more letters of a month name work. |

"Taken" means the file's modification time, which is when it was saved unless it was edited afterwards.

Words with a colon that aren't one of these keys, like `https://`, `12:30` or `error:`, are searched as text, and so are bare words like `today`. A filter that isn't finished yet, such as `date:yes` on the way to `yesterday`, is ignored rather than searched for, so the results don't empty while you type it.

There is no `app:` filter: a screenshot file records nothing about the app it was taken in, and the names screenshot tools give their files don't either.

### Similar screenshots

Screenshots of the same thing taken close together, like a chat captured a few times while scrolling or one page shot twice, show as a single tile marked `+N similar`. Press Ctrl+E, or click the label, to show the rest right after it; again (or `hide N`) to fold them back.

| Behavior | Detail |
|---|---|
| What counts as similar | Taken within 10 minutes of another screenshot in the group, and either with nearly all the same text, with most of the same text moved up or down (a scroll, same size, within 2 minutes), or, for screenshots with little or no text, a nearly identical picture. Different pages of one app share its sidebar and toolbar, but not the rest of the text, so they stay apart. |
| Which one shows | The best match for the search, or the newest when browsing. |
| Counts | The count in the header includes every screenshot found, folded ones too. |
| Marking and the trash | A folded tile stands for its whole group: marking it, or moving it to the trash, includes the screenshots behind it, and the count says so. |
| Existing screenshots | Grouped in the background after an update, at idle priority, which takes about 20 seconds for 7,000 screenshots. Until then they show one by one. |
| Turning it off | Settings, "group similar screenshots". |

## Keyboard shortcuts

| Key | Result grid | Open screenshot |
|---|---|---|
| Typing | Search | Return to the grid and search |
| Arrow keys, Page Up, Page Down | Move the selection | Left and Right move to the adjacent result |
| Enter | Open the selected screenshot | Open the file in the default image viewer |
| Ctrl+C | Copy the screenshot's text | Copy the selected lines, or all text if none are selected |
| Ctrl+Shift+C | Copy the image | Copy the image |
| Ctrl+O | Open the file in the default image viewer | Same |
| Ctrl+Shift+O | Show the file in its folder | Same |
| Ctrl+, | Open settings | |
| Ctrl+E | Show or hide the [similar screenshots](#similar-screenshots) folded behind the selected one | |
| Shift+Arrow keys | Mark a run of screenshots | |
| Ctrl+Shift+A | Mark every result of the current search | |
| Ctrl+Delete | Move the marked screenshots, or the selected one, to the trash | Move this screenshot to the trash |
| Ctrl+Z | Put back what was last moved to the trash | |
| Escape | Cancel a pending move, clear marks, clear the search, then close the window | Return to the grid |

The command shortcuts above are defaults and can be changed in settings, see [Settings](#settings). Changes are stored in `config.toml` under `[keys]`, for example `trash = "ctrl-backspace"`; an entry that is invalid or already in use falls back to the default. On macOS each default uses Cmd instead of Ctrl, written `cmd` in `config.toml` (for example `settings = "cmd-,"`).

The shortcut bound to `gyotaku-app` also closes the window. When reopened, the window restores its previous state: the same query, selection and open screenshot.

## Moving screenshots to the trash

Search for what you no longer need, mark the results, and move them to the trash in one step. For example, search `otp`, press Ctrl+Shift+A to mark every match, then Ctrl+Delete.

- Nothing is deleted. Screenshots go to the system trash following the [freedesktop.org trash specification](https://specifications.freedesktop.org/trash-spec/latest/), so they appear in your file manager's trash and can be restored from there.
- Screenshots on another drive go to that drive's own trash (`.Trash-<uid>` at its root) and are never copied across drives.
- Ctrl+Delete always asks for confirmation, showing how many screenshots will be moved. Press Enter (or Ctrl+Delete again) to confirm, Escape to cancel.
- Ctrl+Z puts the last batch back in place, with its text, so nothing is read again. A screenshot is not restored over a new file that has since taken its name.
- Marking all results requires an active search, so the entire library cannot be marked by accident.
- Marks are cleared when the search changes.

## Mouse

- Click a thumbnail to open it.
- Ctrl+click a thumbnail to mark or unmark it. Shift+click marks every thumbnail between the selection and the one clicked.
- Click `+N similar` on a thumbnail to show the similar screenshots folded behind it, and `hide N` to fold them back.
- The bar that appears while screenshots are marked can also be clicked.
- On an open screenshot, hover to show the detected lines, click a line to copy it, or drag a rectangle to copy every line it intersects.

## Settings

Open settings with Ctrl+,.

| Setting | Description |
|---|---|
| Folders | Folders to index. The home directory and `/` are rejected; select the specific folders your screenshots are saved to. |
| Theme | System, light or dark. |
| Group similar screenshots | On by default. Shows near-identical screenshots taken close together as one tile. See [Similar screenshots](#similar-screenshots). |
| Background indexing | Enables or disables the background indexer. See [Background indexing](#background-indexing). |
| Save copied images | Off by default. Saves every image copied to the clipboard into its own folder, so images that were never saved anywhere become searchable. See [Copied images](#copied-images). |
| Read Devanagari | Off by default. Also reads Hindi, Marathi, Nepali and other text in Devanagari. See [Other scripts](#other-scripts). |
| Read Bengali | Off by default. Also reads Bangla, Assamese and other text in the Bengali script. See [Other scripts](#other-scripts). |
| Cores per screenshot | Number of CPU cores used to read a single screenshot. Higher values are faster; lower values leave more capacity for other work. Indexing always runs at idle priority. |
| Thumbnail cache | Clears cached thumbnails. They are regenerated on demand. |
| Shortcuts | Lists every keyboard shortcut. Select a command and press Enter, then press the new keys; Escape cancels and Delete restores the default. New keys must include Ctrl, Alt or Super (or be a function key) and must not already be in use. Navigation keys (Escape, Enter, arrows, Page Up and Page Down, Shift+arrows) are fixed. |

Settings are stored in `~/.config/gyotaku/config.toml` and can also be edited directly. The background indexer applies changes to the folder list, to saving copied images and to the scripts it reads without a restart.

## Background indexing

`gyotaku watch` indexes every image in the configured folders, then monitors them with inotify and indexes new screenshots as they are saved (0.77 s from save to searchable on the reference machine). Renamed and moved files keep their existing text without being read again. Deleted files are removed from the index.

Resource usage is constrained:

- The process runs at idle CPU and I/O scheduling priority.
- On battery power, processing of an existing backlog slows to one screenshot every few seconds. New screenshots are still indexed immediately.
- The systemd service limits memory to 400 MB (`MemoryHigh`).
- Images larger than about 64 megapixels, or smaller than 16 pixels on either side, are skipped.

Enabling background indexing during first run or in settings installs one of the following:

| System | Mechanism |
|---|---|
| systemd | User service at `~/.config/systemd/user/gyotaku-watch.service`, enabled and started immediately |
| Other init systems | XDG autostart entry at `~/.config/autostart/gyotaku-watch.desktop`, started at login |

To install the service manually, copy [`contrib/gyotaku-watch.service`](../contrib/gyotaku-watch.service) to `~/.config/systemd/user/` and run:

```sh
systemctl --user enable --now gyotaku-watch
```

Status and logs:

```sh
systemctl --user status gyotaku-watch      # service state
journalctl --user -u gyotaku-watch -f      # follow the log
gyotaku stats                              # index location and size
```

## Copied images

With **Save copied images** enabled, the background indexer watches the clipboard and saves each image copied to it as a PNG named like `Clipboard 2026-10-06 14.03.22.png`. The folder defaults to `Clipboard` inside Pictures and can be changed with `clipboard_folder` in `config.toml`. It is indexed like any configured folder, and stays indexed after the setting is turned off, for as long as it exists.

An image is not saved when:

- it is smaller than 32 pixels on either side;
- the same image was saved a moment ago;
- it is already a file in an indexed folder, as with screenshot tools that both save and copy, and with screenshots copied out of gyotaku with Ctrl+Shift+C;
- it was copied as a file in a file manager, or marked as private by the application that copied it (password managers do this).

Nothing watches the clipboard while the setting is off, and only images are ever read from it.

| System | Mechanism | Requires |
|---|---|---|
| Wayland | `wl-paste --watch`, through the compositor's data control protocol | `wl-clipboard`, and a compositor with data control (wlroots based compositors, KDE Plasma and niri have it; GNOME does not) |
| X11 | XFixes selection events, read with `xclip` | `xclip` |
| Windows | A clipboard format listener | Nothing |

Because only the background indexer watches the clipboard, copied images are saved only while it runs. Messages about the clipboard watcher appear in its log (`journalctl --user -u gyotaku-watch` on systemd).

## Other scripts

The default text reader covers Latin, Chinese, Japanese and Greek. With **Read Devanagari** enabled, Hindi, Marathi, Nepali and other text in Devanagari is read too, including lines that mix it with English. In `config.toml` this is:

```toml
scripts = ["devanagari"]
```

Enabling it downloads a 7.9 MB model the first time. If the download fails, for example offline, reading continues without it and the status line says so; the next change to the settings tries again.

It applies to screenshots read from then on. Screenshots read before keep the text they were read with, since reading the whole library again would take as long as the first time. To read a particular screenshot again, touch it (`touch path/to/screenshot.png`), and the background indexer picks it up as changed.

Lines that the default reader already read with confidence, and that contain no Devanagari, are never read twice, so the extra cost depends on the screenshot. On 25 screenshots with no Devanagari in them, about a quarter of lines were read a second time. See [Performance](performance.md#other-scripts).

To check a single image: `gyotaku ocr --script devanagari path/to/screenshot.png`.

**Read Bengali** works the same way for Bangla, Assamese and other text in the Bengali script, with `scripts = ["bengali"]` in `config.toml` and `--script bengali` on the command line. Its model is a 54 MB download and is much slower than the others: a screenshot full of Bangla took 3.1 to 3.3 s to read instead of 0.75 s, and one with no Bangla in it 0.9 s instead of 0.6 s. See [Performance](performance.md#other-scripts).

## Command-line interface

| Command | Description |
|---|---|
| `gyotaku search <words>...` | Print matching screenshot paths and the lines that matched. Accepts the same [filters](#filters) as the search window; quote one with spaces for the shell, `'in:"my shots"'`. `-n, --limit <n>` sets the maximum number of results (default 20). |
| `gyotaku stats` | Print file locations and the number of indexed screenshots. |
| `gyotaku ocr <image>` | Read one image and print its text without indexing it. `--boxes` also prints each line's bounding box and confidence. |
| `gyotaku index [folders]...` | Index the given folders, or the configured folders, then exit. |
| `gyotaku watch [folders]...` | Index, then continue monitoring for new screenshots. Without arguments, follows configuration changes. |

`index`, `watch` and `ocr` accept `--threads <n>` to set the number of cores used per screenshot. Run `gyotaku --help` or `gyotaku <command> --help` for full details.

## Flags and environment variables

| Option | Effect |
|---|---|
| `gyotaku-app --window` | Open as a regular window instead of an overlay. |
| `gyotaku-app --once` | Exit when the window closes instead of remaining resident in the background. |
| `GYOTAKU_THEME=light\|dark` | Override the theme setting. |
| `GYOTAKU_FRAME_STATS=1` | Print frame timing statistics when the window closes. |
| `RUST_LOG=warn` | Print warnings from the window and GPU layers. |
| `ORT_DYLIB_PATH=<path>` | Use an existing `libonnxruntime.so` instead of downloading one. See [Compatibility](compatibility.md#cpu-architectures). |

## File locations

| Path | Contents |
|---|---|
| `~/.config/gyotaku/config.toml` | Folders, theme, cores per screenshot, extra scripts |
| `~/.local/share/gyotaku/index.db` | Text index |
| `~/.local/share/gyotaku/models/` | OCR models |
| `~/.local/share/gyotaku/runtime/` | ONNX Runtime |
| `~/.cache/gyotaku/thumbs/` | Thumbnails. Safe to delete. |

gyotaku never modifies, moves or deletes screenshots.
