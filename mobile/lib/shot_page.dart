import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:share_plus/share_plus.dart';

import 'ink.dart';
import 'src/rust/api/index.dart' as core;
import 'theme.dart';
import 'when.dart';

const _whole = core.Rect(x: 0, y: 0, w: 1, h: 1);

/// One screenshot, whole, with the matching lines lit. Swiping moves through
/// the rest of the results, so a search can be read through without going
/// back to the grid each time.
class ShotPage extends StatefulWidget {
  const ShotPage({super.key, required this.hits, required this.index});

  final List<core.Hit> hits;
  final int index;

  @override
  State<ShotPage> createState() => _ShotPageState();
}

class _ShotPageState extends State<ShotPage> {
  late final _pages = PageController(initialPage: widget.index);
  late int _at = widget.index;

  /// Off shows the screenshot as it was taken, for reading around a match.
  bool _inked = true;

  /// A zoomed screenshot keeps the swipe for itself.
  bool _zoomed = false;

  core.Hit get _hit => widget.hits[_at];

  @override
  void dispose() {
    _pages.dispose();
    super.dispose();
  }

  Future<void> _copyAll() async {
    String said;
    try {
      final lines = await core.shotLines(id: _hit.id);
      if (lines.isEmpty) {
        said = 'No text was found in this screenshot';
      } else {
        await Clipboard.setData(
          ClipboardData(text: lines.map((l) => l.text).join('\n')),
        );
        HapticFeedback.selectionClick();
        said = 'Copied ${lines.length} ${lines.length == 1 ? 'line' : 'lines'}';
      }
    } catch (e) {
      said = 'Could not copy the text';
    }
    if (mounted) say(context, said);
  }

  Future<void> _share() async {
    try {
      await SharePlus.instance.share(ShareParams(files: [XFile(_hit.path)]));
    } catch (e) {
      if (mounted) say(context, 'Could not share this screenshot');
    }
  }

  Future<void> _showText() async {
    final hit = _hit;
    List<core.Line> lines;
    try {
      lines = await core.shotLines(id: hit.id);
    } catch (e) {
      if (mounted) say(context, 'Could not read the text');
      return;
    }
    if (!mounted) return;
    if (lines.isEmpty) {
      say(context, 'No text was found in this screenshot');
      return;
    }
    await showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      builder: (_) => _TextSheet(lines: lines, found: hit.lines.toSet()),
    );
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    final hit = _hit;
    final many = widget.hits.length > 1;

