import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:path_provider/path_provider.dart';

/// Which images have had their second, slow read: the one that adds the
/// scripts the phone's own reader can't make out.
///
/// Kept beside the index, as a plain list. Its first line names the scripts
/// it was made with, so turning one on or off starts the list again and
/// every image gets the new read without anyone asking for it.
class DeepReads {
  DeepReads._(this._file, this._done);

  final File? _file;
  final Set<String> _done;

  static String _key(String path, int mtime) => '$mtime\t$path';

  static Future<File?> _where() async {
    try {
      final dir = await getApplicationSupportDirectory();
      return File('${dir.path}/deep-reads.txt');
    } catch (e) {
      return null;
    }
  }

  /// The list for this set of scripts, emptied if it was made with another.
  static Future<DeepReads> load(List<String> scripts) async {
    final made = scripts.join(',');
    final file = await _where();
    try {
      if (file != null && await file.exists()) {
        final lines = await file.readAsLines();
        if (lines.isNotEmpty && lines.first == made) {
          return DeepReads._(file, lines.skip(1).toSet());
        }
      }
      await file?.writeAsString('$made\n', flush: true);
    } catch (e) {
      // Without the list the worst that happens is reading twice.
      debugPrint('gyotaku: could not load the list of second reads: $e');
    }
    return DeepReads._(file, {});
  }

  /// Forgets every second read, for when everything is read again.
  static Future<void> clear() async {
    try {
      final file = await _where();
      if (file != null && await file.exists()) await file.delete();
    } catch (e) {
      debugPrint('gyotaku: could not clear the list of second reads: $e');
    }
  }

  bool has(String path, int mtime) => _done.contains(_key(path, mtime));

  Future<void> add(String path, int mtime) async {
    final key = _key(path, mtime);
    if (!_done.add(key)) return;
    try {
      await _file?.writeAsString('$key\n', mode: FileMode.append);
    } catch (e) {
      debugPrint('gyotaku: could not note a second read: $e');
    }
  }
}
