package com.soundgrammy.app

import android.app.*
import android.content.*
import android.content.pm.ServiceInfo
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.os.*
import org.json.JSONObject
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

/** Keeps the existing Rust runtime and MediaSession alive; owns no decoder or queue.
 * Not sticky: process death/force-stop requires an explicit user launch and play.
 */
class PlaybackService : Service() {
    private var intentionalStop = false
    private lateinit var wake: PowerManager.WakeLock
    private val noisy = object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            if (intent?.action == AudioManager.ACTION_AUDIO_BECOMING_NOISY) NativeMediaSession.lifecycle(2, 0)
        }
    }
    override fun onCreate() {
        super.onCreate()
        instance = this
        wake = (getSystemService(POWER_SERVICE) as PowerManager)
            .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "SoundGrammy:playback")
        getSystemService(NotificationManager::class.java).createNotificationChannel(
            NotificationChannel(CHANNEL, "Music playback", NotificationManager.IMPORTANCE_LOW))
        if (Build.VERSION.SDK_INT >= 33) registerReceiver(noisy, IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY), RECEIVER_NOT_EXPORTED)
        else registerReceiver(noisy, IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY))
        val notification = notification()
        if (Build.VERSION.SDK_INT >= 29) startForeground(ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK)
        else startForeground(ID, notification)
        wake.acquire()
    }
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        instance = this
        starting = false
        intentionalStop = false
        val notification = notification()
        if (Build.VERSION.SDK_INT >= 29) startForeground(ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK)
        else startForeground(ID, notification)
        val callbacks = waiting.toList()
        waiting.clear()
        callbacks.forEach { it() }
        intent?.getIntExtra("command", -1)?.takeIf { it >= 0 }?.let { NativeMediaSession.command(it) }
        refresh()
        return START_NOT_STICKY
    }
    override fun onBind(intent: Intent?): IBinder? = null
    // A swipe from Recents is an explicit close, unlike moving the Activity to
    // the background. The foreground service otherwise keeps Rust and the old
    // WebView process alive, so a later launch can open onto a blank window.
    override fun onTaskRemoved(rootIntent: Intent?) {
        TaskRemoval.close(::finish) { Process.killProcess(Process.myPid()) }
    }
    private fun refresh() {
        if (!wanted) { finish(); return }
        val needsWake = presentation?.optString("status") != "paused"
        if (needsWake && !wake.isHeld) wake.acquire()
        if (!needsWake && wake.isHeld) wake.release()
        getSystemService(NotificationManager::class.java).notify(ID, notification())
    }
    private fun finish() {
        intentionalStop = true
        if (instance === this) instance = null
        starting = false
        abandonFocus()
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }
    override fun onDestroy() {
        if (instance === this) {
            instance = null
            starting = false
            abandonFocus()
        }
        unregisterReceiver(noisy)
        if (wake.isHeld) wake.release()
        if (!intentionalStop) NativeMediaSession.command(2)
        super.onDestroy()
    }
    private fun action(label: String, command: Int, icon: Int): Notification.Action {
        val intent = Intent(this, PlaybackService::class.java).putExtra("command", command)
        val pending = PendingIntent.getService(this, command, intent, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        return Notification.Action.Builder(icon, label, pending).build()
    }
    private fun notification(): Notification {
        val p = presentation
        val launch = packageManager.getLaunchIntentForPackage(packageName)
        val builder = Notification.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.ic_media_play)
            .setContentTitle(p?.optString("title") ?: "SoundGrammy")
            .setContentText(p?.optString("artist") ?: "")
            .setOnlyAlertOnce(true).setOngoing(true)
            .setVisibility(Notification.VISIBILITY_PUBLIC)
            .addAction(action("Previous", 4, android.R.drawable.ic_media_previous))
            .addAction(action("Pause", 1, android.R.drawable.ic_media_pause))
            .addAction(action("Next", 3, android.R.drawable.ic_media_next))
            .addAction(action("Stop", 2, android.R.drawable.ic_menu_close_clear_cancel))
            .setStyle(Notification.MediaStyle().setMediaSession(NativeMediaSession.getOrCreate(this).token).setShowActionsInCompactView(0, 1, 2))
        if (launch != null) builder.setContentIntent(PendingIntent.getActivity(this, 0, launch, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE))
        return builder.build()
    }
    companion object {
        private const val CHANNEL = "playback"
        private const val ID = 31
        private var instance: PlaybackService? = null
        private var starting = false
        private val waiting = mutableListOf<() -> Unit>()
        private var focusGeneration = 0L
        @Volatile private var wanted = false
        private var presentation: JSONObject? = null
        private var manager: AudioManager? = null
        private var focus: AudioFocusRequest? = null
        private var held = false
        private var interruption = 0L
        private val handler = Handler(Looper.getMainLooper())

        /** Rust waits for foreground promotion and focus before enabling PCM. */
        fun acquire(context: Context): Boolean {
            if (Looper.myLooper() == Looper.getMainLooper()) return false
            val ready = CountDownLatch(1)
            val accepted = AtomicBoolean(false)
            val expired = AtomicBoolean(false)
            handler.post {
                val complete = {
                    if (!expired.get()) accepted.set(requestFocus(context))
                    ready.countDown()
                }
                wanted = true
                if (instance != null) complete()
                else {
                    waiting.add(complete)
                    if (!starting) {
                        try {
                            context.startForegroundService(Intent(context, PlaybackService::class.java))
                            starting = true
                        } catch (_: RuntimeException) {
                            wanted = false
                            waiting.clear()
                            ready.countDown()
                        }
                    }
                }
            }
            if (!ready.await(3, TimeUnit.SECONDS)) {
                expired.set(true)
                handler.post { wanted = false; instance?.refresh() }
                return false
            }
            return accepted.get()
        }
        private fun requestFocus(context: Context): Boolean {
            if (held) return true
            val audio = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager
            manager = audio
            focusGeneration += 1
            val generation = focusGeneration
            val request = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
                .setAudioAttributes(AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_MEDIA).setContentType(AudioAttributes.CONTENT_TYPE_MUSIC).build())
                .setWillPauseWhenDucked(true)
                .setOnAudioFocusChangeListener({ change ->
                    if (generation == focusGeneration) {
                        when (change) {
                            AudioManager.AUDIOFOCUS_GAIN -> NativeMediaSession.lifecycle(1, interruption)
                            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT, AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK -> {
                                interruption += 1
                                NativeMediaSession.lifecycle(0, interruption)
                            }
                            AudioManager.AUDIOFOCUS_LOSS -> {
                                abandonFocus()
                                NativeMediaSession.lifecycle(2, 0)
                            }
                        }
                    }
                }, handler).build()
            return try {
                if (audio.requestAudioFocus(request) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED) {
                    focus = request
                    held = true
                    true
                } else {
                    wanted = false
                    instance?.refresh()
                    false
                }
            } catch (_: RuntimeException) {
                wanted = false
                instance?.refresh()
                false
            }
        }
        fun abandonFocus() {
            focusGeneration += 1
            focus?.let { manager?.abandonAudioFocusRequest(it) }
            focus = null
            held = false
        }
        fun shutdown() {
            wanted = false
            abandonFocus()
            instance?.finish()
        }
        fun update(p: JSONObject) {
            presentation = p
            wanted = p.getBoolean("background_active")
            if (!wanted) abandonFocus()
            instance?.refresh()
        }
    }
}
