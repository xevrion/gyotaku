# gyotaku for phones

An experimental Flutter app over the same index and search as the desktop app. It is not part of any release.

The phone reads its screenshots with the system's own text recognition (ML Kit on Android) and hands the lines to `gyotaku-core`, which stores and searches them exactly as it does on the desktop.

ML Kit reads no Bangla or Devanagari, so reading is in two passes. The first is ML Kit alone, about a quarter of a second an image, which makes a whole library searchable in minutes. The second only happens while one of those scripts is turned on in settings: each image is gone over again by `crates/ocr`, on the ONNX Runtime that ships inside the app, but only for the lines ML Kit was not sure of (`Ocr::read_rest`). That pass takes seconds an image.

Reading carries on with the app put away, in a foreground service (`android/.../ScanService.kt`) that shows the progress and a line at the end saying how it went. It can be paused from the notification or from the app, and stays paused, across restarts, until resumed. Swiping the app out of the recent apps list does not stop it: the app runs in one Flutter engine that outlives its window (`MainActivity.kt`).

## Layout

| Path | Contents |
|---|---|
| `lib/` | The Flutter app: the reader, the search grid, the open screenshot |
| `lib/src/rust/` | Generated bindings. Do not edit |
| `rust/` | The bridge crate. Depends on `../../crates/core` by path and is its own Cargo workspace |
| `rust_builder/` | Generated build glue that compiles `rust/` for each platform |

## Commands

```sh
flutter run                              # on a connected device or emulator
flutter analyze
(cd rust && cargo test)                  # the bridge, on the host
flutter_rust_bridge_codegen generate     # after changing rust/src/api
```

Building needs Flutter, the Android NDK and `cargo install flutter_rust_bridge_codegen`. The Rust targets for Android are installed on the first build.

## Status

Verified on an Android 17 arm64 emulator, in the dark theme, with five real screenshots:

- First run explains itself, asks for photo access, then reads `Pictures/Screenshots`.
- A partial word finds screenshots and the matching lines stay lit.
- The Today and Yesterday chips filter by date.
- An open screenshot can be swiped to the next result, shown without the ink, shared, and its text copied whole or one line at a time.
- The launcher shows the adaptive icon.
- Folders can be turned on and off in settings. Turning the Download folder on read its one image, and turning it off took that image out of search.
- Turning a script on shows each model downloading, with megabytes done and a bar, then the reading count.
- A release build reads with the phone's reader. It did not until ML Kit was kept from being shrunk, see `android/app/proguard-rules.pro`.
- With **Read Bengali** turned on in a freshly reset release build, the app downloaded the models itself, read the eight screenshots again with `crates/ocr`, and the Bangla lines of a Bengali Wikipedia screenshot came back as text.

Not done or not verified:

- iOS. The project and its icons are in place but it has never been built or run. The index keys a screenshot by file path, which the iOS photo library does not provide.
- The light theme, the refused-permission and no-album screens, pull to refresh, loading more than 120 results, the near matches heading, the This week and This month chips, and zooming have not been looked at on a device.
- Whether `date:` filters use the phone's timezone. They depend on `jiff` finding it on Android.
- The Bengali model is served from a release on the `zamansheikh/gyotaku` fork (`models-bengali-v1`). For upstream it belongs in the project's own models release.
- Typing a Bangla search on a device. Bangla search is covered by a test in `crates/core`, not by hand.
- How long the own readers take on a real phone.
- Reading in the background. Screenshots are read when the app opens or comes back to the front.
- A real device, a large library, and any measurement of speed or memory.
- Grouping of near-identical screenshots, moving to the trash, settings, and scripts other than Latin.

## Measured

Pixel 6, Android 17, release build, the phone in use at the time:

| | Time |
|---|---|
| First pass, 1,752 images | 6 min 37 s (0.23 s an image) |
| Second pass with Bengali and Devanagari on, 2,243 images | about 2 h 48 min, estimated by the app from its first 20 |
| Reading everything with `crates/ocr` and both scripts, as before the two passes | 9.7 s an image, the mean of 8 screenshots |
| The same 8 screenshots, ML Kit and then `read_rest` | 8.2 s an image |

With the second version of the Bengali model, on the same phone and library, by the app's own count over a minute or two at a time (the images differ, so these are rough):

| | An image |
|---|---|
| Second pass, app on screen | 2.6 to 6 s |
| Second pass, app put away, reading threads left where the system put them | about 19 s |
| Second pass, app put away, reading threads kept to the faster cores | about 9 s |

Put away, the system allows the app six of the phone's eight cores, four of them the small ones. A recognizer waits for its slowest thread, so the reader is kept to the faster cores it is still allowed.

Several ML Kit readers at once are not used: on the same phone 2, 3, 4 and 6 at once took as long as one (10.2 to 11.5 s against 11.7 s for 60 images) and returned 417, 234, 255 and 103 lines where one returned 1,004.

## Icon

`android/app/src/main/res/drawable/ic_launcher_foreground.xml` and the PNGs under `mipmap-*` and `ios/Runner/Assets.xcassets/AppIcon.appiconset` are drawn from `assets/icon.svg`. Redraw them if that file changes.
