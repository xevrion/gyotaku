import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:photo_manager/photo_manager.dart';

import 'ink.dart';
import 'reader.dart';
import 'settings_sheet.dart';
import 'shot_page.dart';
import 'src/rust/api/index.dart' as core;
import 'theme.dart';
import 'when.dart';

/// The quick date filters. Each is one of the core's own `date:` filters,
/// added to the query on the way to the index, so a phone gets them without
/// typing a colon.
enum _When {
  today('Today', 'date:today'),
  yesterday('Yesterday', 'date:yesterday'),
  week('This week', 'date:week'),
  month('This month', 'date:month');

  const _When(this.label, this.filter);
  final String label;
  final String filter;
}

class SearchPage extends StatefulWidget {
  const SearchPage({super.key, required this.reader});

  final Reader reader;

  @override
  State<SearchPage> createState() => _SearchPageState();
}

class _SearchPageState extends State<SearchPage> with WidgetsBindingObserver {
  static const _step = 120;
  static const _columns = 3;

  final _query = TextEditingController();
  final _focus = FocusNode();
  final _scroll = ScrollController();

  List<core.Hit> _hits = const [];
  _When? _when;
  int _limit = _step;
  int _count = 0;
  int _asked = 0;
  int _seen = -1;
  bool _answered = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    widget.reader.addListener(_onReader);
    _scroll.addListener(_onScroll);
    _search();
    widget.reader.run();
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    widget.reader.removeListener(_onReader);
    _query.dispose();
    _focus.dispose();
    _scroll.dispose();
    super.dispose();
  }

  // A screenshot taken while the app was in the background should be there
  // when it comes back, without anyone asking.
  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) widget.reader.run();
  }

  void _onReader() {
    if (!mounted) return;
    if (widget.reader.generation != _seen) {
      _seen = widget.reader.generation;
      _search();
    }
    setState(() {});
  }

  void _onScroll() {
    // The index has no paging, so more is asked for by raising the limit.
    final p = _scroll.position;
    if (_hits.length >= _limit && p.pixels > p.maxScrollExtent - 800) {
      _limit += _step;
      _search();
    }
  }

  bool get _typed => _query.text.trim().isNotEmpty;
  bool get _narrowed => _typed || _when != null;

  void _changed() {
    _limit = _step;
    if (_scroll.hasClients) _scroll.jumpTo(0);
    _search();
    setState(() {});
  }

  Future<void> _search() async {
    // Answers can come back out of order while typing; only the latest counts.
    final asked = ++_asked;
    final text = [_query.text, ?_when?.filter].join(' ');
    try {
      final hits = await core.search(query: text, limit: _limit);
      final count = await core.shotCount();
      if (!mounted || asked != _asked) return;
      setState(() {
        _hits = hits;
        _count = count;
        _answered = true;
      });
    } catch (e) {
      debugPrint('gyotaku: search failed: $e');
    }
  }

  void _open(int index) {
    _focus.unfocus();
    Navigator.of(context).push(
      MaterialPageRoute<void>(
        builder: (_) => ShotPage(hits: _hits, index: index),
      ),
    );
  }

  String _status() {
    if (_narrowed) {
      final more = _hits.length >= _limit ? '+' : '';
      return _hits.length == 1 ? '1 match' : '${_hits.length}$more matches';
    }
    return _count == 1 ? '1 screenshot' : '$_count screenshots';
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final reader = widget.reader;
    final reading = reader.state == ReaderState.reading;
    // With nothing indexed there is nothing to filter or count, and the
    // page below has the one thing to say.
    final bare = _answered && _count == 0 && !_narrowed;

    return AnnotatedRegion<SystemUiOverlayStyle>(
      value: Theme.of(context).appBarTheme.systemOverlayStyle!,
      child: Scaffold(
        body: SafeArea(
          bottom: false,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 12, 6, 0),
                child: Row(
                  children: [
                    Expanded(
                      child: _SearchField(
                        controller: _query,
                        focus: _focus,
                        onChanged: _changed,
                      ),
                    ),
                    IconButton(
                      onPressed: () {
                        _focus.unfocus();
                        showSettings(context, widget.reader);
                      },
                      icon: const Icon(Icons.tune),
                      color: p.muted,
                      tooltip: 'Settings',
                    ),
                  ],
                ),
              ),
              if (!bare)
                SizedBox(
                  height: 52,
                  child: ListView.separated(
                    scrollDirection: Axis.horizontal,
                    padding: const EdgeInsets.symmetric(
                      horizontal: 16,
                      vertical: 10,
                    ),
                    itemCount: _When.values.length,
                    separatorBuilder: (_, _) => const SizedBox(width: 8),
                    itemBuilder: (context, i) {
                      final w = _When.values[i];
                      return _Chip(
                        label: w.label,
                        selected: _when == w,
                        onTap: () {
                          HapticFeedback.selectionClick();
                          _when = _when == w ? null : w;
                          _changed();
                        },
                      );
                    },
                  ),
                ),
              Padding(
                padding: const EdgeInsets.fromLTRB(18, 2, 18, 8),
                child: bare
                    ? const SizedBox(height: 8)
                    : Row(
                        children: [
                          Text(
                            _status(),
                            style: TextStyle(fontSize: 13, color: p.muted),
                          ),
                          const Spacer(),
                          if (reading)
                            Text(
                              'Reading ${reader.done} of ${reader.total}',
                              style: TextStyle(fontSize: 13, color: p.muted),
                            ),
                        ],
                      ),
              ),
              if (reader.trouble != null)
                Padding(
                  padding: const EdgeInsets.fromLTRB(18, 0, 18, 8),
                  child: Text(
                    "gyotaku's own readers could not be used, so the phone's "
                    'read these instead: ${reader.trouble}',
                    maxLines: 3,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(fontSize: 12.5, color: p.muted),
                  ),
                ),
              SizedBox(
                height: 2,
                child: reading
                    ? LinearProgressIndicator(
                        value: reader.total == 0
                            ? null
                            : reader.done / reader.total,
                        minHeight: 2,
                      )
                    : null,
              ),
              Expanded(child: _body(context)),
            ],
          ),
        ),
      ),
    );
  }

  Widget _body(BuildContext context) {
    final reader = widget.reader;
    if (!_answered) return const SizedBox();

    if (_hits.isEmpty) {
      if (_narrowed) {
        return _Notice(
          icon: Icons.search_off,
          title: 'Nothing matches',
          body: _typed
              ? 'Every word has to appear in the screenshot. Try fewer words, or a few letters of one.'
              : 'No screenshots from ${_when!.label.toLowerCase()}.',
          action: 'Clear search',
          onAction: () {
            _query.clear();
            _when = null;
            _changed();
          },
        );
      }
      return switch (reader.state) {
        ReaderState.unasked => _Notice(
          icon: Icons.photo_library_outlined,
          title: 'Search your screenshots by the text in them',
          body: 'gyotaku reads the words in every screenshot on this phone so you can find one by anything you remember seeing. It only reads them, and nothing leaves this device.',
          action: 'Allow access to photos',
          onAction: () => reader.run(ask: true),
        ),
        ReaderState.refused => _Notice(
          icon: Icons.lock_outline,
          title: 'gyotaku cannot see your screenshots',
          body: 'Access to photos was not allowed. It can be turned on in the system settings.',
          action: 'Open settings',
          onAction: PhotoManager.openSetting,
        ),
        ReaderState.noFolder => _Notice(
          icon: Icons.image_not_supported_outlined,
          title: 'No screenshots yet',
          body: 'There is no screenshots album on this phone. Take a screenshot and it will appear here.',
          action: 'Look again',
          onAction: reader.run,
        ),
        ReaderState.checking || ReaderState.reading => const _Notice(
          icon: Icons.hourglass_empty,
          title: 'Reading your screenshots',
          body: 'The newest come first. You can search as soon as they appear.',
        ),
        _ => _Notice(
          icon: Icons.image_not_supported_outlined,
          title: 'No screenshots yet',
          body: 'Take a screenshot and it will appear here.',
          action: 'Look again',
          onAction: reader.run,
        ),
      };
    }

    final width = MediaQuery.sizeOf(context).width;
    final decode = (width / _columns * MediaQuery.devicePixelRatioOf(context))
        .round();
    final p = Palette.of(context);

    return RefreshIndicator(
      onRefresh: reader.run,
      color: p.text,
      backgroundColor: p.tile,
      elevation: 0,
      child: CustomScrollView(
        controller: _scroll,
        physics: const AlwaysScrollableScrollPhysics(),
        keyboardDismissBehavior: ScrollViewKeyboardDismissBehavior.onDrag,
        slivers: [
          for (final s in _sections()) ...[
            if (s.title != null)
              SliverToBoxAdapter(
                child: Padding(
                  padding: const EdgeInsets.fromLTRB(18, 14, 18, 8),
                  child: Text(
                    s.title!,
                    style: TextStyle(
                      fontSize: 13,
                      fontWeight: FontWeight.w600,
                      color: p.text,
                    ),
                  ),
                ),
              ),
            SliverPadding(
              padding: const EdgeInsets.symmetric(horizontal: 12),
              sliver: SliverGrid.builder(
                gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
                  crossAxisCount: _columns,
                  mainAxisSpacing: 8,
                  crossAxisSpacing: 8,
                  // The tallest a tile gets, see `tile_aspect`.
                  childAspectRatio: 0.5,
                ),
                itemCount: s.count,
                itemBuilder: (context, i) => _Tile(
                  hit: _hits[s.start + i],
                  decodeWidth: decode,
                  onTap: () => _open(s.start + i),
                ),
              ),
            ),
          ],
          SliverToBoxAdapter(
            child: SizedBox(height: 24 + MediaQuery.paddingOf(context).bottom),
          ),
        ],
      ),
    );
  }

  /// Runs of hits under one heading. Browsing, that's the day or month they
  /// were taken. Searching, the order is by how well they match, so the only
  /// heading is the one that sets the near matches apart.
  List<_Section> _sections() {
    final out = <_Section>[];
    final now = DateTime.now();
    String? last;
    for (var i = 0; i < _hits.length; i++) {
      final h = _hits[i];
      final title = _typed
          ? (h.near ? 'Near matches' : null)
          : sectionOf(h.mtime, now);
      if (i == 0 || title != last) {
        out.add(_Section(title, i));
        last = title;
      }
      out.last.count++;
    }
    return out;
  }
}

