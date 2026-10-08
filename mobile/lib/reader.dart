import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:google_mlkit_text_recognition/google_mlkit_text_recognition.dart';
import 'package:photo_manager/photo_manager.dart';

import 'folders.dart';
import 'src/rust/api/index.dart' as core;
import 'src/rust/api/reader.dart' as own;

enum ReaderState {
  /// Not looked yet.
  idle,

  /// Never asked for the photos. The app explains itself before it asks.
  unasked,

  /// Asked and refused; only the system settings can change that now.
  refused,

  /// Allowed, but no folder is turned on, or the phone has none.
  noFolder,

  /// Going through the folders to find what's new.
  checking,

  /// Downloading and loading gyotaku's own readers.
  preparing,
  reading,
  done,
}

/// Finds the images in the folders that are turned on, reads the ones the
/// index doesn't know yet, and hands the lines to the index. Newest first, so
/// the shots most likely to be searched for are there within seconds.
///
/// It only ever reads. Nothing here writes to, moves or deletes a photo.
///
/// Everything it is doing is in its fields, so the app can always say what
/// is going on and how far along it is.
class Reader extends ChangeNotifier {
  ReaderState state = ReaderState.idle;

  /// While checking: images looked at, of how many.
  int checked = 0;
  int toCheck = 0;

  /// While preparing: the model coming down, or null while they load.
  String? downloading;
  int downloadedKb = 0;
  int downloadKb = 0;

  /// While reading: of the images that were new this run.
  int done = 0;
  int total = 0;

  /// The file being read right now.
  String? current;

  /// Whether this run reads with gyotaku's own recognizers. They are only
  /// used while an extra script is on: the phone's reader is faster and needs
  /// no download, but knows no Bangla or Devanagari.
  bool own_ = false;

  /// Why gyotaku's own readers couldn't be used, if they couldn't: a model
  /// that wouldn't download, most likely. The phone's reader carries on in
  /// their place.
  String? trouble;

  /// Images that could not be read this run, and what the last one said.
  /// Without this a run where every image fails looks like an empty phone.
  int failed = 0;
  String? failure;

  /// Goes up each time the index changed enough for the grid to be worth
  /// refreshing, so the page doesn't re-query on every single image.
  int generation = 0;

  bool _running = false;
  bool _again = false;

  static const _option = PermissionRequestOption();

  bool get busy =>
      state == ReaderState.checking ||
      state == ReaderState.preparing ||
      state == ReaderState.reading;

  /// Looks for new images. With `ask` it may show the system's permission
  /// prompt; without, it stays quiet and reports `unasked`. With `quiet`, a
  /// look that finds nothing new shows nothing, for the one made each time
  /// the app comes back to the front.
  Future<void> run({bool ask = false, bool quiet = false}) async {
    if (_running) {
      // Something changed while it was busy: go round once more after.
      _again = true;
      return;
    }
    _running = true;
    try {
      await _run(ask, quiet);
    } catch (e) {
      // A broken library or recogniser leaves search working on what's
      // already indexed.
      debugPrint('gyotaku: reading stopped: $e');
      _set(ReaderState.done);
    } finally {
      _running = false;
    }
    if (_again) {
      _again = false;
      await run();
    }
  }

  /// After the index was emptied: reads the library again from the top.
  Future<void> startOver() async {
    generation++;
    notifyListeners();
    await run();
  }

  /// Downloads and loads gyotaku's own readers now, for when a script has
  /// just been turned on, so the wait is seen where it was asked for.
  Future<void> prepare() async {
    if (_running) {
      _again = true;
      return;
    }
    _running = true;
    final before = state;
    try {
      await _prepare();
    } finally {
      _running = false;
      _set(before == ReaderState.idle ? ReaderState.done : before);
    }
  }

  Future<void> _run(bool ask, bool quiet) async {
    var permission = await PhotoManager.getPermissionState(
      requestOption: _option,
    );
    if (!permission.hasAccess) {
      if (!ask) {
        _set(
          state == ReaderState.refused
              ? ReaderState.refused
              : ReaderState.unasked,
        );
        return;
      }
      permission = await PhotoManager.requestPermissionExtend(
        requestOption: _option,
      );
      if (!permission.hasAccess) {
        _set(ReaderState.refused);
        return;
      }
    }

    final choices = await FolderChoices.load();
    final folders = [
      for (final f in await imageFolders())
        if (choices.isOn(f)) f,
    ];

    final counts = [for (final f in folders) await f.assetCountAsync];
    checked = 0;
    toCheck = counts.fold(0, (a, b) => a + b);
    if (!quiet || state != ReaderState.done) _set(ReaderState.checking);

    final fresh = <_Fresh>[];
    final kept = <String>[];
    for (var n = 0; n < folders.length; n++) {
      for (var start = 0; start < counts[n]; start += _page) {
        final assets = await folders[n].getAssetListRange(
          start: start,
          end: (start + _page).clamp(0, counts[n]),
        );
        for (final asset in assets) {
          final seen = await _Seen.of(asset);
          if (seen != null) {
            kept.add(seen.path);
            if (seen.fresh != null) fresh.add(seen.fresh!);
          }
          checked++;
        }
        if (state == ReaderState.checking) notifyListeners();
      }
    }

    // What's left in the index but no longer in a folder that's on: deleted
    // from the phone, or its folder was turned off.
    if (await core.keepOnly(paths: kept) > 0) generation++;

    if (fresh.isEmpty) {
      _set(folders.isEmpty ? ReaderState.noFolder : ReaderState.done);
      return;
    }

    // Newest first across every folder, not one folder after another.
    fresh.sort((a, b) => b.mtime.compareTo(a.mtime));

    try {
      own_ = (await own.scripts()).any((s) => s.enabled);
    } catch (e) {
      own_ = false;
    }
    if (own_) await _prepare();

    done = 0;
    failed = 0;
    failure = null;
    total = fresh.length;
    _set(ReaderState.reading);
    final recognizer = TextRecognizer(script: TextRecognitionScript.latin);
    try {
      for (final f in fresh) {
        current = f.path.split('/').last;
        notifyListeners();
        await _read(f, recognizer);
        done++;
      }
    } finally {
      await recognizer.close();
      current = null;
    }
    generation++;
    _set(ReaderState.done);
  }

