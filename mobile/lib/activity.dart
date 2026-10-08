import 'package:flutter/material.dart';

import 'reader.dart';
import 'theme.dart';

/// What the reader is doing right now, in words and a bar: looking through
/// the folders, downloading a model, reading. Nothing when it is idle.
class Activity extends StatelessWidget {
  const Activity({super.key, required this.reader});

  final Reader reader;

  /// What a model file is, to someone who never asked for its name.
  static String _named(String file) {
    final f = file.toLowerCase();
    if (f.contains('bengali')) return 'the Bengali reader';
    if (f.contains('devanagari')) return 'the Devanagari reader';
    if (f.contains('_det')) return 'the text finder';
    if (f.contains('_rec')) return 'the text reader';
    return file;
  }

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
            'Downloading ${_named(reader.downloading!)}',
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
            'Reading ${(reader.done + 1).clamp(1, reader.total)} of ${reader.total}',
            reader.own_
                ? "With gyotaku's own readers, a few seconds each"
                : reader.current ?? '',
            reader.total == 0 ? null : reader.done / reader.total,
          ),
          _ => (null, null, null),
        };

        final trouble = reader.trouble;
        final failed = reader.failed;
        if (title == null && trouble == null && failed == 0) {
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
              if (title != null) ...[
                Text(
                  title,
                  style: TextStyle(
                    fontSize: 14.5,
                    fontWeight: FontWeight.w600,
                    color: p.text,
                  ),
                ),
                if (detail!.isNotEmpty) ...[
                  const SizedBox(height: 2),
                  Text(
                    detail,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(fontSize: 13, color: p.muted),
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
                  "The phone's reader is being used instead, which reads no "
                  'Bangla or Devanagari. $trouble',
                  maxLines: 4,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(fontSize: 13, height: 1.35, color: p.muted),
                ),
                if (!reader.busy) ...[
                  const SizedBox(height: 8),
                  GestureDetector(
                    onTap: reader.prepare,
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