class _Section {
  _Section(this.title, this.start);
  final String? title;
  final int start;
  int count = 0;
}

class _SearchField extends StatelessWidget {
  const _SearchField({
    required this.controller,
    required this.focus,
    required this.onChanged,
  });

  final TextEditingController controller;
  final FocusNode focus;
  final VoidCallback onChanged;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return Container(
      height: 50,
      decoration: BoxDecoration(
        color: p.tile,
        borderRadius: BorderRadius.circular(16),
      ),
      child: Row(
        children: [
          const SizedBox(width: 14),
          Icon(Icons.search, size: 22, color: p.muted),
          const SizedBox(width: 10),
          Expanded(
            child: TextField(
              controller: controller,
              focusNode: focus,
              onChanged: (_) => onChanged(),
              onSubmitted: (_) => focus.unfocus(),
              autocorrect: false,
              enableSuggestions: false,
              textInputAction: TextInputAction.search,
              style: TextStyle(fontSize: 17, color: p.text),
              decoration: InputDecoration(
                hintText: 'Search your screenshots',
                hintStyle: TextStyle(color: p.muted),
                border: InputBorder.none,
                isCollapsed: true,
              ),
            ),
          ),
          if (controller.text.isNotEmpty)
            IconButton(
              onPressed: () {
                controller.clear();
                onChanged();
              },
              icon: const Icon(Icons.close, size: 20),
              color: p.muted,
              tooltip: 'Clear',
            )
          else
            const SizedBox(width: 14),
        ],
      ),
    );
  }
}

