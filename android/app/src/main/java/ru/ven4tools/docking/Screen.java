package ru.ven4tools.docking;

import android.app.Activity;
import android.os.Build;
import android.view.View;
import android.view.WindowManager;

/** Window flags of a wall panel, with the Android 7–8 / 8.1+ difference handled. */
final class Screen {
    private Screen() { }

    @SuppressWarnings("deprecation")
    static void keepOn(Activity a) {
        a.getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        if (Build.VERSION.SDK_INT >= 27) {
            // Android 8.1+: the window flags below are deprecated in favour of these calls
            a.setShowWhenLocked(true);
            a.setTurnScreenOn(true);
        } else {
            // Android 7.0–8.0 (the prototype called setShowWhenLocked unconditionally — crash on 7)
            a.getWindow().addFlags(WindowManager.LayoutParams.FLAG_SHOW_WHEN_LOCKED
                    | WindowManager.LayoutParams.FLAG_TURN_SCREEN_ON);
        }
    }

    @SuppressWarnings("deprecation")
    static void immersive(Activity a) {
        a.getWindow().getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                | View.SYSTEM_UI_FLAG_FULLSCREEN | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                | View.SYSTEM_UI_FLAG_LAYOUT_STABLE | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION);
    }
}
