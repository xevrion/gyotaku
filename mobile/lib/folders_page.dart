import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:photo_manager/photo_manager.dart';

import 'activity.dart';
import 'folders.dart';
import 'reader.dart';
import 'theme.dart';

/// Every folder of images on the phone, each with a switch. A page of its
/// own because a phone can have dozens, and a list that long buried the rest
/// of the settings.
class FoldersPage extends StatefulWidget {
  const FoldersPage({super.key, required this.reader});

  final Reader reader;

  @override
  State<FoldersPage> createState() => _FoldersPageState();
}

class _Folder {
  _Folder(this.album, this.count);
  final AssetPathEntity album;
  final int count;
}

class _FoldersPageState extends State<FoldersPage> {
  List<_Folder>? _folders;
  FolderChoices? _choices;
  bool _changed = false;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final state = await PhotoManager.getPermissionState(
        requestOption: const PermissionRequestOption(),
      );
      if (!state.hasAccess) {
        if (mounted) setState(() => _folders = const []);
        return;
      }
      final choices = await FolderChoices.load();
      final folders = [
        for (final f in await imageFolders())
          _Folder(f, await f.assetCountAsync),
      ];
      // The ones that are on first, then the biggest. Sorted once: a row
      // that jumped away when its switch was touched would be worse.
      folders.sort((a, b) {
        final on =
            (choices.isOn(b.album) ? 1 : 0) - (choices.isOn(a.album) ? 1 : 0);
        return on != 0 ? on : b.count.compareTo(a.count);
      });
      if (!mounted) return;
      setState(() {
        _choices = choices;
        _folders = folders;
      });
    } catch (e) {
      debugPrint('gyotaku: could not list the folders: $e');
      if (mounted) setState(() => _folders = const []);
    }
  }

  Future<void> _set(_Folder folder, bool on) async {
    HapticFeedback.selectionClick();
    await _choices!.set(folder.album, on);
    _changed = true;
    if (mounted) setState(() {});
    // Turning one on reads it; turning one off takes its images out of
    // search. Either way the reader goes through the folders again.
    widget.reader.run();
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final folders = _folders;
    final choices = _choices;
    final on = folders == null || choices == null
        ? 0
        : folders.where((f) => choices.isOn(f.album)).length;

    return AnnotatedRegion<SystemUiOverlayStyle>(
      value: Theme.of(context).appBarTheme.systemOverlayStyle!,
      child: Scaffold(
        body: SafeArea(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Padding(
                padding: const EdgeInsets.fromLTRB(4, 4, 16, 0),
                child: Row(
                  children: [
                    IconButton(
                      onPressed: () => Navigator.of(context).pop(_changed),
                      icon: const Icon(Icons.arrow_back),
                      color: p.text,
                      tooltip: 'Back',
                    ),
                    Expanded(
                      child: Text(
                        'Folders',
                        style: TextStyle(
                          fontSize: 16,
                          fontWeight: FontWeight.w600,
                          color: p.text,
                        ),
                      ),
                    ),
                    if (folders != null && folders.isNotEmpty)
                      Text(
                        '$on of ${folders.length} on',
                        style: TextStyle(fontSize: 13, color: p.muted),
                      ),
                  ],
                ),
              ),
              Padding(
                padding: const EdgeInsets.fromLTRB(20, 8, 20, 12),
                child: Text(
                  'gyotaku reads the images in the folders that are on. '
                  'Screenshot folders are on to begin with. Turning a folder '
                  'off only takes its images out of search; the images stay '
                  'where they are.',
                  style: TextStyle(fontSize: 13.5, height: 1.4, color: p.muted),
                ),
              ),
              Activity(reader: widget.reader),
              Expanded(
                child: folders == null
                    ? const SizedBox()
                    : folders.isEmpty
                    ? Padding(
                        padding: const EdgeInsets.all(20),
                        child: Text(
                          'No folders can be seen. Allow access to photos '
                          'first.',
                          style: TextStyle(fontSize: 14.5, color: p.muted),
                        ),
                      )
                    : ListView.builder(
                        padding: const EdgeInsets.only(bottom: 24),
                        itemCount: folders.length,
                        itemBuilder: (context, i) {
                          final f = folders[i];
                          return ToggleRow(
                            title: f.album.name.isEmpty
                                ? 'Unnamed'
                                : f.album.name,
                            detail: f.count == 1
                                ? '1 image'
                                : '${f.count} images',
                            on: choices!.isOn(f.album),
                            onChanged: (v) => _set(f, v),
                          );
                        },
                      ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// A name, a line under it, and a switch.
class ToggleRow extends StatelessWidget {
  const ToggleRow({
    super.key,
    required this.title,
    required this.detail,
    required this.on,
    required this.onChanged,
  });

  final String title;
  final String detail;
  final bool on;
  final ValueChanged<bool> onChanged;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 6, 14, 6),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  title,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(fontSize: 15.5, color: p.text),
                ),
                if (detail.isNotEmpty)
                  Text(detail, style: TextStyle(fontSize: 13, color: p.muted)),
              ],
            ),
          ),
          Switch(
            value: on,
            onChanged: onChanged,
            // Selection is ink.
            activeThumbColor: p.panel,
            activeTrackColor: p.text,
            inactiveThumbColor: p.muted,
            inactiveTrackColor: p.tile,
            trackOutlineColor: WidgetStatePropertyAll(p.track),
          ),
        ],
      ),
    );
  }
}
