import 'package:flutter/foundation.dart';
import 'package:google_mlkit_text_recognition/google_mlkit_text_recognition.dart';
import 'package:photo_manager/photo_manager.dart';

import 'src/rust/api/index.dart' as core;
import 'src/rust/api/reader.dart' as own;

enum ReaderState {
  /// Not looked yet.
  idle,

  /// Never asked for the photos. The app explains itself before it asks.
  unasked,

  /// Asked and refused; only the system settings can change that now.
  refused,

  /// Allowed, but the library has no screenshots album.
  noFolder,

  /// Working out which screenshots are new. Quick, and not worth a word.
  checking,
  reading,
  done,
}

/// Finds the screenshots in the photo library, reads the ones the index
/// doesn't know yet with the system's text recognition, and hands the lines
/// to the index. Newest first, so the shots most likely to be searched for
/// are there within seconds.
///
/// It only ever reads. Nothing here writes to, moves or deletes a photo.
class Reader extends ChangeNotifier {
  ReaderState state = ReaderState.idle;

  /// Of the screenshots that were new this run.
  int done = 0;
  int total = 0;

  /// Goes up each time enough new shots are in for the grid to be worth
  /// refreshing, so the page doesn't re-query on every single one.
  int generation = 0;

  /// Why gyotaku's own readers couldn't be used this run, if they couldn't:
  /// a model that wouldn't download, most likely. The phone's reader carried
  /// on in their place.
  String? trouble;

  bool _running = false;

  /// Whether this run reads with gyotaku's own recognizers. They are only
  /// used while an extra script is on: the phone's reader is faster and needs
  /// no download, but knows no Bangla or Devanagari.
  bool _own = false;

  static const _option = PermissionRequestOption();

  /// Looks for new screenshots. With `ask` it may show the system's
  /// permission prompt; without, it stays quiet and reports `unasked`.
  Future<void> run({bool ask = false}) async {
    if (_running) return;
    _running = true;
    try {
      await _run(ask);
    } catch (e) {
      // A broken library or recogniser leaves search working on what's
      // already indexed.
      debugPrint('gyotaku: reading stopped: $e');
      _set(ReaderState.done);
    } finally {
      _running = false;
    }
  }

  Future<void> _run(bool ask) async {
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

    final folders = await screenshotFolders();
    if (folders.isEmpty) {
      _set(ReaderState.noFolder);
      return;
    }

    // A rescan with nothing new should not flash "reading" at anyone.
    if (state != ReaderState.done) _set(ReaderState.checking);
    final fresh = <_Fresh>[];
    for (final folder in folders) {
      final count = await folder.assetCountAsync;
      for (var start = 0; start < count; start += _page) {
        final assets = await folder.getAssetListRange(
          start: start,
          end: (start + _page).clamp(0, count),
        );
        for (final asset in assets) {
          final f = await _Fresh.of(asset);
          if (f != null) fresh.add(f);
        }
      }
    }
    if (fresh.isEmpty) {
      _set(ReaderState.done);
      return;
    }

    done = 0;
    total = fresh.length;
    trouble = null;
    try {
      _own = (await own.scripts()).any((s) => s.enabled);
    } catch (e) {
      _own = false;
    }
    _set(ReaderState.reading);
    final recognizer = TextRecognizer(script: TextRecognitionScript.latin);
    try {
      for (final f in fresh) {
        await _read(f, recognizer);
        done++;
        notifyListeners();
      }
    } finally {
      await recognizer.close();
    }
    generation++;
    _set(ReaderState.done);
  }

  static const _page = 100;
  static const _refreshEvery = 6;
  int _sinceRefresh = 0;

  Future<void> _read(_Fresh f, TextRecognizer recognizer) async {
    try {
      List<core.Line>? lines;
      if (_own) {
        try {
          lines = await own.readImage(path: f.path);
        } catch (e) {
          // No model, no network, no memory: say so once and let the
          // phone's reader take the rest of the run.
          debugPrint('gyotaku: own reader failed: $e');
          trouble = '$e';
          _own = false;
        }
      }
      final w = f.width, h = f.height;
      lines ??= await _withPhone(f, recognizer);
      await core.insertShot(
        path: f.path,
        mtime: f.mtime,
        width: w,
        height: h,
        lines: lines,
      );
      // The own readers take seconds a screenshot, so each one is shown as
      // it lands rather than a handful at a time.
      if (_own || ++_sinceRefresh >= _refreshEvery) {
        _sinceRefresh = 0;
        generation++;
      }
    } catch (e) {
      // One unreadable image must not stop the rest. It is tried again on
      // the next launch.
      debugPrint('gyotaku: could not read ${f.path}: $e');
    }
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

  /// After the index was emptied: reads the library again from the top.
  Future<void> startOver() async {
    generation++;
    state = ReaderState.idle;
    notifyListeners();
    await run();
  }

  void _set(ReaderState s) {
    state = s;
    notifyListeners();
  }
}

/// A screenshot the index hasn't read, or has read an older version of.
class _Fresh {
  _Fresh(this.path, this.mtime, this.width, this.height);

  final String path;
  final int mtime;
  final int width;
  final int height;

  static Future<_Fresh?> of(AssetEntity asset) async {
    try {
      if (asset.width <= 0 || asset.height <= 0) return null;
      final file = await asset.originFile;
      if (file == null) return null;
      final mtime = asset.modifiedDateSecond ?? asset.createDateSecond ?? 0;
      if (await core.isCurrent(path: file.path, mtime: mtime)) return null;
      return _Fresh(file.path, mtime, asset.width, asset.height);
    } catch (e) {
      debugPrint('gyotaku: could not look at ${asset.title}: $e');
      return null;
    }
  }
}

/// The albums that hold screenshots. Android names the folder and iOS the
/// smart album "Screenshots"; anything with the word in its name counts, so
/// a third party tool's own folder is picked up too.
Future<List<AssetPathEntity>> screenshotFolders() async {
  final all = await PhotoManager.getAssetPathList(type: RequestType.image);
  return [
    for (final f in all)
      if (!f.isAll && f.name.toLowerCase().contains('screenshot')) f,
  ];
}
