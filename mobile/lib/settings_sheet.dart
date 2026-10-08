import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:photo_manager/photo_manager.dart';

import 'about_page.dart';
import 'activity.dart';
import 'folders.dart';
import 'reader.dart';
import 'src/rust/api/index.dart' as core;
import 'src/rust/api/reader.dart' as own;
import 'theme.dart';

/// What each script's row says, by its name in the config. A script this
/// version of the app has no words for still gets a row, under its name.
const _words = {
  'devanagari': (
    'Read Devanagari',
    'Hindi, Marathi, Nepali and more, an 8 MB download',
  ),
  'bengali': ('Read Bengali', 'Bangla and Assamese, a 54 MB download'),
};

Future<void> showSettings(BuildContext context, Reader reader) {
  return showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    useSafeArea: true,
    builder: (_) => _Settings(reader: reader),
  );
}

class _Settings extends StatefulWidget {
  const _Settings({required this.reader});

  final Reader reader;

  @override
  State<_Settings> createState() => _SettingsState();
}

class _Folder {
  _Folder(this.album, this.count);
  final AssetPathEntity album;
  final int count;
}

class _SettingsState extends State<_Settings> {
  List<own.ScriptChoice> _scripts = const [];
  List<_Folder>? _folders;
  FolderChoices? _choices;

  @override
  void initState() {
    super.initState();
    _loadScripts();
    _loadFolders();
  }

  Future<void> _loadScripts() async {
    try {
      final scripts = await own.scripts();
      if (mounted) setState(() => _scripts = scripts);
    } catch (e) {
      debugPrint('gyotaku: could not load the settings: $e');
    }
  }

  Future<void> _loadFolders() async {
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
      // The ones that are on first, then the biggest.
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

  Future<void> _setFolder(_Folder folder, bool on) async {
    HapticFeedback.selectionClick();
    await _choices!.set(folder.album, on);
    if (mounted) setState(() {});
    // Turning one on reads it; turning one off takes its images out of
    // search. Either way the reader goes through the folders again.
    widget.reader.run();
  }

  Future<void> _setScript(own.ScriptChoice script, bool on) async {
    HapticFeedback.selectionClick();
    try {
      await own.setScript(name: script.name, enabled: on);
    } catch (e) {
      if (mounted) say(context, 'Could not save that setting');
    }
    await _loadScripts();
    // Fetch its model now, while the person who asked is looking.
    if (on) widget.reader.prepare();
  }

  Future<void> _readAgain() async {
    try {
      await core.forgetAll();
    } catch (e) {
      if (mounted) say(context, 'Could not start over');
      return;
    }
    if (!mounted) return;
    Navigator.of(context).pop();
    widget.reader.startOver();
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final anyOn = _scripts.any((s) => s.enabled);
    final folders = _folders;

    Widget heading(String text) => Padding(
      padding: const EdgeInsets.fromLTRB(20, 18, 20, 4),
      child: Text(
        text,
        style: TextStyle(
          fontSize: 16,
          fontWeight: FontWeight.w600,
          color: p.text,
        ),
      ),
    );
    Widget note(String text) => Padding(
      padding: const EdgeInsets.fromLTRB(20, 0, 20, 6),
      child: Text(
        text,
        style: TextStyle(fontSize: 13.5, height: 1.4, color: p.muted),
      ),
    );

    return DraggableScrollableSheet(
      expand: false,
      initialChildSize: 0.75,
      minChildSize: 0.4,
      maxChildSize: 0.94,
      builder: (context, scroll) => ListView(
        controller: scroll,
        padding: EdgeInsets.only(
          bottom: 24 + MediaQuery.paddingOf(context).bottom,
        ),
        children: [
          Center(
            child: Container(
              width: 36,
              height: 4,
              margin: const EdgeInsets.only(top: 10, bottom: 6),
              decoration: BoxDecoration(
                color: p.track,
                borderRadius: BorderRadius.circular(2),
              ),
            ),
          ),
          // The same notice as on the page behind, which this sheet covers.
          Padding(
            padding: const EdgeInsets.only(top: 8),
            child: Activity(reader: widget.reader),
          ),
          heading('Folders'),
          note(
            'gyotaku reads the images in the folders that are on. Screenshot '
            'folders are on to begin with. Turning a folder off only takes '
            'its images out of search; the images stay where they are.',
          ),
          if (folders == null)
            note('Looking for folders')
          else if (folders.isEmpty)
            note('No folders can be seen. Allow access to photos first.')
          else
            for (final f in folders)
              _Row(
                title: f.album.name.isEmpty ? 'Unnamed' : f.album.name,
                detail: f.count == 1 ? '1 image' : '${f.count} images',
                on: _choices!.isOn(f.album),
                onChanged: (v) => _setFolder(f, v),
              ),
          heading('Other scripts'),
          note(
            'The phone reads Latin text on its own. For these, gyotaku uses '
            'its own readers, which are slower and download once: 23 MB the '
            'first time one is turned on, plus the size shown.',
          ),
          for (final s in _scripts)
            _Row(
              title: _words[s.name]?.$1 ?? 'Read ${s.name}',
              detail: _words[s.name]?.$2 ?? '',
              on: s.enabled,
              onChanged: (v) => _setScript(s, v),
            ),
          const SizedBox(height: 6),
          note(
            anyOn
                ? 'This applies to images read from now on. Ones read before '
                      'keep the text they were read with, until everything '
                      'is read again.'
                : 'With none turned on, nothing is downloaded.',
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(20, 10, 20, 0),
            child: TextButton(
              onPressed: _readAgain,
              style: TextButton.styleFrom(
                foregroundColor: p.text,
                backgroundColor: p.tile,
                minimumSize: const Size(0, 48),
                shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(14),
                ),
                textStyle: const TextStyle(
                  fontFamily: 'IBM Plex Sans',
                  fontSize: 14.5,
                  fontWeight: FontWeight.w500,
                ),
              ),
              child: const Text('Read everything again'),
            ),
          ),
          const SizedBox(height: 14),
          InkWell(
            onTap: () {
              final navigator = Navigator.of(context);
              navigator.pop();
              navigator.push(
                MaterialPageRoute<void>(builder: (_) => const AboutPage()),
              );
            },
            child: Padding(
              padding: const EdgeInsets.fromLTRB(20, 12, 16, 12),
              child: Row(
                children: [
                  Expanded(
                    child: Text(
                      'About gyotaku',
                      style: TextStyle(fontSize: 15.5, color: p.text),
                    ),
                  ),
                  Icon(Icons.chevron_right, color: p.faint),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _Row extends StatelessWidget {
  const _Row({
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