class _Chip extends StatelessWidget {
  const _Chip({
    required this.label,
    required this.selected,
    required this.onTap,
  });

  final String label;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return Semantics(
      button: true,
      selected: selected,
      child: GestureDetector(
        onTap: onTap,
        behavior: HitTestBehavior.opaque,
        child: Container(
          alignment: Alignment.center,
          padding: const EdgeInsets.symmetric(horizontal: 14),
          decoration: BoxDecoration(
            // Selection is ink.
            color: selected ? p.text : Colors.transparent,
            border: Border.all(color: selected ? p.text : p.track),
            borderRadius: BorderRadius.circular(16),
          ),
          child: Text(
            label,
            style: TextStyle(
              fontSize: 13,
              fontWeight: FontWeight.w500,
              color: selected ? p.panel : p.text,
            ),
          ),
        ),
      ),
    );
  }
}

/// What the page says when there is no grid to show.
class _Notice extends StatelessWidget {
  const _Notice({
    required this.icon,
    required this.title,
    required this.body,
    this.action,
    this.onAction,
  });

  final IconData icon;
  final String title;
  final String body;
  final String? action;
  final VoidCallback? onAction;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return Center(
      child: SingleChildScrollView(
        padding: const EdgeInsets.fromLTRB(36, 0, 36, 96),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Container(
              width: 64,
              height: 64,
              decoration: BoxDecoration(
                color: p.tile,
                borderRadius: BorderRadius.circular(20),
              ),
              child: Icon(icon, size: 28, color: p.muted),
            ),
            const SizedBox(height: 20),
            Text(
              title,
              textAlign: TextAlign.center,
              style: TextStyle(
                fontSize: 19,
                height: 1.25,
                fontWeight: FontWeight.w600,
                color: p.text,
              ),
            ),
            const SizedBox(height: 8),
            Text(
              body,
              textAlign: TextAlign.center,
              style: TextStyle(fontSize: 14.5, height: 1.45, color: p.muted),
            ),
            if (action != null) ...[
              const SizedBox(height: 24),
              InkButton(label: action!, onPressed: onAction!),
            ],
          ],
        ),
      ),
    );
  }
}

