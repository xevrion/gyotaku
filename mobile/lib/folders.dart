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

/// What gyotaku asks the phone for, the same everywhere it asks or checks:
/// images, and nothing else.
///
/// The plugin's own default is images and videos. Android 13 has a
/// permission for each, the app only declares the one for images, and a
/// request that includes one it doesn't declare is refused outright: on
/// Android 13 the app could never see a single screenshot.
const photoAccess = PermissionRequestOption(
  androidPermission: AndroidPermission(
    type: RequestType.image,
    mediaLocation: false,
  ),
);

/// Every album of images on the phone, without the "all photos" one that
/// would repeat the rest.
Future<List<AssetPathEntity>> imageFolders() async {
  final all = await PhotoManager.getAssetPathList(
    type: RequestType.image,
    // An order has to be named. With none the plugin still asks for a page
    // at a time, and on Android 10 that comes out as "ORDER BY LIMIT", which
    // the system refuses: every listing failed there.
    filterOption: FilterOptionGroup(
      // Any size: the library doesn't always know one (an image copied
      // onto the phone, say), and leaving those out hid whole folders.
      imageOption: const FilterOption(
        sizeConstraint: SizeConstraint(ignoreSize: true),
      ),
      orders: const [OrderOption(type: OrderOptionType.createDate)],
    ),
  );
  return [
    for (final f in all)
      if (!f.isAll) f,
  ];
}