  /// Gets gyotaku's own readers ready, saying how far each download is.
  Future<void> _prepare() async {
    trouble = null;
    downloading = null;
    _set(ReaderState.preparing);
    final poll = Timer.periodic(const Duration(milliseconds: 150), (_) {
      final d = own.downloadProgress();
      final file = d?.file;
      if (file != downloading ||
          (d?.doneKb ?? 0) != downloadedKb ||
          (d?.totalKb ?? 0) != downloadKb) {
        downloading = file;
        downloadedKb = d?.doneKb ?? 0;
        downloadKb = d?.totalKb ?? 0;
        notifyListeners();
      }
    });
    try {
      await own.loadReaders();
      own_ = true;
    } catch (e) {
      // No network, no space, a file that changed: say so, and let the
      // phone's reader take over.
      debugPrint('gyotaku: own readers failed: $e');
      trouble = '$e';
      own_ = false;
    } finally {
      poll.cancel();
      downloading = null;
    }
  }

  static const _page = 100;
  static const _refreshEvery = 6;
  int _sinceRefresh = 0;

  Future<void> _read(_Fresh f, TextRecognizer recognizer) async {
    try {
      List<core.Line>? lines;
      if (own_) {
        try {
          lines = await own.readImage(path: f.path);
        } catch (e) {
          debugPrint('gyotaku: own reader failed: $e');
          trouble = '$e';
          own_ = false;
        }
      }
      lines ??= await _withPhone(f, recognizer);
      await core.insertShot(
        path: f.path,
        mtime: f.mtime,
        width: f.width,
        height: f.height,
        lines: lines,
      );
      // The own readers take seconds an image, so each one is shown as it
      // lands rather than a handful at a time.
      if (own_ || ++_sinceRefresh >= _refreshEvery) {
        _sinceRefresh = 0;
        generation++;
      }
    } catch (e) {
      // One unreadable image must not stop the rest. It is tried again on
      // the next launch.
      debugPrint('gyotaku: could not read ${f.path}: $e');
      failed++;
      failure = '$e'.split('\n').first;
    }
    notifyListeners();
  }

  Future<List<core.Line>> _withPhone(
    _Fresh f,
    TextRecognizer recognizer,
  ) async {
    final text = await recognizer.processImage(InputImage.fromFilePath(f.path));
    final w = f.width, h = f.height;
    final lines = <core.Line>[];
    for (final block in text.blocks) {
      for (final line in block.lines) {
        final b = line.boundingBox;
        lines.add(
          core.Line(
            text: line.text,
            rect: core.Rect(
              x: (b.left / w).clamp(0.0, 1.0),
              y: (b.top / h).clamp(0.0, 1.0),
              w: (b.width / w).clamp(0.0, 1.0),
              h: (b.height / h).clamp(0.0, 1.0),
            ),
            score: line.confidence ?? 1.0,
          ),
        );
      }
    }
    return lines;
  }

  void _set(ReaderState s) {
    state = s;
    notifyListeners();
  }
}

/// An image in a folder that's on, and whether it still needs reading.
class _Seen {
  _Seen(this.path, this.fresh);

  final String path;
  final _Fresh? fresh;

  static Future<_Seen?> of(AssetEntity asset) async {
    try {
      if (asset.width <= 0 || asset.height <= 0) return null;
      final file = await asset.originFile;
      if (file == null) return null;
      final mtime = asset.modifiedDateSecond ?? asset.createDateSecond ?? 0;
      if (await core.isCurrent(path: file.path, mtime: mtime)) {
        return _Seen(file.path, null);
      }
      return _Seen(
        file.path,
        _Fresh(file.path, mtime, asset.width, asset.height),
      );
    } catch (e) {
      debugPrint('gyotaku: could not look at ${asset.title}: $e');
      return null;
    }
  }
}

/// An image the index hasn't read, or has read an older version of.
class _Fresh {
  _Fresh(this.path, this.mtime, this.width, this.height);

  final String path;
  final int mtime;
  final int width;
  final int height;
}