class _Tile extends StatefulWidget {
  const _Tile({
    required this.hit,
    required this.decodeWidth,
    required this.onTap,
  });

  final core.Hit hit;
  final int decodeWidth;
  final VoidCallback onTap;

  @override
  State<_Tile> createState() => _TileState();
}

class _TileState extends State<_Tile> {
  bool _down = false;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final hit = widget.hit;
    return GestureDetector(
      onTap: widget.onTap,
      onTapDown: (_) => setState(() => _down = true),
      onTapUp: (_) => setState(() => _down = false),
      onTapCancel: () => setState(() => _down = false),
      child: Align(
        alignment: Alignment.topCenter,
        child: AspectRatio(
          aspectRatio: core.tileAspect(width: hit.width, height: hit.height),
          child: Opacity(
            opacity: _down ? 0.7 : 1,
            child: Container(
              foregroundDecoration: BoxDecoration(
                border: Border.all(color: p.track.withValues(alpha: 0.6)),
                borderRadius: BorderRadius.circular(12),
              ),
              child: ClipRRect(
                borderRadius: BorderRadius.circular(12),
                child: Stack(
                  fit: StackFit.expand,
                  children: [
                    ColoredBox(color: p.tile),
                    InkedShot(
                      path: hit.path,
                      crop: core.tileCrop(width: hit.width, height: hit.height),
                      lit: hit.lines,
                      decodeWidth: widget.decodeWidth,
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
