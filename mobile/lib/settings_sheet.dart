import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:photo_manager/photo_manager.dart';

import 'about_page.dart';
import 'activity.dart';
import 'folders.dart';
import 'folders_page.dart';
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

class _SettingsState extends State<_Settings> {
  List<own.ScriptChoice> _scripts = const [];

  /// "Screenshots and 2 more", or what stands in for it.
  String _folders = '';

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
    var said = 'Choose which folders are read';
    try {
      final state = await PhotoManager.getPermissionState(
        requestOption: const PermissionRequestOption(),
      );
      if (state.hasAccess) {
        final choices = await FolderChoices.load();
        final all = await imageFolders();
        final on = [
          for (final f in all)
            if (choices.isOn(f)) f.name,
        ];
        said = switch (on.length) {
          0 => 'None is on, of ${all.length}',
          1 => '${on.first}, of ${all.length}',
          _ => '${on.first} and ${on.length - 1} more, of ${all.length}',
        };
      } else {
        said = 'Allow access to photos first';
      }
    } catch (e) {
      debugPrint('gyotaku: could not count the folders: $e');
    }
    if (mounted) setState(() => _folders = said);
  }

  Future<void> _setScript(own.ScriptChoice script, bool on) async {
    HapticFeedback.selectionClick();
    try {
      await own.setScript(name: script.name, enabled: on);
    } catch (e) {
      if (mounted) say(context, 'Could not save that setting');
    }
    await _loadScripts();
    // Either way there is work: with one more on, its reader to fetch
    // (begun at once, and shown) and every image to go back over; with one
    // off, nothing owed any more.
    widget.reader.scriptsChanged();
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
    Widget link(String title, String detail, VoidCallback onTap) => InkWell(
      onTap: onTap,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(20, 12, 16, 12),
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(title, style: TextStyle(fontSize: 15.5, color: p.text)),
                  if (detail.isNotEmpty)
                    Text(
                      detail,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(fontSize: 13, color: p.muted),
                    ),
                ],
              ),
            ),
            Icon(Icons.chevron_right, color: p.faint),
          ],
        ),
      ),
    );

    return DraggableScrollableSheet(
      expand: false,
      initialChildSize: 0.7,
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
          link('Folders', _folders, () async {
            await Navigator.of(context).push(
              MaterialPageRoute<bool>(
                builder: (_) => FoldersPage(reader: widget.reader),
              ),
            );
            _loadFolders();
          }),
          heading('Other scripts'),
          note(
            'The phone reads Latin text on its own. For these, gyotaku uses '
            'its own readers, which are slower and download once: 23 MB the '
            'first time one is turned on, plus the size shown.',
          ),
          for (final s in _scripts)
            ToggleRow(
              title: _words[s.name]?.$1 ?? 'Read ${s.name}',
              detail: _words[s.name]?.$2 ?? '',
              on: s.enabled,
              onChanged: (v) => _setScript(s, v),
            ),
          const SizedBox(height: 6),
          note(
            anyOn
                ? 'Every image is gone over a second time for these, after '
                      'it has been made searchable. That takes seconds an '
                      'image, so a large library takes hours.'
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
          Padding(
            padding: const EdgeInsets.fromLTRB(20, 8, 20, 6),
            child: Text(
              'Reading carries on when gyotaku is put away, with its progress '
              'in a notification, even after it is swiped closed. It can be '
              'paused there or here, and stays paused until resumed.',
              style: TextStyle(fontSize: 13, height: 1.4, color: p.faint),
            ),
          ),
          link('About gyotaku', '', () {
            final navigator = Navigator.of(context);
            navigator.pop();
            navigator.push(
              MaterialPageRoute<void>(builder: (_) => const AboutPage()),
            );
          }),
        ],
      ),
    );
  }
}
