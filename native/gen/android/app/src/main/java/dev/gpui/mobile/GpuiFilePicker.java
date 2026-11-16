package dev.gpui.mobile;

import android.app.Activity;
import android.content.Intent;
import android.net.Uri;
import android.provider.DocumentsContract;

import java.util.ArrayList;
import java.util.concurrent.CountDownLatch;

/**
 * File picker helper using the Storage Access Framework.
 *
 * <p>All public methods are static and called from Rust via JNI.
 * They block the calling thread until the user completes or cancels the picker.</p>
 */
public final class GpuiFilePicker {

    /**
     * Open a single file picker.
     *
     * @param activity The current Activity.
     * @param mimeTypes Pipe-separated MIME types (e.g. "image/jpeg|image/png") or "*\/*" for all.
     * @return The selected file URI as a string, or null if cancelled.
     */
    public static String openFile(final Activity activity, final String mimeTypes) {
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        applyMimeTypes(intent, mimeTypes);

        ArrayList<String> result = launchPicker(activity, intent);
        if (result != null && !result.isEmpty()) {
            return result.get(0);
        }
        return null;
    }

    /**
     * Open a multi-file picker.
     *
     * @param activity The current Activity.
     * @param mimeTypes Pipe-separated MIME types.
     * @return Array of selected file URIs, or null if cancelled.
     */
    public static String[] openFiles(final Activity activity, final String mimeTypes) {
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
        applyMimeTypes(intent, mimeTypes);

        ArrayList<String> result = launchPicker(activity, intent);
        if (result != null) {
            return result.toArray(new String[0]);
        }
        return null;
    }

    /**
     * Open a save-file dialog (ACTION_CREATE_DOCUMENT).
     *
     * @param activity The current Activity.
     * @param mimeType MIME type for the file to create.
     * @param suggestedName Suggested file name.
     * @return The chosen save URI as a string, or null if cancelled.
     */
    public static String getSavePath(final Activity activity, final String mimeType, final String suggestedName) {
        Intent intent = new Intent(Intent.ACTION_CREATE_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.setType(mimeType != null ? mimeType : "*/*");
        if (suggestedName != null) {
            intent.putExtra(Intent.EXTRA_TITLE, suggestedName);
        }

        ArrayList<String> result = launchPicker(activity, intent);
        if (result != null && !result.isEmpty()) {
            return result.get(0);
        }
        return null;
    }

    /**
     * Open a directory picker (ACTION_OPEN_DOCUMENT_TREE).
     *
     * @param activity The current Activity.
     * @return The chosen directory URI as a string, or null if cancelled.
     */
    public static String getDirectoryPath(final Activity activity) {
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);

        ArrayList<String> result = launchPicker(activity, intent);
        if (result != null && !result.isEmpty()) {
            return result.get(0);
        }
        return null;
    }

    // ── Internal ─────────────────────────────────────────────────────────

    private static ArrayList<String> launchPicker(Activity activity, Intent intent) {
        CountDownLatch latch = new CountDownLatch(1);
        GpuiPickerActivity.sLatch = latch;
        GpuiPickerActivity.sResult.set(null);
        GpuiPickerActivity.sPendingIntent = intent;

        Intent proxy = new Intent(activity, GpuiPickerActivity.class);
        proxy.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        activity.startActivity(proxy);

        try {
            latch.await();
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return null;
        }

        return GpuiPickerActivity.sResult.get();
    }

    private static void applyMimeTypes(Intent intent, String mimeTypes) {
        if (mimeTypes == null || mimeTypes.isEmpty() || mimeTypes.equals("*/*")) {
            intent.setType("*/*");
            return;
        }

        String[] types = mimeTypes.split("\\|");
        if (types.length == 1) {
            intent.setType(types[0]);
        } else {
            intent.setType("*/*");
            intent.putExtra(Intent.EXTRA_MIME_TYPES, types);
        }
    }

    private GpuiFilePicker() {}
}
