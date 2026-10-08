import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:path_provider/path_provider.dart';
import 'package:photo_manager/photo_manager.dart';

/// Which albums gyotaku reads. Screenshot folders are on until turned off,
/// everything else is off until turned on, and only the choices that differ
/// from that are kept, so a new screenshots folder is picked up on its own.
class FolderChoices {
  FolderChoices._(this._file, this._chosen);

  final File? _file;
  final Map<String, bool> _chosen;

  static Future<FolderChoices> load() async {
    try {
      final dir = await getApplicationSupportDirectory();
      final file = File('${dir.path}/folders.json');
      if (!await file.exists()) return FolderChoices._(file, {});
      final saved = jsonDecode(await file.readAsString()) as Map;
      return FolderChoices._(file, {
        for (final e in saved.entries)
          if (e.value is bool) '${e.key}': e.value as bool,
      });
    } catch (e) {
      // A broken file means the defaults, not a crash.
      debugPrint('gyotaku: could not load the folder choices: $e');
      return FolderChoices._(null, {});
    }
  }

  /// Android names the folder and iOS the smart album "Screenshots";
  /// anything with the word in its name counts, so a capture tool's own
  /// folder is on by default too.
  static bool screenshots(AssetPathEntity folder) =>
      folder.name.toLowerCase().contains('screenshot');

  bool isOn(AssetPathEntity folder) =>
      _chosen[folder.id] ?? screenshots(folder);

  Future<void> set(AssetPathEntity folder, bool on) async {
    if (on == screenshots(folder)) {
      _chosen.remove(folder.id);
    } else {
      _chosen[folder.id] = on;
    }
    try {
      await _file?.writeAsString(jsonEncode(_chosen));
    } catch (e) {
      debugPrint('gyotaku: could not save the folder choices: $e');
    }
  }
}

/// Every album of images on the phone, without the "all photos" one that
/// would repeat the rest.
Future<List<AssetPathEntity>> imageFolders() async {
  final all = await PhotoManager.getAssetPathList(type: RequestType.image);
  return [
    for (final f in all)
      if (!f.isAll) f,
  ];
}
