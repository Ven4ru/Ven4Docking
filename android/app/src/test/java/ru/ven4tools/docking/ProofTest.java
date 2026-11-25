package ru.ven4tools.docking;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNotEquals;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

import java.security.SecureRandom;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/** Proof of the paired PC (agent POST /api/v1/hello) and the choice of a new address. */
public class ProofTest {
    private static final String NONCE = "00112233445566778899aabbccddeeff";

    @Test
    public void sameVectorAsTheAgent() {
        // agent/tests/http_tests.rs hello_hmac_vectors checks the same values
        assertEquals("4c5dc9b7708905f77f5e5d16316b5dfb425e68cb326dcd55a860e90a7707031e", Proof.tokenHash("test-token"));
        assertEquals("a19a3f990815c79089877d5bf141489408eb7a458267709a936bcc9301a0e46c", Proof.expected("test-token", NONCE));
    }

    @Test
    public void nonceIsFreshHex() {
        SecureRandom rnd = new SecureRandom();
        String a = Proof.newNonce(rnd), b = Proof.newNonce(rnd);
        assertEquals(64, a.length());
        assertTrue(a.matches("[0-9a-f]{64}"));
        assertNotEquals(a, b);
    }

    @Test
    public void containsComparesWholeList() {
        String want = Proof.expected("test-token", NONCE);
        assertTrue(Proof.contains(Arrays.asList("00", want.toUpperCase(), "ff"), want));
        assertTrue(Proof.contains(Arrays.asList(null, want), want));
        assertFalse(Proof.contains(Arrays.asList(Proof.expected("other-token", NONCE), want.substring(1)), want));
        assertFalse(Proof.contains(Collections.<String>emptyList(), want));
        assertFalse(Proof.contains(null, want));
    }

    /** Fake agents: base → token they know (null = answers with garbage); records what each host received. */
    private static final class Lan implements Rediscovery.Hello {
        final Map<String, String> known = new HashMap<>();
        final List<String> asked = new ArrayList<>();
        final List<String> received = new ArrayList<>();

        @Override
        public List<String> proofs(String base, String nonce) throws Exception {
            asked.add(base);
            received.add(nonce);
            if (!known.containsKey(base)) throw new java.io.IOException("connection refused");
            String token = known.get(base);
            if (token == null) return Collections.singletonList("not a proof");
            return Arrays.asList(Proof.expected("someone-else", nonce), Proof.expected(token, nonce));
        }
    }

    @Test
    public void picksOnlyAHostThatProvedIt() {
        Lan lan = new Lan();
        lan.known.put("http://192.168.1.20:8765/", null);          // answers, but not our PC
        lan.known.put("http://192.168.1.30:8765/", "other-token");  // another user's agent
        lan.known.put("http://192.168.1.40:8765/", "my-token");     // our PC at its new address
        List<String> found = Arrays.asList("http://192.168.1.10:8765/", "http://192.168.1.20:8765/",
                "http://192.168.1.30:8765/", "http://192.168.1.40:8765/");
        String got = Rediscovery.pick("http://192.168.1.10:8765/", found, "my-token", lan, new SecureRandom());
        assertEquals("http://192.168.1.40:8765/", got);
        assertFalse("the saved address is not asked", lan.asked.contains("http://192.168.1.10:8765/"));
        for (String r : lan.received) assertFalse("the token never leaves the phone", r.contains("my-token"));
        assertEquals("a fresh nonce per host", lan.received.size(), new java.util.HashSet<>(lan.received).size());
    }

    @Test
    public void nothingChangesWhenNobodyProves() {
        Lan lan = new Lan();
        lan.known.put("http://192.168.1.30:8765/", "other-token");
        String saved = "http://192.168.1.10:8765/";
        assertNull(Rediscovery.pick(saved, Arrays.asList(saved, "http://192.168.1.30:8765/", "http://192.168.1.50:8765/"),
                "my-token", lan, new SecureRandom()));
        // broadcasts blocked on this network: no candidates at all
        assertNull(Rediscovery.pick(saved, Collections.<String>emptyList(), "my-token", lan, new SecureRandom()));
        // the PC is back at the same address: not a move
        lan.known.put(saved, "my-token");
        assertNull(Rediscovery.pick(saved, Collections.singletonList(saved), "my-token", lan, new SecureRandom()));
        // no token: nothing to prove against
        assertNull(Rediscovery.pick(saved, Collections.singletonList("http://192.168.1.30:8765/"), "", lan, new SecureRandom()));
    }

    @Test
    public void asksAtMostMaxCandidates() {
        Lan lan = new Lan();
        List<String> many = new ArrayList<>();
        for (int i = 0; i < 20; i++) many.add("http://10.0.0." + (i + 1) + ":8765/");
        assertNull(Rediscovery.pick("http://10.0.0.100:8765/", many, "my-token", lan, new SecureRandom()));
        assertEquals(Rediscovery.MAX_CANDIDATES, lan.asked.size());
    }
}
