package ru.ven4tools.docking;

import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.os.BatteryManager;
import android.os.Build;
import android.webkit.WebSettings;

import org.json.JSONException;
import org.json.JSONObject;

import java.util.regex.Matcher;
import java.util.regex.Pattern;

/** What the phone tells about itself: bridge battery()/info() and POST /api/v1/phone. */
final class DeviceInfo {
    private static String webViewVersion;

    private DeviceInfo() { }

    /**
     * WebView (Chromium) version. WebView.getCurrentWebViewPackage() exists only on Android 8+,
     * the default user agent works on 7 too: "... Chrome/52.0.2743.100 Mobile ...".
     */
    static synchronized String webView(Context c) {
        if (webViewVersion == null) {
            String v = "";
            try {
                Matcher m = Pattern.compile("Chrome/([\\d.]+)").matcher(WebSettings.getDefaultUserAgent(c));
                if (m.find()) v = m.group(1);
            } catch (RuntimeException ignored) { }
            webViewVersion = v;
        }
        return webViewVersion;
    }

    static JSONObject battery(Context c) {
        JSONObject j = new JSONObject();
        try {
            Intent b = c.getApplicationContext().registerReceiver(null, new IntentFilter(Intent.ACTION_BATTERY_CHANGED));
            if (b == null) return j;
            int level = b.getIntExtra(BatteryManager.EXTRA_LEVEL, -1);
            int scale = b.getIntExtra(BatteryManager.EXTRA_SCALE, 100);
            int temp = b.getIntExtra(BatteryManager.EXTRA_TEMPERATURE, Integer.MIN_VALUE);
            int status = b.getIntExtra(BatteryManager.EXTRA_STATUS, -1);
            int plugged = b.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0);
            j.put("level", level < 0 || scale <= 0 ? JSONObject.NULL : Math.round(level * 100f / scale));
            j.put("temp", temp == Integer.MIN_VALUE ? JSONObject.NULL : temp / 10.0);
            j.put("charging", plugged != 0 || status == BatteryManager.BATTERY_STATUS_CHARGING || status == BatteryManager.BATTERY_STATUS_FULL);
        } catch (JSONException | RuntimeException ignored) { }
        return j;
    }

    static JSONObject info(Context c, String deviceId) {
        JSONObject j = new JSONObject();
        try {
            j.put("model", Build.MODEL);
            j.put("android", Build.VERSION.RELEASE);
            j.put("webview", webView(c));
            j.put("deviceId", deviceId);
        } catch (JSONException ignored) { }
        return j;
    }

    /** Body of POST /api/v1/phone. */
    static JSONObject report(Context c) {
        JSONObject b = battery(c);
        JSONObject j = new JSONObject();
        try {
            j.put("battery", b.opt("level") == null ? JSONObject.NULL : b.opt("level"));
            j.put("temp", b.opt("temp") == null ? JSONObject.NULL : b.opt("temp"));
            j.put("charging", b.optBoolean("charging", false));
            j.put("webview", webView(c));
            j.put("android", Build.VERSION.RELEASE);
            j.put("model", Build.MODEL);
            j.put("launcherVersion", BuildConfig.VERSION_NAME);
        } catch (JSONException ignored) { }
        return j;
    }
}