    return AnnotatedRegion<SystemUiOverlayStyle>(
      value: Theme.of(context).appBarTheme.systemOverlayStyle!,
      child: Scaffold(
        body: SafeArea(
          child: Column(
            children: [
              Padding(
                padding: const EdgeInsets.fromLTRB(4, 4, 4, 4),
                child: Row(
                  children: [
                    IconButton(
                      onPressed: () => Navigator.of(context).pop(),
                      icon: const Icon(Icons.arrow_back),
                      color: p.text,
                      tooltip: 'Back',
                    ),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            stamp(hit.mtime),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              fontSize: 15,
                              fontWeight: FontWeight.w600,
                              color: p.text,
                            ),
                          ),
                          Text(
                            [
                              folderOf(hit.path),
                              if (many) '${_at + 1} of ${widget.hits.length}',
                              if (hit.near) 'near match',
                            ].join('  ·  '),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(fontSize: 12.5, color: p.muted),
                          ),
                        ],
                      ),
                    ),
                    if (hit.lines.isNotEmpty)
                      IconButton(
                        onPressed: () => setState(() => _inked = !_inked),
                        icon: Icon(
                          _inked
                              ? Icons.visibility_outlined
                              : Icons.visibility_off_outlined,
                        ),
                        color: p.text,
                        tooltip: _inked
                            ? 'Show the whole screenshot'
                            : 'Show only what matched',
                      ),
                  ],
                ),
              ),
              Expanded(
                child: PageView.builder(
                  controller: _pages,
                  physics: _zoomed
                      ? const NeverScrollableScrollPhysics()
                      : const PageScrollPhysics(),
                  itemCount: widget.hits.length,
                  onPageChanged: (i) => setState(() {
                    _at = i;
                    _zoomed = false;
                  }),
                  itemBuilder: (context, i) => _Shot(
                    key: ValueKey(widget.hits[i].id),
                    hit: widget.hits[i],
                    inked: _inked,
                    onZoom: (z) {
                      if (z != _zoomed) setState(() => _zoomed = z);
                    },
                  ),
                ),
              ),
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 10, 16, 10),
                child: Row(
                  children: [
                    Expanded(
                      child: _Action(
                        icon: Icons.notes,
                        label: 'Text',
                        onTap: _showText,
                      ),
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: _Action(
                        icon: Icons.copy_outlined,
                        label: 'Copy text',
                        onTap: _copyAll,
                      ),
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: _Action(
                        icon: Icons.ios_share,
                        label: 'Share',
                        onTap: _share,
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _Shot extends StatefulWidget {
  const _Shot({
    super.key,
    required this.hit,
    required this.inked,
    required this.onZoom,
  });

  final core.Hit hit;
  final bool inked;
  final ValueChanged<bool> onZoom;

  @override
  State<_Shot> createState() => _ShotState();
}

class _ShotState extends State<_Shot> {
  final _view = TransformationController();
  Offset _tapped = Offset.zero;

  @override
  void initState() {
    super.initState();
    _view.addListener(
      () => widget.onZoom(_view.value.getMaxScaleOnAxis() > 1.01),
    );
  }

  @override
  void dispose() {
    _view.dispose();
    super.dispose();
  }

  // Twice on the same spot zooms into it; twice again goes back.
  void _doubleTap() {
    if (_view.value.getMaxScaleOnAxis() > 1.01) {
      _view.value = Matrix4.identity();
      return;
    }
    const scale = 2.5;
    _view.value = Matrix4.identity()
      ..translateByDouble(
        -_tapped.dx * (scale - 1),
        -_tapped.dy * (scale - 1),
        0,
        1,
      )
      ..scaleByDouble(scale, scale, 1, 1);
  }

  @override
  Widget build(BuildContext context) {
    final hit = widget.hit;
    return GestureDetector(
      onDoubleTapDown: (d) => _tapped = d.localPosition,
      onDoubleTap: _doubleTap,
      child: InteractiveViewer(
        transformationController: _view,
        maxScale: 6,
        child: Center(
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12),
            child: AspectRatio(
              aspectRatio: hit.width / hit.height,
              child: ClipRRect(
                borderRadius: BorderRadius.circular(10),
                child: InkedShot(
                  path: hit.path,
                  crop: _whole,
                  lit: widget.inked ? hit.lines : const [],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _Action extends StatelessWidget {
  const _Action({required this.icon, required this.label, required this.onTap});

  final IconData icon;
  final String label;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return TextButton.icon(
      onPressed: onTap,
      icon: Icon(icon, size: 18),
      label: Text(label, maxLines: 1, overflow: TextOverflow.ellipsis),
      style: TextButton.styleFrom(
        foregroundColor: p.text,
        backgroundColor: p.tile,
        minimumSize: const Size(0, 48),
        padding: const EdgeInsets.symmetric(horizontal: 8),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(14)),
        textStyle: const TextStyle(
          fontFamily: 'IBM Plex Sans',
          fontSize: 14,
          fontWeight: FontWeight.w500,
        ),
      ),
    );
  }
}

/// Every line read from the screenshot, top to bottom. Tapping one copies
/// just that line, which on a phone stands in for dragging a box around it.
class _TextSheet extends StatelessWidget {
  const _TextSheet({required this.lines, required this.found});

  final List<core.Line> lines;
  final Set<core.Line> found;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return DraggableScrollableSheet(
      expand: false,
      initialChildSize: 0.6,
      minChildSize: 0.3,
      maxChildSize: 0.92,
      builder: (context, scroll) => Column(
        children: [
          Container(
            width: 36,
            height: 4,
            margin: const EdgeInsets.only(top: 10),
            decoration: BoxDecoration(
              color: p.track,
              borderRadius: BorderRadius.circular(2),
            ),
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(20, 14, 12, 6),
            child: Row(
              children: [
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'Text in this screenshot',
                        style: TextStyle(
                          fontSize: 16,
                          fontWeight: FontWeight.w600,
                          color: p.text,
                        ),
                      ),
                      Text(
                        'Tap a line to copy it',
                        style: TextStyle(fontSize: 12.5, color: p.muted),
                      ),
                    ],
                  ),
                ),
                TextButton(
                  onPressed: () async {
                    await Clipboard.setData(
                      ClipboardData(text: lines.map((l) => l.text).join('\n')),
                    );
                    HapticFeedback.selectionClick();
                    if (!context.mounted) return;
                    Navigator.of(context).pop();
                    say(context, 'Copied ${lines.length} lines');
                  },
                  style: TextButton.styleFrom(foregroundColor: p.text),
                  child: const Text(
                    'Copy all',
                    style: TextStyle(fontWeight: FontWeight.w600),
                  ),
                ),
              ],
            ),
          ),
          Expanded(
            child: ListView.builder(
              controller: scroll,
              padding: const EdgeInsets.only(bottom: 24),
              itemCount: lines.length,
              itemBuilder: (context, i) {
                final line = lines[i];
                final hit = found.contains(line);
                return InkWell(
                  onTap: () async {
                    await Clipboard.setData(ClipboardData(text: line.text));
                    HapticFeedback.selectionClick();
                    if (!context.mounted) return;
                    Navigator.of(context).pop();
                    say(context, 'Copied the line');
                  },
                  // The same mark as on the screenshot: shu beside what the
                  // search found, and nowhere else.
                  child: Container(
                    margin: const EdgeInsets.fromLTRB(8, 2, 20, 2),
                    padding: const EdgeInsets.fromLTRB(10, 7, 0, 7),
                    decoration: BoxDecoration(
                      border: Border(
                        left: BorderSide(
                          width: 3,
                          color: hit ? p.shu : Colors.transparent,
                        ),
                      ),
                    ),
                    child: Text(
                      line.text,
                      style: TextStyle(
                        fontSize: 15,
                        height: 1.35,
                        color: p.text,
                        fontWeight: hit ? FontWeight.w500 : FontWeight.w400,
                      ),
                    ),
                  ),
                );
              },
            ),
          ),
        ],
      ),
    );
  }
}
