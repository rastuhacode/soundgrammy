package com.soundgrammy.app

import android.os.Build
import android.provider.MediaStore
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File

@RunWith(AndroidJUnit4::class)
class AndroidDownloadsTest {
    @Test
    fun savesTrackInPublicDownloads() {
        assumeTrue(Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q)
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val source = File.createTempFile("soundgrammy-export-", ".mp3", context.cacheDir)
        val bytes = byteArrayOf(1, 2, 3, 4)
        source.writeBytes(bytes)
        val uri = android.net.Uri.parse(
            AndroidDownloads.save(context, source.absolutePath, "Regression track.mp3", "audio/mpeg")
        )
        try {
            val resolver = context.contentResolver
            assertEquals("content", uri.scheme)
            assertArrayEquals(bytes, resolver.openInputStream(uri)!!.use { it.readBytes() })
            resolver.query(uri, arrayOf(MediaStore.MediaColumns.RELATIVE_PATH), null, null, null)!!.use {
                it.moveToFirst()
                assertEquals("Download/SoundGrammy/", it.getString(0))
            }
        } finally {
            context.contentResolver.delete(uri, null, null)
            source.delete()
        }
    }
}
