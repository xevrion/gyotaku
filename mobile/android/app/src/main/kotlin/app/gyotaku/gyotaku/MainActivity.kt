package app.gyotaku.gyotaku

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

class MainActivity : FlutterActivity() {
    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        // How the reader, which lives in Dart, says what the notification
        // should show. See ScanService.
        val channel = MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "gyotaku/scan")
        // Pause pressed on the notification, possibly with this window put
        // away. The reader lives in Dart, so it is told there.
        ScanService.onPause = { runOnUiThread { channel.invokeMethod("pause", null) } }
        channel.setMethodCallHandler { call, result ->
                when (call.method) {
                    "show" -> result.success(
                        ScanService.show(
                            applicationContext,
                            call.argument<String>("title") ?: "",
                            call.argument<String>("text") ?: "",
                            call.argument<Int>("done") ?: 0,
                            call.argument<Int>("total") ?: 0,
                        ),
                    )
                    "finish" -> {
                        ScanService.finish(
                            applicationContext,
                            call.argument<String>("title"),
                            call.argument<String>("text"),
                        )
                        result.success(null)
                    }
                    "ask" -> {
                        // Android 13 and later show nothing until asked.
                        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
                            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) !=
                            PackageManager.PERMISSION_GRANTED
                        ) {
                            requestPermissions(
                                arrayOf(Manifest.permission.POST_NOTIFICATIONS),
                                1,
                            )
                        }
                        result.success(null)
                    }
                    else -> result.notImplemented()
                }
            }
    }

    override fun cleanUpFlutterEngine(flutterEngine: FlutterEngine) {
        ScanService.onPause = null
        super.cleanUpFlutterEngine(flutterEngine)
    }
}
