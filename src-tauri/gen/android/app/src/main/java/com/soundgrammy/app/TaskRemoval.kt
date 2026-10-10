package com.soundgrammy.app

/** Stop foreground playback before ending the process when its task is removed. */
internal object TaskRemoval {
    fun close(stopService: () -> Unit, terminateProcess: () -> Unit) {
        try {
            stopService()
        } finally {
            terminateProcess()
        }
    }
}
