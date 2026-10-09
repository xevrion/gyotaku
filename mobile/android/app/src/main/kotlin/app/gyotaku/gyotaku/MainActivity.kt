package app.gyotaku.gyotaku

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.embedding.engine.FlutterEngineCache
import io.flutter.embedding.engine.dart.DartExecutor
import io.flutter.plugin.common.MethodChannel
import java.lang.ref.WeakReference

class MainActivity : FlutterActivity() {
    /**
     * The app runs in one engine that outlives this window.
     *
     * The reader lives in Dart. With the usual engine, one made for the
     * activity and destroyed with it, swiping the app out of the recent apps
     * list destroyed the reader mid-read, however much was left. An engine
     * handed over by the host is left alone when the activity goes, so the
     * reading carries on behind its notification, and the next window opened
     * finds the app as it was.
     */
    override fun provideFlutterEngine(context: Context): FlutterEngine = engine(context)

    override fun onCreate(savedInstanceState: android.os.Bundle?) {
        front = WeakReference(this)
        super.onCreate(savedInstanceState)
    }

    override fun onDestroy() {
        if (front.get() === this) front = WeakReference(null)
        super.onDestroy()
    }

    companion object {
        private const val ENGINE = "gyotaku"

        // The window on screen, if there is one, for the one thing that
        // needs it: asking for a permission.
        private var front = WeakReference<MainActivity>(null)

        fun engine(context: Context): FlutterEngine {
            FlutterEngineCache.getInstance().get(ENGINE)?.let { return it }
            val app = context.applicationContext
            val engine = FlutterEngine(app)
            channel(app, engine)
            engine.dartExecutor.executeDartEntrypoint(DartExecutor.DartEntrypoint.createDefault())
            FlutterEngineCache.getInstance().put(ENGINE, engine)
            return engine
        }

        // How the reader says what the notification should show, and how
        // Pause on the notification gets back to it. Set up with the engine,
        // not with a window, so it works when there is no window.
        private fun channel(app: Context, engine: FlutterEngine) {
            val channel = MethodChannel(engine.dartExecutor.binaryMessenger, "gyotaku/scan")
            val main = Handler(Looper.getMainLooper())
            ScanService.onPause = { main.post { channel.invokeMethod("pause", null) } }
            channel.setMethodCallHandler { call, result ->
                when (call.method) {
                    "show" -> result.success(
                        ScanService.show(
                            app,
                            call.argument<String>("title") ?: "",
                            call.argument<String>("text") ?: "",
                            call.argument<Int>("done") ?: 0,
                            call.argument<Int>("total") ?: 0,
                        ),
                    )
                    "finish" -> {
                        ScanService.finish(
                            app,
                            call.argument<String>("title"),
                            call.argument<String>("text"),
                        )
                        result.success(null)
                    }
                    "ask" -> {
                        // Android 13 and later show nothing until asked.
                        val activity = front.get()
                        if (activity != null &&
                            Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
                            activity.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) !=
                            PackageManager.PERMISSION_GRANTED
                        ) {
                            activity.requestPermissions(
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
    }
}
