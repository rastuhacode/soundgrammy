package com.soundgrammy.app

import org.junit.Assert.assertEquals
import org.junit.Test

class TaskRemovalTest {
    @Test
    fun stopsServiceBeforeEndingProcess() {
        val calls = mutableListOf<String>()

        TaskRemoval.close({ calls += "stop service" }, { calls += "end process" })

        assertEquals(listOf("stop service", "end process"), calls)
    }

    @Test
    fun stillEndsProcessIfServiceCleanupFails() {
        val calls = mutableListOf<String>()

        try {
            TaskRemoval.close({ error("cleanup failed") }, { calls += "end process" })
        } catch (_: IllegalStateException) {
            // A failed service cleanup must not leave the player process alive.
        }

        assertEquals(listOf("end process"), calls)
    }
}
