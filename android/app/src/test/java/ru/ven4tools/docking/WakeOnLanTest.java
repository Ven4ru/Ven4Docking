package ru.ven4tools.docking;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNull;

import org.junit.Test;

import java.io.IOException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

/** Wake on LAN: MAC parsing, the magic packet, where and how often it is sent. */
public class WakeOnLanTest {
    private static final byte[] MAC = {(byte) 0xAA, (byte) 0xBB, (byte) 0xCC, 0x01, 0x02, 0x03};

    @Test
    public void parsesMacInCommonForms() {
        assertArrayEquals(MAC, WakeOnLan.parseMac("AA:BB:CC:01:02:03"));
        assertArrayEquals(MAC, WakeOnLan.parseMac("aa-bb-cc-01-02-03"));
        assertArrayEquals(MAC, WakeOnLan.parseMac(" aabbcc010203 "));
        assertEquals("AA:BB:CC:01:02:03", WakeOnLan.normalizeMac("aa-bb-cc-01-02-03"));
    }

    @Test
    public void rejectsBadMac() {
        assertNull(WakeOnLan.parseMac(null));
        assertNull(WakeOnLan.parseMac(""));
        assertNull(WakeOnLan.parseMac("AA:BB:CC:01:02"));
        assertNull(WakeOnLan.parseMac("AA:BB:CC:01:02:03:04"));
        assertNull(WakeOnLan.parseMac("GG:BB:CC:01:02:03"));
        assertNull(WakeOnLan.parseMac("00:00:00:00:00:00"));
        assertNull(WakeOnLan.parseMac("FF:FF:FF:FF:FF:FF"));
        assertNull(WakeOnLan.normalizeMac("not a mac"));
    }

    @Test
    public void packetIsSixFfAndSixteenMacs() {
        byte[] p = WakeOnLan.packet(MAC);
        assertEquals(102, p.length);
        for (int i = 0; i < 6; i++) assertEquals((byte) 0xFF, p[i]);
        for (int i = 0; i < 16; i++) assertArrayEquals(MAC, Arrays.copyOfRange(p, 6 + i * 6, 12 + i * 6));
    }

    @Test
    public void ipv4LiteralsOnly() {
        assertArrayEquals(new byte[]{(byte) 192, (byte) 168, 1, (byte) 255}, WakeOnLan.parseIpv4("192.168.1.255"));
        assertNull(WakeOnLan.parseIpv4("192.168.1"));
        assertNull(WakeOnLan.parseIpv4("192.168.1.256"));
        assertNull(WakeOnLan.parseIpv4("192.168..1"));
        assertNull(WakeOnLan.parseIpv4("pc.local"));
        assertNull(WakeOnLan.parseIpv4(null));
    }

    @Test
    public void targetsAreBroadcastAndGlobal() {
        List<byte[]> t = WakeOnLan.targets("192.168.1.255");
        assertEquals(2, t.size());
        assertArrayEquals(new byte[]{(byte) 192, (byte) 168, 1, (byte) 255}, t.get(0));
        assertArrayEquals(new byte[]{-1, -1, -1, -1}, t.get(1));
        // unknown, malformed or the global address itself: only 255.255.255.255
        assertEquals(1, WakeOnLan.targets("").size());
        assertEquals(1, WakeOnLan.targets("pc.local").size());
        assertEquals(1, WakeOnLan.targets("255.255.255.255").size());
        assertEquals(1, WakeOnLan.targets("0.0.0.0").size());
    }

    @Test
    public void threeRoundsToEveryTargetWithPauses() {
        final List<String> log = new ArrayList<>();
        final List<Long> pauses = new ArrayList<>();
        int n = WakeOnLan.send("AA:BB:CC:01:02:03", "10.0.0.255", (data, ip, port) -> {
            assertEquals(102, data.length);
            assertEquals(9, port);
            log.add((ip[0] & 0xFF) + "." + (ip[1] & 0xFF) + "." + (ip[2] & 0xFF) + "." + (ip[3] & 0xFF));
        }, pauses::add);
        assertEquals(6, n);
        assertEquals(Arrays.asList("10.0.0.255", "255.255.255.255", "10.0.0.255", "255.255.255.255",
                "10.0.0.255", "255.255.255.255"), log);
        assertEquals(Arrays.asList(300L, 300L), pauses);
    }

    @Test
    public void aFailedTargetDoesNotStopTheOthers() {
        int n = WakeOnLan.send("AA:BB:CC:01:02:03", "10.0.0.255", (data, ip, port) -> {
            if (ip[0] == 10) throw new IOException("network unreachable");
        }, ms -> { });
        assertEquals(3, n);
    }

    @Test
    public void badMacSendsNothing() {
        final int[] calls = {0};
        assertEquals(0, WakeOnLan.send("nope", "10.0.0.255", (data, ip, port) -> calls[0]++, ms -> { }));
        assertEquals(0, calls[0]);
    }
}
