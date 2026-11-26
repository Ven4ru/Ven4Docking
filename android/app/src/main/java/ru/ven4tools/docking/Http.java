package ru.ven4tools.docking;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;

/** Minimal blocking HTTP client for background threads. The token goes only into the header. */
final class Http {
    static final int TIMEOUT_MS = 5000;

    static final class Response {
        final int code;
        final String body;
        Response(int code, String body) { this.code = code; this.body = body; }
        boolean ok() { return code >= 200 && code < 300; }
    }

    private Http() { }

    private static HttpURLConnection open(String url, String token) throws IOException {
        HttpURLConnection c = (HttpURLConnection) new URL(url).openConnection();
        c.setConnectTimeout(TIMEOUT_MS);
        c.setReadTimeout(TIMEOUT_MS);
        c.setUseCaches(false);
        c.setInstanceFollowRedirects(false);
        c.setRequestProperty("Accept", "application/json");
        if (token != null && !token.isEmpty()) c.setRequestProperty("Authorization", "Bearer " + token);
        return c;
    }

    static Response get(String url, String token) throws IOException {
        HttpURLConnection c = open(url, token);
        try {
            return read(c);
        } finally {
            c.disconnect();
        }
    }

    static Response postJson(String url, String token, String json) throws IOException {
        HttpURLConnection c = open(url, token);
        try {
            byte[] body = json.getBytes(StandardCharsets.UTF_8);
            c.setRequestMethod("POST");
            c.setDoOutput(true);
            c.setRequestProperty("Content-Type", "application/json; charset=utf-8");
            c.setFixedLengthStreamingMode(body.length);
            OutputStream os = c.getOutputStream();
            try { os.write(body); } finally { os.close(); }
            return read(c);
        } finally {
            c.disconnect();
        }
    }

    private static Response read(HttpURLConnection c) throws IOException {
        int code = c.getResponseCode();
        InputStream in = code >= 400 ? c.getErrorStream() : c.getInputStream();
        String body = "";
        if (in != null) {
            try {
                ByteArrayOutputStream bo = new ByteArrayOutputStream();
                byte[] buf = new byte[8192];
                int n;
                while ((n = in.read(buf)) > 0 && bo.size() < 1 << 20) bo.write(buf, 0, n);
                body = new String(bo.toByteArray(), StandardCharsets.UTF_8);
            } finally {
                in.close();
            }
        }
        return new Response(code, body);
    }

    static String hex(byte[] b) {
        StringBuilder sb = new StringBuilder(b.length * 2);
        for (byte x : b) sb.append(String.format("%02x", x & 0xff));
        return sb.toString();
    }
}
