import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

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

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final scripts = await own.scripts();
      if (mounted) setState(() => _scripts = scripts);
    } catch (e) {
      debugPrint('gyotaku: could not load the settings: $e');
    }
  }

  Future<void> _set(own.ScriptChoice script, bool on) async {
    HapticFeedback.selectionClick();
    try {
      await own.setScript(name: script.name, enabled: on);
    } catch (e) {
      if (mounted) say(context, 'Could not save that setting');
    }
    await _load();
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
    return SingleChildScrollView(
      padding: EdgeInsets.fromLTRB(
        20,
        10,
        20,
        24 + MediaQuery.paddingOf(context).bottom,
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Center(
            child: Container(
              width: 36,
              height: 4,
              decoration: BoxDecoration(
                color: p.track,
                borderRadius: BorderRadius.circular(2),
              ),
            ),
          ),
          const SizedBox(height: 18),
          Text(
            'Other scripts',
            style: TextStyle(
              fontSize: 16,
              fontWeight: FontWeight.w600,
              color: p.text,
            ),
          ),
          const SizedBox(height: 4),
          Text(
            'The phone reads Latin text on its own. For these, gyotaku uses '
            'its own readers, which are slower and download once: 23 MB the '
            'first time one is turned on, plus the size shown.',
            style: TextStyle(fontSize: 13.5, height: 1.4, color: p.muted),
          ),
          const SizedBox(height: 8),
          for (final s in _scripts)
            _Row(
              title: _words[s.name]?.$1 ?? 'Read ${s.name}',
              detail: _words[s.name]?.$2 ?? '',
              on: s.enabled,
              onChanged: (v) => _set(s, v),
            ),
          const SizedBox(height: 12),
          Text(
            anyOn
                ? 'This applies to screenshots read from now on. Ones read '
                      'before keep the text they were read with.'
                : 'With none turned on, nothing is downloaded.',
            style: TextStyle(fontSize: 13.5, height: 1.4, color: p.muted),
          ),
          const SizedBox(height: 16),
          SizedBox(
            width: double.infinity,
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
              child: const Text('Read all screenshots again'),
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
      padding: const EdgeInsets.symmetric(vertical: 8),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(title, style: TextStyle(fontSize: 15.5, color: p.text)),
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
