package com.soundgrammy.app

import android.content.Context
import android.net.Uri
import java.io.File
import java.io.IOException

object NativeFiles {
    @JvmStatic
    fun copyDocument(context: Context, uri: String, path: String, write: Boolean) {
        val resolver = context.contentResolver
        if (write) {
            val output = resolver.openOutputStream(Uri.parse(uri), "wt")
                ?: throw IOException("Cannot open the selected document")
            output.use { stream -> File(path).inputStream().use { it.copyTo(stream) } }
        } else {
            val input = resolver.openInputStream(Uri.parse(uri))
                ?: throw IOException("Cannot read the selected document")
            input.use { stream -> File(path).outputStream().use { stream.copyTo(it) } }
        }
    }
}
