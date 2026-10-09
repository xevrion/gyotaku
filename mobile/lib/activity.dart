import 'package:flutter/material.dart';

import 'reader.dart';
import 'theme.dart';

/// What the reader is doing right now, in words and a bar: looking through
/// the folders, downloading a model, reading. Nothing when it is idle.
class Activity extends StatelessWidget {
  const Activity({super.key, required this.reader});

  final Reader reader;

  static String _mb(int kb) => (kb / 1024).toStringAsFixed(1);

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: reader,
      builder: (context, _) {
        final (title, detail, progress) = switch (reader.state) {
          ReaderState.checking => (
            'Looking through your folders',
            reader.toCheck == 0
                ? 'Counting the images'
                : '${reader.checked} of ${reader.toCheck} images checked',
            reader.toCheck == 0 ? null : reader.checked / reader.toCheck,
          ),
          ReaderState.preparing when reader.downloading != null => (
            'Downloading ${modelName(reader.downloading!)}',
            reader.downloadKb == 0
                ? '${_mb(reader.downloadedKb)} MB so far'
                : '${_mb(reader.downloadedKb)} of ${_mb(reader.downloadKb)} MB',
            reader.downloadKb == 0
                ? null
                : reader.downloadedKb / reader.downloadKb,
          ),
          ReaderState.preparing => (
            "Getting gyotaku's own readers ready",
            'Downloading what is missing, then loading them',
            null,
          ),
          ReaderState.reading => (
            reader.deep
                ? 'Adding ${reader.scripts}'
                : 'Reading your screenshots',
            reader.deep
                ? '${reader.progressLine}. This is the slow part, and it '
                      'carries on with gyotaku put away.'
                : reader.progressLine,
            reader.total == 0 ? null : reader.done / reader.total,
          ),
          _ => (null, null, null),
        };

        final trouble = reader.trouble;
        final failed = reader.failed;
        final reading = reader.state == ReaderState.reading;
        // On hold with nothing under way: say so, and how to carry on.
        final held = reader.paused && !reader.busy && reader.waiting > 0;
        if (title == null && trouble == null && failed == 0 && !held) {
          return const SizedBox.shrink();
        }

        final p = Palette.of(context);
        return Container(
          margin: const EdgeInsets.fromLTRB(16, 0, 16, 10),
          padding: const EdgeInsets.fromLTRB(14, 12, 14, 12),
          decoration: BoxDecoration(
            color: p.tile,
            borderRadius: BorderRadius.circular(14),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              if (held) ...[
                Row(
                  children: [
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            'Reading is paused',
                            style: TextStyle(
                              fontSize: 14.5,
                              fontWeight: FontWeight.w600,
                              color: p.text,
                            ),
                          ),
                          const SizedBox(height: 2),
                          Text(
                            reader.waiting == 1
                                ? '1 image is waiting'
                                : '${reader.waiting} images are waiting',
                            style: TextStyle(fontSize: 13, color: p.muted),
                          ),
                        ],
                      ),
                    ),
                    _Button(label: 'Resume', onTap: reader.resume),
                  ],
                ),
                if (failed > 0 || trouble != null) const SizedBox(height: 12),
              ],
              if (title != null) ...[
                Row(
                  children: [
                    Expanded(
                      child: Text(
                        title,
                        style: TextStyle(
                          fontSize: 14.5,
                          fontWeight: FontWeight.w600,
                          color: p.text,
                        ),
                      ),
                    ),
                    if (reading && reader.paused)
                      Text(
                        'Pausing after this image',
                        style: TextStyle(fontSize: 13, color: p.muted),
                      )
                    else if (reading)
                      _Button(label: 'Pause', onTap: reader.pause),
                  ],
                ),
                if (detail!.isNotEmpty) ...[
                  const SizedBox(height: 2),
                  Text(
                    detail,
                    maxLines: 3,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      fontSize: 13,
                      height: 1.35,
                      color: p.muted,
                    ),
                  ),
                ],
                const SizedBox(height: 10),
                ClipRRect(
                  borderRadius: BorderRadius.circular(2),
                  child: LinearProgressIndicator(value: progress, minHeight: 4),
                ),
              ],
              if (failed > 0) ...[
                if (title != null) const SizedBox(height: 12),
                Text(
                  failed == 1
                      ? '1 image could not be read'
                      : '$failed images could not be read',
                  style: TextStyle(
                    fontSize: 14.5,
                    fontWeight: FontWeight.w600,
                    color: p.text,
                  ),
                ),
                const SizedBox(height: 2),
                Text(
                  'They are tried again the next time gyotaku looks. '
                  '${reader.failure ?? ''}',
                  maxLines: 3,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(fontSize: 13, height: 1.35, color: p.muted),
                ),
              ],
              if (trouble != null) ...[
                if (title != null || failed > 0) const SizedBox(height: 12),
                Text(
                  "gyotaku's own readers could not be used",
                  style: TextStyle(
                    fontSize: 14.5,
                    fontWeight: FontWeight.w600,
                    color: p.text,
                  ),
                ),
                const SizedBox(height: 2),
                Text(
                  "Everything is still searchable by what the phone's reader "
                  'made of it, which is no Bangla or Devanagari. $trouble',
                  maxLines: 4,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(fontSize: 13, height: 1.35, color: p.muted),
                ),
                if (!reader.busy) ...[
                  const SizedBox(height: 8),
                  GestureDetector(
                    onTap: reader.run,
                    child: Text(
                      'Try again',
                      style: TextStyle(
                        fontSize: 14,
                        fontWeight: FontWeight.w600,
                        color: p.text,
                      ),
                    ),
                  ),
                ],
              ],
            ],
          ),
        );
      },
    );
  }
}

/// A word to press, sized for a thumb without shouting like a filled button.
class _Button extends StatelessWidget {
  const _Button({required this.label, required this.onTap});

  final String label;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return GestureDetector(
      onTap: onTap,
      behavior: HitTestBehavior.opaque,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 7),
        decoration: BoxDecoration(
          border: Border.all(color: p.track),
          borderRadius: BorderRadius.circular(14),
        ),
        child: Text(
          label,
          style: TextStyle(
            fontSize: 13.5,
            fontWeight: FontWeight.w600,
            color: p.text,
          ),
        ),
      ),
    );
  }
}
