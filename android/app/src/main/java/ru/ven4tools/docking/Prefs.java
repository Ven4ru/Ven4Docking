package ru.ven4tools.docking;

import android.content.Context;
import android.content.SharedPreferences;

import java.util.UUID;

/** Pairing data. MODE_PRIVATE: only this app can read the token; it never goes into a URL. */
final class Prefs {
    private static final String FILE = "docking";
    private final SharedPreferences p;

    Prefs(Context c) { p = c.getSharedPreferences(FILE, Context.MODE_PRIVATE); }

    /** Agent base URL, e.g. "http://192.168.1.10:8765/" (no token, no path). */
    String baseUrl() { return p.getString("baseUrl", ""); }

    String token() { return p.getString("token", ""); }

    boolean paired() { return !baseUrl().isEmpty() && !token().isEmpty(); }

    void savePairing(String baseUrl, String token) {
        p.edit().putString("baseUrl", baseUrl).putString("token", token).apply();
    }

    void saveBaseUrl(String baseUrl) { p.edit().putString("baseUrl", baseUrl).apply(); }

    void forgetToken() { p.edit().remove("token").apply(); }

    /** The PC address as the user typed it on the pairing screen; kept so an expired code is one tap to renew. */
    String typedAddress() { return p.getString("typedAddress", ""); }

    void saveTypedAddress(String s) { p.edit().putString("typedAddress", s == null ? "" : s.trim()).apply(); }

    /** PC's MAC for Wake on LAN ("AA:BB:CC:DD:EE:FF", from layout.wol), "" when unknown. */
    String wakeMac() { return p.getString("wakeMac", ""); }

    String wakeBroadcast() { return p.getString("wakeBroadcast", ""); }

    boolean canWake() { return WakeOnLan.parseMac(wakeMac()) != null; }

    /** Keeps a valid MAC (normalized) and the broadcast address; a malformed MAC changes nothing. */
    boolean saveWake(String mac, String broadcast) {
        String m = WakeOnLan.normalizeMac(mac);
        if (m == null) return false;
        String b = WakeOnLan.parseIpv4(broadcast) != null ? broadcast.trim() : "";
        if (m.equals(wakeMac()) && b.equals(wakeBroadcast())) return true;
        p.edit().putString("wakeMac", m).putString("wakeBroadcast", b).apply();
        return true;
    }

    /** Random id of this phone for /pair/start (not ANDROID_ID: nothing to track across apps). */
    String deviceId() {
        String id = p.getString("deviceId", "");
        if (id.isEmpty()) {
            id = UUID.randomUUID().toString();
            p.edit().putString("deviceId", id).apply();
        }
        return id;
    }

    /**
     * "192.168.1.10", "192.168.1.10:8765", "http://host:port/anything" → "http://host:port/".
     * Returns null when the input is not a usable address.
     */
    static String normalizeBase(String in, int defaultPort) {
        if (in == null) return null;
        String s = in.trim();
        if (s.isEmpty()) return null;
        String scheme = "http://";
        if (s.startsWith("http://")) s = s.substring(7);
        else if (s.startsWith("https://")) { s = s.substring(8); scheme = "https://"; }
        int slash = s.indexOf('/');
        if (slash >= 0) s = s.substring(0, slash);
        int hash = s.indexOf('#');
        if (hash >= 0) s = s.substring(0, hash);
        if (s.isEmpty() || s.contains("@") || s.contains(" ")) return null;
        if (s.indexOf(':') < 0) s = s + ":" + defaultPort;
        int colon = s.lastIndexOf(':');
        try {
            int port = Integer.parseInt(s.substring(colon + 1));
            if (port <= 0 || port > 65535) return null;
        } catch (NumberFormatException e) {
            return null;
        }
        if (colon == 0) return null;
        return scheme + s + "/";
    }

    /** "http://host:port/" → "http://host:port" (origin as WebView reports it). */
    static String origin(String url) {
        if (url == null) return "";
        int start = url.indexOf("://");
        if (start < 0) return "";
        int end = url.indexOf('/', start + 3);
        String o = (end < 0 ? url : url.substring(0, end)).toLowerCase(java.util.Locale.ROOT);
        if (o.startsWith("http://") && o.endsWith(":80")) o = o.substring(0, o.length() - 3);
        if (o.startsWith("https://") && o.endsWith(":443")) o = o.substring(0, o.length() - 4);
        return o;
    }
}
