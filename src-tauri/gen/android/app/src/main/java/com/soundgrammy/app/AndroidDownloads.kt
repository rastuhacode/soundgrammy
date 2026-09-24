package com.soundgrammy.app

import android.content.ContentValues
import android.content.Context
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import java.io.File
import java.io.IOException

/** Publishes an app-cached audio file in the system Downloads collection. */
object AndroidDownloads {
    @JvmStatic
    fun save(context: Context, sourcePath: String, displayName: String, mimeType: String): String {
        require(displayName == File(displayName).name) { "Invalid download filename" }
        val source = File(sourcePath)
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
            // MediaStore Downloads and RELATIVE_PATH were added in API 29.
            val downloads = context.getExternalFilesDir(Environment.DIRECTORY_DOWNLOADS)
                ?: throw IOException("Downloads folder unavailable")
            val folder = File(downloads, "SoundGrammy")
            if (!folder.isDirectory && !folder.mkdirs()) throw IOException("Cannot create Downloads folder")
            var destination = File(folder, displayName)
            val basename = displayName.substringBeforeLast('.')
            val extension = displayName.substringAfterLast('.')
            var suffix = 2
            while (destination.exists()) {
                destination = File(folder, "$basename ($suffix).$extension")
                suffix++
            }
            source.copyTo(destination)
            return destination.absolutePath
        }

        val resolver = context.contentResolver
        val values = ContentValues().apply {
            put(MediaStore.MediaColumns.DISPLAY_NAME, displayName)
            put(MediaStore.MediaColumns.MIME_TYPE, mimeType)
            put(MediaStore.MediaColumns.RELATIVE_PATH, "${Environment.DIRECTORY_DOWNLOADS}/SoundGrammy/")
            put(MediaStore.MediaColumns.IS_PENDING, 1)
        }
        val uri = resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
            ?: throw IOException("Cannot create file in Downloads")
        try {
            val output = resolver.openOutputStream(uri, "w")
                ?: throw IOException("Cannot open file in Downloads")
            output.use { stream -> source.inputStream().use { it.copyTo(stream) } }
            val ready = ContentValues().apply { put(MediaStore.MediaColumns.IS_PENDING, 0) }
            if (resolver.update(uri, ready, null, null) != 1) {
                throw IOException("Cannot finish file in Downloads")
            }
            return uri.toString()
        } catch (error: Exception) {
            try {
                resolver.delete(uri, null, null)
            } catch (_: Exception) {
                // Preserve the write failure even if cleanup also fails.
            }
            throw error
        }
    }
}
