import 'package:flutter/cupertino.dart' show CupertinoPageTransitionsBuilder;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

/// The same palette as the desktop app (`crates/app/src/theme.rs`). The
/// screenshots are the colour; everything around them is off white or near
/// black. Shu only ever means "the search found this", and selection is ink.
class Palette {
  const Palette({
    required this.panel,
    required this.tile,
    required this.track,
    required this.text,
    required this.muted,
    required this.faint,
    required this.shu,
    required this.ink,
  });

  final Color panel;
  final Color tile;
  final Color track;
  final Color text;
  final Color muted;
  final Color faint;
  final Color shu;

  /// What a screenshot is covered with while searching, so only the matching
  /// lines stay lit.
  final Color ink;

  static const light = Palette(
    panel: Color(0xfff6f6f4),
    tile: Color(0xffe7e6e2),
    track: Color(0xffdad9d4),
    text: Color(0xff18181a),
    muted: Color(0xff6d6c68),
    faint: Color(0xffa9a7a1),
    shu: Color(0xffe0531f),
    ink: Color(0xc718181a),
  );

  static const dark = Palette(
    panel: Color(0xff141416),
    tile: Color(0xff222226),
    track: Color(0xff3a3a3f),
    text: Color(0xffecebe7),
    muted: Color(0xff9b9a95),
    faint: Color(0xff5f5e5a),
    shu: Color(0xffff7438),
    ink: Color(0xd10a0a0b),
  );

  static Palette of(BuildContext context) =>
      Theme.of(context).brightness == Brightness.dark ? dark : light;
}

ThemeData themeFor(Brightness brightness) {
  final dark = brightness == Brightness.dark;
  final p = dark ? Palette.dark : Palette.light;
  return ThemeData(
    brightness: brightness,
    fontFamily: 'IBM Plex Sans',
    scaffoldBackgroundColor: p.panel,
    colorScheme: ColorScheme.fromSeed(
      seedColor: p.text,
      brightness: brightness,
      surface: p.panel,
      onSurface: p.text,
      primary: p.text,
      onPrimary: p.panel,
    ),
    textSelectionTheme: TextSelectionThemeData(
      cursorColor: p.text,
      selectionColor: p.faint.withValues(alpha: 0.4),
      selectionHandleColor: p.text,
    ),
    splashFactory: NoSplash.splashFactory,
    highlightColor: Colors.transparent,
    snackBarTheme: SnackBarThemeData(
      behavior: SnackBarBehavior.floating,
      backgroundColor: p.text,
      contentTextStyle: TextStyle(
        fontFamily: 'IBM Plex Sans',
        fontSize: 14,
        color: p.panel,
      ),
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      insetPadding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
      elevation: 0,
    ),
    bottomSheetTheme: BottomSheetThemeData(
      backgroundColor: p.panel,
      surfaceTintColor: Colors.transparent,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
      ),
    ),
    progressIndicatorTheme: ProgressIndicatorThemeData(
      color: p.text,
      linearTrackColor: p.track,
    ),
    appBarTheme: AppBarTheme(
      systemOverlayStyle: SystemUiOverlayStyle(
        statusBarColor: Colors.transparent,
        systemNavigationBarColor: Colors.transparent,
        statusBarIconBrightness: dark ? Brightness.light : Brightness.dark,
        statusBarBrightness: brightness,
        systemNavigationBarIconBrightness: dark
            ? Brightness.light
            : Brightness.dark,
      ),
    ),
    // Nothing moves on its own, and a page arriving is no exception.
    pageTransitionsTheme: const PageTransitionsTheme(
      builders: {
        TargetPlatform.android: FadeForwardsPageTransitionsBuilder(),
        TargetPlatform.iOS: CupertinoPageTransitionsBuilder(),
      },
    ),
  );
}

/// One short line at the bottom of the screen, replacing whatever was there.
void say(BuildContext context, String text) {
  ScaffoldMessenger.of(context)
    ..hideCurrentSnackBar()
    ..showSnackBar(
      SnackBar(content: Text(text), duration: const Duration(seconds: 2)),
    );
}

/// The filled, ink coloured button used for the one thing a screen asks for.
class InkButton extends StatelessWidget {
  const InkButton({super.key, required this.label, required this.onPressed});

  final String label;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final p = Palette.of(context);
    return FilledButton(
      onPressed: onPressed,
      style: FilledButton.styleFrom(
        backgroundColor: p.text,
        foregroundColor: p.panel,
        minimumSize: const Size(0, 48),
        padding: const EdgeInsets.symmetric(horizontal: 24),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(14)),
        textStyle: const TextStyle(
          fontFamily: 'IBM Plex Sans',
          fontSize: 15,
          fontWeight: FontWeight.w600,
        ),
      ),
      child: Text(label),
    );
  }
}
