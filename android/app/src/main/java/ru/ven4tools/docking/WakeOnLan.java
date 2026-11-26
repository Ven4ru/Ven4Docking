package ru.ven4tools.docking;

import java.io.IOException;
import java.net.DatagramPacket;
import java.net.DatagramSocket;
import java.net.InetAddress;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;

/**
 * Wake on LAN: the magic packet (6 × FF + 16 × MAC) to UDP 9 at the PC's broadcast address and at
 * 255.255.255.255, three rounds 300 ms apart. MAC and broadcast come from the agent (layout.wol)
 * while the PC is on. Plain Java: covered by the JVM test WakeOnLanTest.
 */
final class WakeOnLan {
    static final int PORT = 9;
    static final int ROUNDS = 3;
    static final long PAUSE_MS = 300;
    static final String GLOBAL = "255.255.255.255";

    interface Sender { void send(byte[] data, byte[] ipv4, int port) throws IOException; }

    interface Sleeper { void sleep(long ms) throws InterruptedException; }

    private WakeOnLan() { }

    /** "AA:BB:CC:DD:EE:FF", "aa-bb-cc-dd-ee-ff" or "AABBCCDDEEFF" → 6 bytes; null when malformed. */
    static byte[] parseMac(String s) {
        if (s == null) return null;
        String hex = s.trim().replace(":", "").replace("-", "");
        if (hex.length() != 12) return null;
        byte[] mac = new byte[6];
        for (int i = 0; i < 6; i++) {
            int hi = Character.digit(hex.charAt(i * 2), 16), lo = Character.digit(hex.charAt(i * 2 + 1), 16);
            if (hi < 0 || lo < 0) return null;
            mac[i] = (byte) (hi << 4 | lo);
        }
        boolean zero = true, all = true;
        for (byte b : mac) { zero &= b == 0; all &= b == (byte) 0xFF; }
        return zero || all ? null : mac;
    }

    /** The canonical "AA:BB:CC:DD:EE:FF", or null when malformed. */
    static String normalizeMac(String s) {
        byte[] mac = parseMac(s);
        if (mac == null) return null;
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < mac.length; i++) {
            if (i > 0) sb.append(':');
            sb.append(String.format(Locale.ROOT, "%02X", mac[i] & 0xFF));
        }
        return sb.toString();
    }

    /** Dotted IPv4 literal → 4 bytes (no DNS lookups); null otherwise. */
    static byte[] parseIpv4(String s) {
        if (s == null) return null;
        String[] parts = s.trim().split("\\.", -1);
        if (parts.length != 4) return null;
        byte[] ip = new byte[4];
        for (int i = 0; i < 4; i++) {
            if (parts[i].isEmpty() || parts[i].length() > 3) return null;
            int v;
            try { v = Integer.parseInt(parts[i]); } catch (NumberFormatException e) { return null; }
            if (v < 0 || v > 255) return null;
            ip[i] = (byte) v;
        }
        return ip;
    }

    /** 102 bytes: 6 × 0xFF, then the MAC 16 times. */
    static byte[] packet(byte[] mac) {
        byte[] p = new byte[6 + 16 * 6];
        for (int i = 0; i < 6; i++) p[i] = (byte) 0xFF;
        for (int i = 0; i < 16; i++) System.arraycopy(mac, 0, p, 6 + i * 6, 6);
        return p;
    }

    /** Where the packet goes: the PC's broadcast address (if it is a usable IPv4 literal) and 255.255.255.255. */
    static List<byte[]> targets(String broadcast) {
        List<byte[]> out = new ArrayList<>();
        byte[] b = parseIpv4(broadcast);
        byte[] global = parseIpv4(GLOBAL);
        if (b != null && !(b[0] == 0 && b[1] == 0 && b[2] == 0 && b[3] == 0) && !Arrays.equals(b, global)) out.add(b);
        out.add(global);
        return out;
    }

    /**
     * Sends the packet ROUNDS times to every target with PAUSE_MS between rounds.
     * Returns how many datagrams went out (0 when the MAC is malformed).
     */
    static int send(String mac, String broadcast, Sender sender, Sleeper sleeper) {
        byte[] m = parseMac(mac);
        if (m == null) return 0;
        byte[] data = packet(m);
        List<byte[]> to = targets(broadcast);
        int sent = 0;
        for (int round = 0; round < ROUNDS; round++) {
            if (round > 0) {
                try { sleeper.sleep(PAUSE_MS); } catch (InterruptedException e) { Thread.currentThread().interrupt(); return sent; }
            }
            for (byte[] ip : to) {
                try { sender.send(data, ip, PORT); sent++; } catch (IOException ignored) { /* next target */ }
            }
        }
        return sent;
    }

    /** Real network. Blocking (about 0.6 s) — call from a background thread. */
    static int wake(String mac, String broadcast) {
        DatagramSocket s = null;
        try {
            s = new DatagramSocket();
            s.setBroadcast(true);
            final DatagramSocket sock = s;
            return send(mac, broadcast,
                    (data, ip, port) -> sock.send(new DatagramPacket(data, data.length, InetAddress.getByAddress(ip), port)),
                    Thread::sleep);
        } catch (IOException e) {
            return 0;
        } finally {
            if (s != null) s.close();
        }
    }
}
