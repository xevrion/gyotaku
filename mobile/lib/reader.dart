import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/services.dart' show PlatformException;
import 'package:flutter/widgets.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart' show Int64List;
import 'package:google_mlkit_text_recognition/google_mlkit_text_recognition.dart';
import 'package:path_provider/path_provider.dart';
import 'package:photo_manager/photo_manager.dart';

import 'deep.dart';
import 'folders.dart';
import 'notice.dart';
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
/// Reading is in two passes. The first uses the phone's own text
/// recognition, a fraction of a second an image, so a whole library is
/// searchable in minutes. The second only happens while an extra script is
/// on: gyotaku's own readers go back over each image for the Bangla or
/// Devanagari the phone's reader could make nothing of. That one takes
/// seconds an image, and is why it comes second.
///
/// It only ever reads. Nothing here writes to, moves or deletes a photo.
///
/// Everything it is doing is in its fields, so the app can always say what
/// is going on and how far along it is, and the same is put in a
/// notification so the reading can go on with the window put away.
class Reader extends ChangeNotifier {
  ReaderState state = ReaderState.idle;

  /// While checking: images looked at, of how many.
  int checked = 0;
  int toCheck = 0;

  /// While preparing: the model coming down, or null while they load.
  String? downloading;
  int downloadedKb = 0;
  int downloadKb = 0;

  /// While reading: of the images this pass has to get through.
  int done = 0;
  int total = 0;

  /// The file being read right now.
  String? current;

  /// Whether the reading under way is the second pass, and for which
  /// scripts, worded for a person: "Bangla and Devanagari".
  bool deep = false;
  String scripts = '';

  /// How many images the second pass still has to go over, for saying what
  /// comes next while the first is at work. Null when that isn't counted
  /// yet; with no script on there is no second pass and `scripts` is empty.
  int? owed;

  /// While reading: how long the rest looks like taking, once there is
  /// enough behind to judge by.
  Duration? left;

  /// Why gyotaku's own readers couldn't be used, if they couldn't: a model
  /// that wouldn't download, most likely. Everything is still searchable by
  /// what the phone's reader made of it.
  String? trouble;

  /// Images that could not be read this run, and what the last one said.
  /// Without this a run where every image fails looks like an empty phone.
  int failed = 0;
  String? failure;

  /// How the last run that read anything went, in a line.
  String? report;

  /// Goes up each time the index changed enough for the grid to be worth
  /// refreshing, so the page doesn't re-query on every single image.
  int generation = 0;

  /// Whether reading has been put on hold. It stays so, across restarts,
  /// until `resume`: someone who paused a three hour read to save their
  /// battery does not want it starting again behind their back.
  bool paused = false;

  /// While paused: how many images are still waiting.
  int waiting = 0;

  Reader() {
    _notice.onPause = pause;
  }

  final _notice = ScanNotice();
  Future<void>? _remembered;
  bool _running = false;
  bool _again = false;

  bool get busy =>
      state == ReaderState.checking ||
      state == ReaderState.preparing ||
      state == ReaderState.reading;

