package ru.ven4tools.docking;

import android.app.Activity;
import android.app.PendingIntent;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageInstaller;
import android.net.Uri;
import android.os.Build;
import android.provider.Settings;

import org.json.JSONObject;

import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;

/**
 * Launcher self-update: the agent publishes /apk/latest.json {version, versionCode, url, sha256};
 * a newer versionCode shows the "update available" plate, a tap downloads the APK (same origin only),
 * checks SHA-256 and installs it through a PackageInstaller session.
 */
final class Updater {
    static final class Latest {
        final String version, url, sha256;
        final int versionCode;
        Latest(String version, int versionCode, String url, String sha256) {
            this.version = version; this.versionCode = versionCode; this.url = url; this.sha256 = sha256;
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
        return new Latest(j.optString("version", "?"), code, j.optString("url", "/apk/ven4docking.apk"), j.optString("sha256", ""));
    }

    /** Resolves a relative APK url against the agent; refuses other hosts. */
    static String resolve(String baseUrl, String url) {
        if (url == null || url.isEmpty()) return null;
        if (url.startsWith("/")) return baseUrl + url.substring(1);
        if (Prefs.origin(url).equals(Prefs.origin(baseUrl))) return url;
        return null;
    }

    /** Android 8+: the user must allow "install unknown apps" for us once; 7: global "unknown sources". */
    @SuppressWarnings("deprecation")
    static boolean canInstall(Activity a) {
        if (Build.VERSION.SDK_INT >= 26) {
            if (a.getPackageManager().canRequestPackageInstalls()) return true;
            a.startActivity(new Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:" + a.getPackageName())));
            return false;
        }
        try {
            if (Settings.Secure.getInt(a.getContentResolver(), Settings.Secure.INSTALL_NON_MARKET_APPS) == 1) return true;
        } catch (Settings.SettingNotFoundException ignored) {
            return true; // let the system installer decide
        }
        a.startActivity(new Intent(Settings.ACTION_SECURITY_SETTINGS));
        return false;
    }

    /** Blocking: download + verify + commit a session. The result arrives in InstallReceiver. */
    static void install(Context c, String baseUrl, String token, String url, String sha256) throws IOException {
        String full = resolve(baseUrl, url);
        if (full == null) throw new IOException("APK не с этого ПК");
        File apk = new File(c.getCacheDir(), "update.apk");
        String got = Http.download(full, token, apk);
        if (sha256 != null && !sha256.isEmpty() && !sha256.equalsIgnoreCase(got)) {
            apk.delete();
            throw new IOException("контрольная сумма не совпала");
        }
        PackageInstaller pi = c.getPackageManager().getPackageInstaller();
        PackageInstaller.SessionParams params = new PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL);
        params.setAppPackageName(c.getPackageName());
        int id = pi.createSession(params);
        PackageInstaller.Session s = pi.openSession(id);
        try {
            OutputStream out = s.openWrite("base.apk", 0, apk.length());
            InputStream in = new FileInputStream(apk);
            try {
                byte[] buf = new byte[65536];
                int n;
                while ((n = in.read(buf)) > 0) out.write(buf, 0, n);
                s.fsync(out);
            } finally {
                in.close();
                out.close();
            }
            int flags = PendingIntent.FLAG_UPDATE_CURRENT;
            // the installer fills in the status extras, so the PendingIntent must be mutable (Android 12+)
            if (Build.VERSION.SDK_INT >= 31) flags |= PendingIntent.FLAG_MUTABLE;
            PendingIntent done = PendingIntent.getBroadcast(c, id, new Intent(c, InstallReceiver.class), flags);
            s.commit(done.getIntentSender());
        } catch (IOException | RuntimeException e) {
            s.abandon();
            throw e;
        } finally {
            s.close();
            apk.delete();
        }
    }
}
