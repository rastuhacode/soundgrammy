package com.soundgrammy.app

import org.junit.Assert.assertEquals
import org.junit.Test

/** Prevent a compiling host from silently dropping composition/soft-keyboard APIs. */
class NativeHostTest {
    @Test fun activityRetainsUpstreamComposingInputConnection() {
        assertEquals(dev.gpui.mobile.GpuiInputActivity::class.java,
            dev.gpui.mobile.GpuiActivity::class.java.superclass)
        assertEquals(dev.gpui.mobile.GpuiInputActivity::class.java,
            MainActivity::class.java.getMethod("gpuiShowKeyboard", Int::class.javaPrimitiveType,
                Long::class.javaPrimitiveType).declaringClass)
        assertEquals(dev.gpui.mobile.GpuiInputActivity::class.java,
            MainActivity::class.java.getMethod("gpuiHideKeyboard", Long::class.javaPrimitiveType).declaringClass)
    }
}
