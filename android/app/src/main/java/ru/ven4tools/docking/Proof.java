package ru.ven4tools.docking;

import java.nio.charset.StandardCharsets;
import java.security.GeneralSecurityException;
import java.security.MessageDigest;
import java.security.SecureRandom;
import java.util.List;
import java.util.Locale;

import javax.crypto.Mac;
import javax.crypto.spec.SecretKeySpec;

/**
 * Proof that a host is the paired PC, without sending the token (agent POST /api/v1/hello).
 * The agent keeps only sha256hex(token) and answers HMAC-SHA256(key = that hex text, message = nonce)
 * for every paired phone; the phone computes the same value and looks for it in the list.
 * Plain Java: covered by the JVM test ProofTest.
 */
final class Proof {
    static final int NONCE_BYTES = 32;

    private Proof() { }

    /** 64 hex characters from SecureRandom. */
    static String newNonce(SecureRandom rnd) {
        byte[] b = new byte[NONCE_BYTES];
        rnd.nextBytes(b);
        return Http.hex(b);
    }

    /** sha256hex(token), as the agent stores it (tokenHash). */
    static String tokenHash(String token) {
        try {
            return Http.hex(MessageDigest.getInstance("SHA-256").digest(token.getBytes(StandardCharsets.UTF_8)));
        } catch (GeneralSecurityException e) {
            throw new IllegalStateException(e);
        }
    }

    /** hex(HMAC-SHA256(key = tokenHash(token), message = nonce)). */
    static String expected(String token, String nonce) {
        try {
            Mac mac = Mac.getInstance("HmacSHA256");
            mac.init(new SecretKeySpec(tokenHash(token).getBytes(StandardCharsets.US_ASCII), "HmacSHA256"));
            return Http.hex(mac.doFinal(nonce.getBytes(StandardCharsets.US_ASCII)));
        } catch (GeneralSecurityException e) {
            throw new IllegalStateException(e);
        }
    }

    /** Is expected among proofs? Every entry is compared in constant time, no early exit. */
    static boolean contains(List<String> proofs, String expected) {
        if (proofs == null || expected == null || expected.isEmpty()) return false;
        byte[] want = expected.toLowerCase(Locale.ROOT).getBytes(StandardCharsets.US_ASCII);
        boolean hit = false;
        for (String p : proofs) {
            if (p == null) continue;
            hit |= MessageDigest.isEqual(want, p.toLowerCase(Locale.ROOT).getBytes(StandardCharsets.US_ASCII));
        }
        return hit;
    }
}
