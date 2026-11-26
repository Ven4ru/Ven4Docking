package ru.ven4tools.docking;

import org.json.JSONObject;

/**
 * "A newer launcher is available" check: the agent publishes /apk/latest.json {version, versionCode, ...}.
 * The launcher only reads it and tells the user; it never downloads or installs anything itself —
 * the editor on the PC installs the new APK over adb (USB or wireless debugging), like the setup wizard.
 */
final class Updater {
    static final class Latest {
        final String version;
        final int versionCode;
        Latest(String version, int versionCode) {
            this.version = version; this.versionCode = versionCode;
        }
    }

    private Updater() { }

    /** Blocking. Returns the newer version or null. */
    static Latest check(String baseUrl, String token) throws Exception {
        Http.Response r = Http.get(baseUrl + "apk/latest.json", token);
        if (!r.ok()) return null;
        JSONObject j = new JSONObject(r.body);
        int code = j.optInt("versionCode", 0);
        if (code <= BuildConfig.VERSION_CODE) return null;
        return new Latest(j.optString("version", "?"), code);
    }
}