  /// Whether the app's window is in front of the person right now.
  static bool get _visible {
    final s = WidgetsBinding.instance.lifecycleState;
    return s == null || s == AppLifecycleState.resumed;
  }

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
      await _notice.finish();
    } finally {
      _running = false;
    }
    if (_again) {
      _again = false;
      await run();
    }
  }

  /// Puts reading on hold. The image under way is finished first, which for
  /// the second pass can be a few seconds.
  Future<void> pause() async {
    if (paused) return;
    paused = true;
    notifyListeners();
    await _Hold.set(true);
  }

  /// Carries on from where `pause` left it.
  Future<void> resume() async {
    paused = false;
    waiting = 0;
    notifyListeners();
    await _Hold.set(false);
    await run();
  }

  /// After the index was emptied: reads the library again from the top.
  Future<void> startOver() async {
    await DeepReads.clear();
    generation++;
    notifyListeners();
    await run();
  }

  Future<void> _run(bool ask, bool quiet) async {
    // Once: whether reading was left paused the last time the app ran.
    await (_remembered ??= _Hold.get().then((held) {
      if (held) paused = true;
    }));
    var permission = await PhotoManager.getPermissionState(
      requestOption: photoAccess,
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
      await PhotoManager.requestPermissionExtend(requestOption: photoAccess);
      // Asked again rather than trusting what the request came back with.
      // On Android 9 and earlier the plugin asks for leave to write as well,
      // which this app never declares or wants; that half is refused, and
      // the plugin reports the whole request as refused even though reading
      // was just allowed.
      permission = await PhotoManager.getPermissionState(
        requestOption: photoAccess,
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
    final loud = !quiet || state != ReaderState.done;
    if (loud) _set(ReaderState.checking);

    final all = <_Image>[];
    final fresh = <_Image>[];
    for (var n = 0; n < folders.length; n++) {
      for (var start = 0; start < counts[n]; start += _page) {
        final assets = await folders[n].getAssetListRange(
          start: start,
          end: (start + _page).clamp(0, counts[n]),
        );
        // A page at a time: the paths asked for all at once, then one trip
        // to the index for the lot. One image at a time, a library of a few
        // thousand took most of a minute just to be looked at.
        final seen = (await Future.wait(assets.map(_Image.of))).nonNulls
            .toList();
        final current = await core.areCurrent(
          paths: [for (final s in seen) s.path],
          mtimes: Int64List.fromList([for (final s in seen) s.mtime]),
        );
        for (var i = 0; i < seen.length; i++) {
          all.add(seen[i]);
          if (!current[i]) fresh.add(seen[i]);
        }
        checked += assets.length;
        if (loud) _tick(toCheck > _worthANotice);
      }
    }

    // An icon, a 1x2 test image, a slip of the finger: nothing to read, and
    // the phone's reader refuses anything under 32 pixels outright. Noted in
    // the index with no size, which is how it remembers an image it should
    // neither show nor try again, the same as on the desktop.
    final tiny = fresh.where((i) => i.tiny).toList();
    for (final i in tiny) {
      await core.insertShot(
        path: i.path,
        mtime: i.mtime,
        width: 0,
        height: 0,
        lines: const [],
      );
    }
    fresh.removeWhere((i) => i.tiny);

    // What's left in the index but no longer in a folder that's on: deleted
    // from the phone, or its folder was turned off.
    if (await core.keepOnly(paths: [for (final i in all) i.path]) > 0) {
      generation++;
    }

    // The scripts that are on decide whether there is a second pass, and
    // which images still owe one.
    var on = <String>[];
    try {
      on = [
        for (final s in await own.scripts())
          if (s.enabled) s.name,
      ]..sort();
    } catch (e) {
      debugPrint('gyotaku: could not read the scripts: $e');
    }
    final deepReads = on.isEmpty ? null : await DeepReads.load(on);
    scripts = _named(on);
    final owed = deepReads == null
        ? <_Image>[]
        : [
            for (final i in all)
              if (!i.tiny && !deepReads.has(i.path, i.mtime)) i,
          ];

    this.owed = owed.length;
    if (fresh.isEmpty && owed.isEmpty) {
      _set(folders.isEmpty ? ReaderState.noFolder : ReaderState.done);
      await _notice.finish();
      return;
    }

    // Left on hold: say what is waiting and leave it there.
    if (paused) {
      waiting = fresh.length + owed.length;
      _set(ReaderState.done);
      await _notice.finish();
      return;
    }

    // From here on it can take a while, so it goes on with the window put
    // away, behind a notification.
    await _notice.ask();
    _finished = 0;
    failed = 0;
    failure = null;
    report = null;
    final said = <String>[];

    // Newest first across every folder, not one folder after another.
    int newest(_Image a, _Image b) => b.mtime.compareTo(a.mtime);
    fresh.sort(newest);
    owed.sort(newest);

    // A second pass is coming: fetch its readers while the first is at work.
    if (deepReads != null && owed.isNotEmpty) unawaited(fetchReaders());

    final recognizer = TextRecognizer(script: TextRecognitionScript.latin);
    try {
      if (fresh.isNotEmpty) {
        final took = await _pass(fresh, (image) async {
          final lines = await _withPhone(image, recognizer);
          await _put(image, [for (final l in lines) l.line]);
          if (++_sinceRefresh >= _refreshEvery) {
            _sinceRefresh = 0;
            generation++;
          }
        });
        generation++;
        final read = done - failed;
        said.add(
          'Read $read ${read == 1 ? 'image' : 'images'} in ${_took(took)}',
        );
        debugPrint(
          'gyotaku: first pass, ${fresh.length} images, '
          '${took.inMilliseconds} ms',
        );
      }

      if (deepReads != null && owed.isNotEmpty && !paused) {
        // Usually over already: the download began when the script was
        // turned on. If not, this is the stage that shows it.
        _set(ReaderState.preparing);
        _tick(true, now: true);
        await fetchReaders();
        if (trouble == null) {
          deep = true;
          final before = failed;
          final took = await _pass(owed, (image) async {
            own.bePolite(polite: _visible);
            // The phone's reader again, for which lines it is sure of: a
            // few tenths of a second against the seconds it saves.
            final phone = await _withPhone(image, recognizer);
            final sure = [
              for (final l in phone)
                if (l.sure) l.line,
            ];
            final rest = await own.readRest(
              path: image.path,
              known: [for (final l in sure) l.rect],
            );
            await _put(image, [...sure, ...rest]);
            await deepReads.add(image.path, image.mtime);
            // Seconds apart, so each one is shown as it lands.
            generation++;
          });
          deep = false;
          // A few hundred megabytes of models, not needed again until the
          // next image turns up.
          try {
            await own.releaseReaders();
          } catch (e) {
            debugPrint('gyotaku: could not let go of the readers: $e');
          }
          final read = done - (failed - before);
          said.add(
            'added $scripts to $read ${read == 1 ? 'image' : 'images'} '
            'in ${_took(took)}',
          );
          debugPrint(
            'gyotaku: second pass ($on), ${owed.length} images, '
            '${took.inMilliseconds} ms',
          );
        }
      }
    } finally {
      deep = false;
      current = null;
      left = null;
      await recognizer.close();
    }

    if (failed > 0) said.add('$failed could not be read');
    if (paused) {
      // What both passes together still had to get through.
      waiting = (fresh.length + owed.length - _finished).clamp(0, 1 << 31);
    }
    if (said.isNotEmpty) {
      final line = said.join(', ');
      report = line[0].toUpperCase() + line.substring(1);
    }
    debugPrint('gyotaku: $report');
    _set(ReaderState.done);
    // Someone watching has just seen it finish. Someone who put the app
    // away gets told.
    if (paused && !_visible) {
      await _notice.finish(
        title: 'Reading paused',
        text: 'Open gyotaku to carry on.',
      );
    } else if (_visible || report == null) {
      await _notice.finish();
    } else {
      await _notice.finish(title: 'gyotaku finished reading', text: report);
    }
  }

  /// Takes `images` one at a time through `read`, keeping the count, the
  /// time left and the notification up to date. One at a time on purpose:
  /// several of the phone's readers at once were no faster on an eight core
  /// Pixel 6 and lost most of the lines.
  Future<Duration> _pass(
    List<_Image> images,
    Future<void> Function(_Image) read,
  ) async {
    done = 0;
    total = images.length;
    left = null;
    _set(ReaderState.reading);
    _tick(true, now: true);
    final clock = Stopwatch()..start();
    for (final image in images) {
      if (paused) break;
      current = image.path.split('/').last;
      try {
        await read(image);
      } catch (e) {
        // One unreadable image must not stop the rest. It is tried again
        // on the next look.
        debugPrint('gyotaku: could not read ${image.path}: $e');
        failed++;
        failure = _plain(e);
      }
      done++;
      _finished++;
      // Not worth a guess until there is something to go on.
      if (done >= 3 && clock.elapsed.inSeconds >= 5) {
        left = clock.elapsed * ((total - done) / done);
      }
      _tick(true);
    }
    return clock.elapsed;
  }

  /// Downloads and loads gyotaku's own readers, saying how far each
  /// download is. Started the moment a script is turned on, alongside
  /// whatever reading is under way, so the wait for it is over by the time
  /// it is needed and is never a surprise at the end. Asking again while it
  /// is at it joins the one already going.
  Future<void> fetchReaders() =>
      _fetch ??= _fetchReaders().whenComplete(() => _fetch = null);

  Future<void>? _fetch;

  /// Whether the readers are being downloaded or loaded right now.
  bool get fetching => _fetch != null;

  Future<void> _fetchReaders() async {
    trouble = null;
    downloading = null;
    notifyListeners();
    final poll = Timer.periodic(const Duration(milliseconds: 150), (_) {
      final d = own.downloadProgress();
      final file = d?.file;
      if (file != downloading ||
          (d?.doneKb ?? 0) != downloadedKb ||
          (d?.totalKb ?? 0) != downloadKb) {
        final stage = file != downloading;
        downloading = file;
        downloadedKb = d?.doneKb ?? 0;
        downloadKb = d?.totalKb ?? 0;
        _tick(state == ReaderState.preparing, now: stage);
      }
    });
    try {
      await own.loadReaders();
    } catch (e) {
      // No network, no space, a file that changed: say so. The first pass
      // makes everything searchable regardless.
      debugPrint('gyotaku: own readers failed: $e');
      trouble = '$e';
    } finally {
      poll.cancel();
      downloading = null;
      notifyListeners();
    }
  }

  /// A script was just turned on or off in settings. What follows from
  /// that is put in view straight away: its reader starts downloading, and
  /// the second pass is listed as coming, rather than both turning up
  /// unannounced once the reading under way has finished.
  Future<void> scriptsChanged() async {
    var on = <String>[];
    try {
      on = [
        for (final s in await own.scripts())
          if (s.enabled) s.name,
      ]..sort();
    } catch (e) {
      debugPrint('gyotaku: could not read the scripts: $e');
    }
    scripts = _named(on);
    owed = null;
    notifyListeners();
    if (on.isNotEmpty) unawaited(fetchReaders());
    await run();
  }

  // How many images to ask the library about in one go.
  static const _page = 200;

  // A look through this many images is long enough to be worth a notice.
  static const _worthANotice = 1500;

  // The phone's reader gives real text 0.66 and up and the junk it makes of
  // Bangla 0.47 and under (measured on two Bangla screenshots). A line it
  // is this sure of is taken as read; anything less goes to the own readers.
  static const _sure = 0.8;

  static const _refreshEvery = 6;
  int _sinceRefresh = 0;

  // Images got through this run, over both passes.
  int _finished = 0;

  Future<void> _put(_Image image, List<core.Line> lines) => core.insertShot(
    path: image.path,
    mtime: image.mtime,
    width: image.width,
    height: image.height,
    lines: lines,
  );

  Future<List<_PhoneLine>> _withPhone(
    _Image f,
    TextRecognizer recognizer,
  ) async {
    final text = await recognizer.processImage(InputImage.fromFilePath(f.path));
    final w = f.width, h = f.height;
    final lines = <_PhoneLine>[];
    for (final block in text.blocks) {
      for (final line in block.lines) {
        final b = line.boundingBox;
        final confidence = line.confidence;
        lines.add(
          _PhoneLine(
            core.Line(
              text: line.text,
              rect: core.Rect(
                x: (b.left / w).clamp(0.0, 1.0),
                y: (b.top / h).clamp(0.0, 1.0),
                w: (b.width / w).clamp(0.0, 1.0),
                h: (b.height / h).clamp(0.0, 1.0),
              ),
              score: confidence ?? 1.0,
            ),
            // No confidence given means no telling, so not sure.
            confidence != null && confidence >= _sure,
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

  /// Something moved: redraw, and with `notice` say the same in the
  /// notification.
  void _tick(bool notice, {bool now = false}) {
    notifyListeners();
    if (!notice) return;
    final (title, text, at, of) = switch (state) {
      ReaderState.checking => (
        'Looking through your folders',
        '$checked of $toCheck images checked',
        checked,
        toCheck,
      ),
      ReaderState.preparing when downloading != null => (
        'Downloading ${modelName(downloading!)}',
        downloadKb == 0
            ? '${_mb(downloadedKb)} MB so far'
            : '${_mb(downloadedKb)} of ${_mb(downloadKb)} MB',
        downloadedKb,
        downloadKb,
      ),
      ReaderState.preparing => (
        "Getting gyotaku's own readers ready",
        '',
        0,
        0,
      ),
      ReaderState.reading => (
        deep ? 'Adding $scripts' : 'Reading screenshots',
        deep || next == null ? progressLine : '$progressLine. $next',
        done,
        total,
      ),
      _ => (null, '', 0, 0),
    };
    if (title != null) {
      unawaited(_notice.show(title, text, done: at, total: of, now: now));
    }
  }

  /// "14 of 120, about 18 min left", for the banner and the notification.
  String get progressLine => [
    '$done of $total',
    if (left != null) 'about ${_took(left!)} left',
    if (failed > 0) '$failed could not be read',
  ].join(', ');

  /// What went wrong, without the wrapping a plugin puts around it.
  static String _plain(Object e) {
    if (e is PlatformException) {
      final said = e.message ?? e.code;
      return said.split(': ').last.split('\n').first;
    }
    return '$e'.split('\n').first;
  }

  /// What follows the reading under way, in a sentence, or null when
  /// nothing does. So that a second pass, hours long, is expected.
  String? get next {
    if (state != ReaderState.reading || deep || scripts.isEmpty) return null;
    final n = owed;
    if (n == 0) return null;
    return n == null
        ? 'Next: adding $scripts to every image'
        : 'Next: adding $scripts to $n ${n == 1 ? 'image' : 'images'}';
  }

  static String _mb(int kb) => (kb / 1024).toStringAsFixed(1);

  static String _took(Duration d) {
    if (d.inSeconds < 60) return '${d.inSeconds.clamp(1, 59)} s';
    if (d.inMinutes < 60) return '${d.inMinutes} min ${d.inSeconds % 60} s';
    return '${d.inHours} h ${d.inMinutes % 60} min';
  }

  /// "Bangla and Devanagari", from the names in the config.
  static String _named(List<String> scripts) {
    final names = [
      for (final s in scripts)
        switch (s) {
          'bengali' => 'Bangla',
          'devanagari' => 'Devanagari',
          _ => s,
        },
    ];
    if (names.length < 2) return names.join();
    return '${names.sublist(0, names.length - 1).join(', ')} and ${names.last}';
  }
}

/// What a model file is, to someone who never asked for its name.
String modelName(String file) {
  final f = file.toLowerCase();
  if (f.contains('bengali')) return 'the Bengali reader';
  if (f.contains('devanagari')) return 'the Devanagari reader';
  if (f.contains('_det')) return 'the text finder';
  if (f.contains('_rec')) return 'the text reader';
  return file;
}

/// A line as the phone's reader gave it, and whether it vouches for it.
class _PhoneLine {
  _PhoneLine(this.line, this.sure);

  final core.Line line;
  final bool sure;
}

/// An image in a folder that's on, as the library describes it.
class _Image {
  _Image(this.path, this.mtime, this.width, this.height);

  final String path;
  final int mtime;
  final int width;
  final int height;

  /// Too small to hold anything worth reading.
  bool get tiny => width < _smallest || height < _smallest;

  // The phone's reader takes nothing smaller.
  static const _smallest = 32;

  static Future<_Image?> of(AssetEntity asset) async {
    try {
      final file = await asset.originFile;
      if (file == null) return null;
      var (width, height) = (asset.width, asset.height);
      if (width <= 0 || height <= 0) {
        // The library has no size for it. The file's own header does.
        final buffer = await ui.ImmutableBuffer.fromFilePath(file.path);
        final header = await ui.ImageDescriptor.encoded(buffer);
        (width, height) = (header.width, header.height);
        header.dispose();
        buffer.dispose();
        if (width <= 0 || height <= 0) return null;
      }
      return _Image(
        file.path,
        asset.modifiedDateSecond ?? asset.createDateSecond ?? 0,
        width,
        height,
      );
    } catch (e) {
      debugPrint('gyotaku: could not look at ${asset.title}: $e');
      return null;
    }
  }
}

/// Whether reading is paused, kept as a file that is there or isn't.
class _Hold {
  static Future<File?> _file() async {
    try {
      final dir = await getApplicationSupportDirectory();
      return File('${dir.path}/paused');
    } catch (e) {
      return null;
    }
  }

  static Future<bool> get() async {
    try {
      return await (await _file())?.exists() ?? false;
    } catch (e) {
      return false;
    }
  }

  static Future<void> set(bool held) async {
    try {
      final file = await _file();
      if (file == null) return;
      if (held) {
        await file.writeAsString('');
      } else if (await file.exists()) {
        await file.delete();
      }
    } catch (e) {
      debugPrint('gyotaku: could not remember the pause: $e');
    }
  }
}
