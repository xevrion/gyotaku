package app.gyotaku.gyotaku

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat

/**
 * Keeps the app alive while it reads screenshots with its window put away.
 *
 * The reading itself happens in the Flutter side of the same process. All
 * this does is tell Android that work the person asked for is under way, so
 * the process is not frozen the moment it leaves the screen, hold the
 * processor awake for it, and show how far along it is.
 *
 * It ends when the reading does, when it is paused, or when Android says its
 * time is up. Swiping the app out of the recent apps list does not end it:
 * the engine the reader runs in outlives the window, see MainActivity.
 */
class ScanService : Service() {
    private var awake: PowerManager.WakeLock? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // The Pause button on the notification. The reading is not ours to
        // stop, it happens in the Flutter side: pass the word on.
        if (intent?.action == ACTION_PAUSE) {
            onPause?.invoke()
            if (!running) stopSelf()
            return START_NOT_STICKY
        }
        val notification = progress(
            this,
            intent?.getStringExtra(TITLE) ?: "Reading screenshots",
            intent?.getStringExtra(TEXT) ?: "",
            intent?.getIntExtra(DONE, 0) ?: 0,
            intent?.getIntExtra(TOTAL, 0) ?: 0,
        )
        try {
            ServiceCompat.startForeground(
                this,
                PROGRESS_ID,
                notification,
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                    ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC
                } else {
                    0
                },
            )
        } catch (e: Exception) {
            // Not allowed right now (started from the background, say). The
            // reading carries on for as long as Android lets it.
            stopSelf()
            return START_NOT_STICKY
        }
        running = true
        if (awake == null) {
            val power = getSystemService(Context.POWER_SERVICE) as PowerManager
            awake = power.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "gyotaku:reading").apply {
                setReferenceCounted(false)
                // A ceiling, in case nothing ever says the reading is over.
                acquire(6 * 60 * 60 * 1000L)
            }
        }
        // If the process is killed there is no reading left to keep alive.
        return START_NOT_STICKY
    }

    // Android 15 gives this kind of service six hours a day.
    override fun onTimeout(startId: Int, fgsType: Int) {
        stopSelf()
    }

    override fun onDestroy() {
        running = false
        awake?.let { if (it.isHeld) it.release() }
        awake = null
        ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
        super.onDestroy()
    }

    companion object {
        const val TITLE = "title"
        const val TEXT = "text"
        const val DONE = "done"
        const val TOTAL = "total"

        const val ACTION_PAUSE = "app.gyotaku.gyotaku.PAUSE"

        /** Set by the activity: what to do when Pause is pressed. */
        @Volatile
        var onPause: (() -> Unit)? = null

        @Volatile
        private var running = false

        private const val PROGRESS_ID = 1
        private const val REPORT_ID = 2
        private const val PROGRESS_CHANNEL = "reading"
        private const val REPORT_CHANNEL = "report"

        private fun channels(context: Context) {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
            val manager = context.getSystemService(NotificationManager::class.java)
            // Low importance: shown, never a sound or a pop-up. Progress that
            // buzzed on every screenshot would be turned off within a minute.
            manager.createNotificationChannel(
                NotificationChannel(
                    PROGRESS_CHANNEL,
                    "Reading in progress",
                    NotificationManager.IMPORTANCE_LOW,
                ),
            )
            manager.createNotificationChannel(
                NotificationChannel(
                    REPORT_CHANNEL,
                    "Reading finished",
                    NotificationManager.IMPORTANCE_LOW,
                ),
            )
        }

        private fun open(context: Context): PendingIntent = PendingIntent.getActivity(
            context,
            0,
            Intent(context, MainActivity::class.java)
                .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        private fun progress(
            context: Context,
            title: String,
            text: String,
            done: Int,
            total: Int,
        ): Notification {
            channels(context)
            return NotificationCompat.Builder(context, PROGRESS_CHANNEL)
                .setSmallIcon(R.drawable.ic_stat_gyotaku)
                .setContentTitle(title)
                .setContentText(text)
                // With no total, a bar that only says "working".
                .setProgress(total, done, total <= 0)
                .setOngoing(true)
                .setOnlyAlertOnce(true)
                .setCategory(NotificationCompat.CATEGORY_PROGRESS)
                .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
                .setContentIntent(open(context))
                .addAction(
                    0,
                    "Pause",
                    PendingIntent.getService(
                        context,
                        1,
                        Intent(context, ScanService::class.java).setAction(ACTION_PAUSE),
                        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
                    ),
                )
                .build()
        }

        /** Starts the service, or just moves its bar if it is running. */
        fun show(context: Context, title: String, text: String, done: Int, total: Int): Boolean {
            // Once it is running, moving the bar is only a notification with
            // the same id. Starting a service again is something Android
            // refuses an app that is no longer on screen.
            if (running) {
                return try {
                    context.getSystemService(NotificationManager::class.java)
                        .notify(PROGRESS_ID, progress(context, title, text, done, total))
                    true
                } catch (e: SecurityException) {
                    false
                }
            }
            val intent = Intent(context, ScanService::class.java)
                .putExtra(TITLE, title)
                .putExtra(TEXT, text)
                .putExtra(DONE, done)
                .putExtra(TOTAL, total)
            return try {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    context.startForegroundService(intent)
                } else {
                    context.startService(intent)
                }
                true
            } catch (e: Exception) {
                false
            }
        }

        /**
         * Ends the service. With a title, leaves a notification behind that
         * says how the reading went, for someone who was not looking.
         */
        fun finish(context: Context, title: String?, text: String?) {
            context.stopService(Intent(context, ScanService::class.java))
            val manager = context.getSystemService(NotificationManager::class.java)
            if (title == null) {
                manager.cancel(REPORT_ID)
                return
            }
            channels(context)
            try {
                manager.notify(
                    REPORT_ID,
                    NotificationCompat.Builder(context, REPORT_CHANNEL)
                        .setSmallIcon(R.drawable.ic_stat_gyotaku)
                        .setContentTitle(title)
                        .setContentText(text ?: "")
                        .setStyle(NotificationCompat.BigTextStyle().bigText(text ?: ""))
                        .setAutoCancel(true)
                        .setContentIntent(open(context))
                        .build(),
                )
            } catch (e: SecurityException) {
                // Notifications were refused. Nothing to do about that here.
            }
        }
    }
}
