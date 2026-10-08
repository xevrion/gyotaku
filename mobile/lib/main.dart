import 'dart:io';

import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';

import 'reader.dart';
import 'search_page.dart';
import 'src/rust/api/index.dart' as core;
import 'src/rust/api/reader.dart' as own;
import 'src/rust/frb_generated.dart';
import 'theme.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();

  // The index lives in the app's own storage. It can always be rebuilt from
  // the photos, so it is kept out of backups' way in the support directory.
  String? failure;
  try {
    final dir = await getApplicationSupportDirectory();
    // Before anything else in the core runs: it works out its folders from
    // this. Android carries ONNX Runtime inside the app, found by name.
    own.settle(
      home: dir.path,
      runtime: Platform.isAndroid ? 'libonnxruntime.so' : null,
    );
    await core.openIndex(path: '${dir.path}/index.db');
  } catch (e) {
    failure = '$e';
  }

  runApp(GyotakuApp(failure: failure));
}

class GyotakuApp extends StatefulWidget {
  const GyotakuApp({super.key, this.failure});

  /// Why the index could not be opened, if it couldn't.
  final String? failure;

  @override
  State<GyotakuApp> createState() => _GyotakuAppState();
}

class _GyotakuAppState extends State<GyotakuApp> {
  final _reader = Reader();

  @override
  void dispose() {
    _reader.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'gyotaku',
      debugShowCheckedModeBanner: false,
      theme: themeFor(Brightness.light),
      darkTheme: themeFor(Brightness.dark),
      home: widget.failure == null
          ? SearchPage(reader: _reader)
          : Scaffold(
              body: SafeArea(
                child: Padding(
                  padding: const EdgeInsets.all(24),
                  child: Text(
                    'The index could not be opened.\n\n${widget.failure}',
                  ),
                ),
              ),
            ),
    );
  }
}
