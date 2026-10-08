import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:url_launcher/url_launcher.dart';

import 'theme.dart';

const _project = 'https://github.com/xevrion/gyotaku';
const _site = 'https://gyotaku.app';
const _androidSource = 'https://github.com/zamansheikh/gyotaku';

/// What gyotaku is, who made it, and where its source lives.
class AboutPage extends StatefulWidget {
  const AboutPage({super.key});

  @override
  State<AboutPage> createState() => _AboutPageState();
}

class _AboutPageState extends State<AboutPage> {
  String? _version;

  @override
  void initState() {
    super.initState();
    PackageInfo.fromPlatform()
        .then((info) {
          if (mounted) setState(() => _version = info.version);
        })
        .catchError((Object e) {
          debugPrint('gyotaku: could not read the version: $e');
        });
  }

  Future<void> _open(String url) async {
    var opened = false;
    try {
      opened = await launchUrl(
        Uri.parse(url),
        mode: LaunchMode.externalApplication,
      );
    } catch (e) {
      debugPrint('gyotaku: could not open $url: $e');
    }
    // No browser, or none willing: the address is still worth having.
    if (!opened && mounted) {
      await Clipboard.setData(ClipboardData(text: url));
      if (mounted) say(context, 'Copied the link instead');
    }
  }

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);

    Widget heading(String text) => Padding(
      padding: const EdgeInsets.fromLTRB(20, 26, 20, 8),
      child: Text(
        text,
        style: TextStyle(
          fontSize: 13,
          fontWeight: FontWeight.w600,
          color: p.muted,
        ),
      ),
    );

    return AnnotatedRegion<SystemUiOverlayStyle>(
      value: Theme.of(context).appBarTheme.systemOverlayStyle!,
      child: Scaffold(
        body: SafeArea(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Padding(
                padding: const EdgeInsets.fromLTRB(4, 4, 4, 0),
                child: Row(
                  children: [
                    IconButton(
                      onPressed: () => Navigator.of(context).pop(),
                      icon: const Icon(Icons.arrow_back),
                      color: p.text,
                      tooltip: 'Back',
                    ),
                    Text(
                      'About',
                      style: TextStyle(
                        fontSize: 16,
                        fontWeight: FontWeight.w600,
                        color: p.text,
                      ),
                    ),
                  ],
                ),
              ),
              Expanded(
                child: ListView(
                  padding: const EdgeInsets.only(bottom: 32),
                  children: [
                    const SizedBox(height: 20),
                    const Center(child: _Mark(size: 84)),
                    const SizedBox(height: 16),
                    Text(
                      'gyotaku',
                      textAlign: TextAlign.center,
                      style: TextStyle(
                        fontSize: 26,
                        fontWeight: FontWeight.w600,
                        color: p.text,
                      ),
                    ),
                    const SizedBox(height: 2),
                    Text(
                      _version == null ? ' ' : 'Version $_version for Android',
                      textAlign: TextAlign.center,
                      style: TextStyle(fontSize: 13.5, color: p.muted),
                    ),
                    Padding(
                      padding: const EdgeInsets.fromLTRB(28, 16, 28, 0),
                      child: Text(
                        'Search every screenshot by the text inside it. '
                        'Everything is read and kept on this phone. The only '
                        'time gyotaku uses the network is to download a '
                        'reader you turn on.',
                        textAlign: TextAlign.center,
                        style: TextStyle(
                          fontSize: 14.5,
                          height: 1.45,
                          color: p.muted,
                        ),
                      ),
                    ),
                    heading('Made by'),
                    _Line(
                      title: 'Yash Bavadiya',
                      detail: 'Created gyotaku · @xevrion',
                      onTap: () => _open('https://github.com/xevrion'),
                    ),
                    _Line(
                      title: 'Zaman Sheikh',
                      detail: 'Android app and Bengali reading · @zamansheikh',
                      onTap: () => _open('https://github.com/zamansheikh'),
                    ),
                    heading('Open source'),
                    _Line(
                      title: 'gyotaku on GitHub',
                      detail: 'github.com/xevrion/gyotaku',
                      onTap: () => _open(_project),
                    ),
                    _Line(
                      title: 'Source of this Android app',
                      detail: 'github.com/zamansheikh/gyotaku',
                      onTap: () => _open(_androidSource),
                    ),
                    _Line(
                      title: 'Website',
                      detail: 'gyotaku.app',
                      onTap: () => _open(_site),
                    ),
                    _Line(
                      title: 'License',
                      detail: 'GNU General Public License v3.0 or later',
                      onTap: () => _open('$_project/blob/main/LICENSE'),
                    ),
                    heading('Built with'),
                    const _Line(
                      title: 'PaddleOCR PP-OCRv6 and PP-OCRv5',
                      detail: 'Text finding and reading models, Apache-2.0',
                    ),
                    const _Line(
                      title: 'EasyOCR',
                      detail: 'The Bengali reading model, Apache-2.0',
                    ),
                    const _Line(
                      title: 'ONNX Runtime',
                      detail: 'Runs the models, MIT',
                    ),
                    const _Line(
                      title: 'ML Kit',
                      detail: "The phone's own text recognition",
                    ),
                    const _Line(
                      title: 'SQLite FTS5',
                      detail: 'The search index, public domain',
                    ),
                    const _Line(
                      title: 'Flutter and flutter_rust_bridge',
                      detail: 'The app and its bridge to the Rust core',
                    ),
                    const _Line(
                      title: 'IBM Plex Sans',
                      detail: 'The typeface, SIL Open Font License 1.1',
                    ),
                    Padding(
                      padding: const EdgeInsets.fromLTRB(20, 22, 20, 0),
                      child: TextButton(
                        onPressed: () => showLicensePage(
                          context: context,
                          applicationName: 'gyotaku',
                          applicationVersion: _version,
                        ),
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
                        child: const Text('All licenses'),
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

class _Line extends StatelessWidget {
  const _Line({required this.title, required this.detail, this.onTap});

  final String title;
  final String detail;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return InkWell(
      onTap: onTap,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(20, 9, 16, 9),
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(title, style: TextStyle(fontSize: 15.5, color: p.text)),
                  Text(detail, style: TextStyle(fontSize: 13, color: p.muted)),
                ],
              ),
            ),
            if (onTap != null) Icon(Icons.north_east, size: 18, color: p.faint),
          ],
        ),
      ),
    );
  }
}

/// The app's mark, drawn from the same shapes as `assets/icon.svg`: a fish
/// in lines of text, with the line the search found in shu.
class _Mark extends StatelessWidget {
  const _Mark({required this.size});

  final double size;

  @override
  Widget build(BuildContext context) {
    return SizedBox.square(
      dimension: size,
      child: CustomPaint(painter: _MarkPainter(Palette.dark)),
    );
  }
}

class _MarkPainter extends CustomPainter {
  _MarkPainter(this.p);

  // The icon is the same in both themes, like the one on the home screen.
  final Palette p;

  static const _bars = [
    (7.0, 10.5, 8.0),
    (11.0, 17.0, 8.0),
    (26.0, 17.0, 18.0),
    (17.0, 23.5, 33.0),
    (15.0, 36.5, 34.0),
    (11.0, 43.0, 8.0),
    (26.0, 43.0, 18.0),
    (7.0, 49.5, 8.0),
  ];

  @override
  void paint(Canvas canvas, Size size) {
    canvas.scale(size.width / 64);
    RRect pill(double x, double y, double w) => RRect.fromRectAndRadius(
      Rect.fromLTWH(x, y, w, 4),
      const Radius.circular(2),
    );
    canvas.drawRRect(
      RRect.fromRectAndRadius(
        const Rect.fromLTWH(0, 0, 64, 64),
        const Radius.circular(14),
      ),
      Paint()..color = p.panel,
    );
    final ink = Paint()..color = p.text;
    for (final (x, y, w) in _bars) {
      canvas.drawRRect(pill(x, y, w), ink);
    }
    canvas.drawCircle(const Offset(44.6, 25.5), 1.1, Paint()..color = p.panel);
    canvas.drawRRect(pill(13, 30, 44), Paint()..color = p.shu);
  }

  @override
  bool shouldRepaint(_MarkPainter old) => old.p != p;
}
