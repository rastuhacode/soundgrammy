package com.soundgrammy.app

import android.graphics.Bitmap
import android.graphics.Color
import android.media.MediaMetadata
import android.media.session.MediaController
import android.os.SystemClock
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONObject
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File

@RunWith(AndroidJUnit4::class)
class NativeMediaSessionTest {
    @Test
    fun replacesPreviousCoverWithAppIconAndClearsMetadataWithQueue() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val context = instrumentation.targetContext
        lateinit var session: NativeMediaSession
        instrumentation.runOnMainSync { session = NativeMediaSession.getOrCreate(context) }
        val controller = MediaController(context, session.token)
        val cover = File.createTempFile("media-cover-", ".png", context.cacheDir)
        val bitmap = Bitmap.createBitmap(8, 8, Bitmap.Config.ARGB_8888)
        bitmap.eraseColor(Color.MAGENTA)
        cover.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
        bitmap.recycle()

        fun update(revision: Long, track: Long?, path: String? = null) {
            val p = JSONObject()
                .put("revision", revision)
                .put("identity", track?.let {
                    JSONObject().put("track", it).put("attempt", "test-$revision")
                } ?: JSONObject.NULL)
                .put("artwork", path ?: JSONObject.NULL)
                .put("title", "Track $track")
                .put("artist", "Test artist")
                .put("duration", 60.0)
                .put("position", 0.0)
                .put("rate", 0.0)
                .put("status", "paused")
                .put("background_active", false)
                .put("actions", JSONObject()
                    .put("play", true).put("pause", false).put("stop", true)
                    .put("next", false).put("previous", false).put("seek", false))
            session.update(p.toString())
        }

        fun awaitMetadata(matches: (MediaMetadata?) -> Boolean): MediaMetadata? {
            val deadline = SystemClock.elapsedRealtime() + 3000
            while (SystemClock.elapsedRealtime() < deadline) {
                val metadata = controller.metadata
                if (matches(metadata)) return metadata
                SystemClock.sleep(20)
            }
            throw AssertionError("Expected media metadata was not published")
        }

        fun awaitArtwork(track: Long, matches: (Bitmap) -> Boolean): Bitmap {
            val metadata = awaitMetadata {
                it?.getString(MediaMetadata.METADATA_KEY_TITLE) == "Track $track" &&
                    it.getBitmap(MediaMetadata.METADATA_KEY_ART)?.let(matches) == true
            }
            return metadata!!.getBitmap(MediaMetadata.METADATA_KEY_ART)!!
        }

        try {
            update(1, 1)
            val fallback = awaitArtwork(1) { true }

            update(2, 2, cover.absolutePath)
            awaitArtwork(2) { it.getPixel(0, 0) == Color.MAGENTA }

            update(3, 3)
            assertTrue(awaitArtwork(3) { it.sameAs(fallback) }.sameAs(fallback))

            // An unreadable cover also retains the placeholder, never the old cover.
            update(4, 4, "${cover.absolutePath}.missing")
            awaitArtwork(4) { it.sameAs(fallback) }

            update(5, 5, cover.absolutePath)
            awaitArtwork(5) { it.getPixel(0, 0) == Color.MAGENTA }

            update(6, null)
            awaitMetadata { it == null }
        } finally {
            session.release()
            instrumentation.waitForIdleSync()
            cover.delete()
        }
    }
}
