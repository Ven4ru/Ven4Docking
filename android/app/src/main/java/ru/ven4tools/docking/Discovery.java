package ru.ven4tools.docking;

import org.json.JSONObject;

import java.net.DatagramPacket;
import java.net.DatagramSocket;
import java.net.InetAddress;
import java.net.InterfaceAddress;
import java.net.NetworkInterface;
import java.net.SocketTimeoutException;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Finds agents in the local network: broadcast "V4D?1" to UDP {@link BuildConfig#DISCOVERY_PORT},
 * agents answer {"name","port","ver"}. Blocking — call from a background thread.
 * Note: the Android emulator's NAT does not pass broadcasts to the host; use the manual address there.
 */
final class Discovery {
    static final class Found {
        final String name, host, ver;
        final int port;
        Found(String name, String host, int port, String ver) { this.name = name; this.host = host; this.port = port; this.ver = ver; }
        String baseUrl() { return "http://" + host + ":" + port + "/"; }
    }

    private Discovery() { }

    static List<Found> scan(int udpPort, int waitMs) {
        Map<String, Found> found = new LinkedHashMap<>();
        DatagramSocket s = null;
        try {
            s = new DatagramSocket();
            s.setBroadcast(true);
            byte[] q = "V4D?1".getBytes(StandardCharsets.US_ASCII);
            for (InetAddress b : broadcastAddresses()) {
                try { s.send(new DatagramPacket(q, q.length, b, udpPort)); } catch (Exception ignored) { }
            }
            long end = System.currentTimeMillis() + waitMs;
            byte[] buf = new byte[1024];
            while (true) {
                long left = end - System.currentTimeMillis();
                if (left <= 0) break;
                s.setSoTimeout((int) left);
                DatagramPacket p = new DatagramPacket(buf, buf.length);
                try {
                    s.receive(p);
                } catch (SocketTimeoutException e) {
                    break;
                }
                try {
                    JSONObject j = new JSONObject(new String(p.getData(), 0, p.getLength(), StandardCharsets.UTF_8));
                    String host = p.getAddress().getHostAddress();
                    int port = j.optInt("port", BuildConfig.AGENT_PORT);
                    found.put(host + ":" + port, new Found(j.optString("name", host), host, port, j.optString("ver", "")));
                } catch (Exception ignored) {
                    // not our answer
                }
            }
        } catch (Exception ignored) {
            // no network: empty list
        } finally {
            if (s != null) s.close();
        }
        return new ArrayList<>(found.values());
    }

    private static List<InetAddress> broadcastAddresses() {
        List<InetAddress> out = new ArrayList<>();
        try {
            for (NetworkInterface ni : Collections.list(NetworkInterface.getNetworkInterfaces())) {
                if (!ni.isUp() || ni.isLoopback()) continue;
                for (InterfaceAddress ia : ni.getInterfaceAddresses()) {
                    if (ia.getBroadcast() != null && !out.contains(ia.getBroadcast())) out.add(ia.getBroadcast());
                }
            }
        } catch (Exception ignored) { }
        try { out.add(InetAddress.getByName("255.255.255.255")); } catch (Exception ignored) { }
        return out;
    }
}
