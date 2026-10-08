# gyotaku for phones

An experimental Flutter app over the same index and search as the desktop app. It is not part of any release.

The phone reads its screenshots with the system's own text recognition (ML Kit on Android) and hands the lines to `gyotaku-core`, which stores and searches them exactly as it does on the desktop.

ML Kit reads no Bangla or Devanagari. When one of those scripts is turned on in the app's settings, screenshots are read by `crates/ocr` instead, the same PP-OCRv6 pipeline and extra recognizers as the desktop, on the ONNX Runtime that ships inside the app.

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

## Icon

`android/app/src/main/res/drawable/ic_launcher_foreground.xml` and the PNGs under `mipmap-*` and `ios/Runner/Assets.xcassets/AppIcon.appiconset` are drawn from `assets/icon.svg`. Redraw them if that file changes.
