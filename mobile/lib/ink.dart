import 'dart:io';

import 'package:flutter/material.dart';

import 'src/rust/api/index.dart' as core;
import 'theme.dart';

/// A screenshot, cut to `crop`, inked dark with only `lit` left showing.
/// With nothing lit it is shown plain: there is no query to answer.
class InkedShot extends StatelessWidget {
  const InkedShot({
    super.key,
    required this.path,
    required this.crop,
    required this.lit,
    this.decodeWidth,
  });

  final String path;
  final core.Rect crop;
  final List<core.Line> lit;

  /// Decode no larger than this many pixels wide. A full size phone
  /// screenshot is around 10 MB once decoded, and a grid shows dozens.
  final int? decodeWidth;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return Stack(
      fit: StackFit.expand,
      children: [
        Image.file(
          File(path),
          fit: BoxFit.cover,
          // Tall shots keep their top and wide ones their middle, which is
          // what `tile_crop` in the core decides.
          alignment: crop.h < 1 ? Alignment.topCenter : Alignment.center,
          cacheWidth: decodeWidth,
          gaplessPlayback: true,
          filterQuality: FilterQuality.medium,
          errorBuilder: (_, _, _) => ColoredBox(color: p.tile),
        ),
        if (lit.isNotEmpty)
          CustomPaint(
            painter: _Ink(crop: crop, lit: lit, ink: p.ink, shu: p.shu),
          ),
      ],
    );
  }
}

class _Ink extends CustomPainter {
  _Ink({
    required this.crop,
    required this.lit,
    required this.ink,
    required this.shu,
  });

  final core.Rect crop;
  final List<core.Line> lit;
  final Color ink;
  final Color shu;

  @override
  void paint(Canvas canvas, Size size) {
    final bounds = Offset.zero & size;
    final holes = [
      for (final l in lit)
        Rect.fromLTWH(
          (l.rect.x - crop.x) / crop.w * size.width,
          (l.rect.y - crop.y) / crop.h * size.height,
          l.rect.w / crop.w * size.width,
          l.rect.h / crop.h * size.height,
        ).inflate(2).intersect(bounds),
    ].where((r) => r.width > 0 && r.height > 0).toList();

    canvas.saveLayer(bounds, Paint());
    canvas.drawRect(bounds, Paint()..color = ink);
    final clear = Paint()..blendMode = BlendMode.clear;
    for (final r in holes) {
      canvas.drawRRect(
        RRect.fromRectAndRadius(r, const Radius.circular(2)),
        clear,
      );
    }
    canvas.restore();

    final under = Paint()
      ..color = shu
      ..strokeWidth = 1.5;
    for (final r in holes) {
      canvas.drawLine(r.bottomLeft, r.bottomRight, under);
    }
  }

  @override
  bool shouldRepaint(_Ink old) =>
      old.lit != lit || old.crop != crop || old.ink != ink || old.shu != shu;
}
