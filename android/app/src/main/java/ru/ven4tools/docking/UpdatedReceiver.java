package ru.ven4tools.docking;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;

/** After the editor updated the launcher over adb: open the panel again (it is the home screen). */
public class UpdatedReceiver extends BroadcastReceiver {
    @Override
    public void onReceive(Context c, Intent i) {
        if (!Intent.ACTION_MY_PACKAGE_REPLACED.equals(i.getAction())) return;
        Intent start = new Intent(c, MainActivity.class);
        start.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        try { c.startActivity(start); } catch (RuntimeException ignored) { /* background start blocked: HOME brings it back */ }
    }
}
