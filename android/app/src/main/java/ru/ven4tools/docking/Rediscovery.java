package ru.ven4tools.docking;

import android.util.Log;

import org.json.JSONArray;
import org.json.JSONObject;

import java.security.SecureRandom;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * The paired PC stopped answering at its saved address (it may have got another IP from DHCP):
 * find agents on the LAN by UDP and accept one only after it proved it knows this phone's token
 * (POST /api/v1/hello with a fresh nonce, see {@link Proof}). The token is never sent to a candidate.
 * Blocking — call from a background thread.
 */
final class Rediscovery {
    static final String TAG = "Ven4Docking";
    static final int SCAN_MS = 2500;
    /** Candidates asked per scan; each hello is bounded by Http.TIMEOUT_MS. */
    static final int MAX_CANDIDATES = 8;
    private static final int MAX_PROOFS = 64;

    /** POST /api/v1/hello to a candidate. Gets no token on purpose. */
    interface Hello {
        List<String> proofs(String base, String nonce) throws Exception;
    }

    private Rediscovery() { }

    /** New base URL of the paired PC, or null when no other host proved itself. */
    static String find(String savedBase, String token) {
        List<String> bases = new ArrayList<>();
        for (Discovery.Found f : Discovery.scan(BuildConfig.DISCOVERY_PORT, SCAN_MS)) bases.add(f.baseUrl());
        String found = pick(savedBase, bases, token, Rediscovery::hello, new SecureRandom());
        Log.i(TAG, "rediscovery: " + bases.size() + " agent(s) answered, " + (found == null ? "no new address" : "moved to " + found));
        return found;
    }

    /**
     * First candidate other than the saved address whose proof list contains this phone's value.
     * The token is used only locally to compute the expected proof.
     */
    static String pick(String savedBase, List<String> candidates, String token, Hello hello, SecureRandom rnd) {
        if (token == null || token.isEmpty() || candidates == null) return null;
        String saved = Prefs.origin(savedBase);
        int asked = 0;
        for (String base : candidates) {
            if (base == null || Prefs.origin(base).isEmpty() || Prefs.origin(base).equals(saved)) continue;
            if (++asked > MAX_CANDIDATES) break;
            String nonce = Proof.newNonce(rnd);
            try {
                if (Proof.contains(hello.proofs(base, nonce), Proof.expected(token, nonce))) return base;
            } catch (Exception ignored) {
                // not an agent, an old agent without /api/v1/hello, or gone: next candidate
            }
        }
        return null;
    }

    private static List<String> hello(String base, String nonce) throws Exception {
        String body = new JSONObject().put("nonce", nonce).toString();
        Http.Response r = Http.postJson(base + "api/v1/hello", null, body);
        if (r.code != 200) return Collections.emptyList();
        JSONArray a = new JSONObject(r.body).optJSONArray("proofs");
        List<String> out = new ArrayList<>();
        if (a == null) return out;
        for (int i = 0; i < a.length() && i < MAX_PROOFS; i++) out.add(a.optString(i, ""));
        return out;
    }
}
