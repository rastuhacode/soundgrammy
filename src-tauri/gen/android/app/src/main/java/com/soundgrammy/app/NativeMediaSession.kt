package com.soundgrammy.app

import android.content.Context
import android.graphics.BitmapFactory
import android.media.MediaMetadata
import android.media.session.MediaSession
import android.media.session.PlaybackState
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import org.json.JSONObject
import java.util.concurrent.Executors

/** Process-owned session shared with the playback foreground service. */
class NativeMediaSession private constructor(context: Context) {
    private val application = context.applicationContext
    private val handler = Handler(Looper.getMainLooper())
    private val artworkWorker = Executors.newSingleThreadExecutor()
    private val session = MediaSession(context.applicationContext, "SoundGrammy")
    private var released = false
    private var revision = -1L
    private var identity: String? = null
    private var artworkPath: String? = null
    private var artwork: android.graphics.Bitmap? = null
    private var current: JSONObject? = null

    // The foreground service reuses this token.
    val token: MediaSession.Token get() = session.sessionToken

    init {
        session.setCallback(object : MediaSession.Callback() {
            override fun onPlay() = nativeCommand(0, 0.0)
            override fun onPause() = nativeCommand(1, 0.0)
            override fun onStop() = nativeCommand(2, 0.0)
            override fun onSkipToNext() = nativeCommand(3, 0.0)
            override fun onSkipToPrevious() = nativeCommand(4, 0.0)
            override fun onSeekTo(pos: Long) = nativeCommand(5, pos / 1000.0)
        }, handler)
    }

    fun acquire(): Boolean = PlaybackService.acquire(application)

    fun update(json: String) {
        val receivedAt = SystemClock.elapsedRealtime()
        handler.post {
            if (released) return@post
            val p = JSONObject(json)
            val nextRevision = p.getLong("revision")
            if (nextRevision < revision) return@post
            revision = nextRevision
            val nextIdentity = p.optJSONObject("identity")?.let { "${it.getLong("track")}:${it.getString("attempt")}" }
            val path = if (p.isNull("artwork")) null else p.getString("artwork")
            val artworkChanged = identity != nextIdentity || artworkPath != path
            identity = nextIdentity
            artworkPath = path
            current = p
            PlaybackService.update(p)
            if (artworkChanged) artwork = null
            session.isActive = identity != null
            publishMetadata()
            val actions = p.getJSONObject("actions")
            var mask = 0L
            if (actions.getBoolean("play")) mask = mask or PlaybackState.ACTION_PLAY
            if (actions.getBoolean("pause")) mask = mask or PlaybackState.ACTION_PAUSE
            if (identity != null) mask = mask or PlaybackState.ACTION_PLAY_PAUSE
            if (actions.getBoolean("stop")) mask = mask or PlaybackState.ACTION_STOP
            if (actions.getBoolean("next")) mask = mask or PlaybackState.ACTION_SKIP_TO_NEXT
            if (actions.getBoolean("previous")) mask = mask or PlaybackState.ACTION_SKIP_TO_PREVIOUS
            if (actions.getBoolean("seek")) mask = mask or PlaybackState.ACTION_SEEK_TO
            val status = when (p.getString("status")) {
                "playing" -> PlaybackState.STATE_PLAYING
                "loading", "buffering" -> PlaybackState.STATE_BUFFERING
                "paused", "ready" -> PlaybackState.STATE_PAUSED
                "error" -> PlaybackState.STATE_ERROR
                else -> PlaybackState.STATE_STOPPED
            }
            session.setPlaybackState(PlaybackState.Builder().setActions(mask)
                .setState(status, (p.getDouble("position") * 1000).toLong(), p.getDouble("rate").toFloat(), receivedAt).build())
            if (artworkChanged && path != null && identity != null) {
                artworkWorker.execute {
                    val bitmap = runCatching { BitmapFactory.decodeFile(path) }.getOrNull()
                    handler.post {
                        if (!released && identity == nextIdentity && artworkPath == path) {
                            artwork = bitmap
                            publishMetadata()
                        }
                    }
                }
            }
        }
    }

    private fun publishMetadata() {
        val p = current
        if (identity == null || p == null) { session.setMetadata(null); return }
        val metadata = MediaMetadata.Builder()
            .putString(MediaMetadata.METADATA_KEY_MEDIA_ID, identity)
            .putString(MediaMetadata.METADATA_KEY_TITLE, p.getString("title"))
            .putString(MediaMetadata.METADATA_KEY_ARTIST, p.getString("artist"))
            .putLong(MediaMetadata.METADATA_KEY_DURATION, (p.getDouble("duration") * 1000).toLong())
        artwork?.let { metadata.putBitmap(MediaMetadata.METADATA_KEY_ART, it) }
        session.setMetadata(metadata.build())
    }

    fun release() {
        handler.post {
            if (released) return@post
            PlaybackService.shutdown()
            released = true
            current = null
            identity = null
            artwork = null
            session.isActive = false
            session.setMetadata(null)
            session.setCallback(null)
            session.release()
            artworkWorker.shutdownNow()
        }
    }

    companion object {
        @Volatile private var instance: NativeMediaSession? = null
        @JvmStatic @Synchronized fun getOrCreate(context: Context): NativeMediaSession =
            instance ?: NativeMediaSession(context.applicationContext).also { instance = it }
        fun command(command: Int) = nativeCommand(command, 0.0)
        fun lifecycle(event: Int, token: Long) = nativeLifecycle(event, token)
        @JvmStatic private external fun nativeLifecycle(event: Int, token: Long)
        @JvmStatic private external fun nativeCommand(command: Int, seconds: Double)
    }
}
