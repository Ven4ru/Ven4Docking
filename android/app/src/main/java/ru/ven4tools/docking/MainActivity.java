package ru.ven4tools.docking;

import android.app.Activity;
import android.app.AlertDialog;
import android.content.Context;
import android.content.Intent;
import android.graphics.Color;
import android.graphics.drawable.GradientDrawable;
import android.net.ConnectivityManager;
import android.net.Network;
import android.os.Build;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.provider.Settings;
import android.view.Gravity;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowManager;
import android.view.inputmethod.InputMethodManager;
import android.webkit.JavascriptInterface;
import android.webkit.RenderProcessGoneDetail;
import android.webkit.WebChromeClient;
import android.webkit.WebResourceError;
import android.webkit.WebResourceRequest;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.FrameLayout;
import android.widget.TextView;
import android.widget.Toast;

import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/**
 * Ven4Docking launcher: a HOME activity that shows the PC panel full screen.
 * Not paired → pairing screen. Paired → WebView with the agent's panel; the page gets the token
 * through the DockAndroid bridge (never in the URL).
 * Provisioning from the PC over adb (editor wizard):
 *   adb shell am start -n ru.ven4tools.docking/.MainActivity --es url http://PC:8765/ --es token TOKEN
 * Escape hatch: 5 quick taps in the top-left corner.
 */
public class MainActivity extends Activity {
    private static final long RETRY_MS = 5000;
    private static final long REPORT_MS = 60000;
    private static final long UPDATE_CHECK_MS = 6 * 3600 * 1000L;

    private final Handler ui = new Handler(Looper.getMainLooper());
    private final ExecutorService io = Executors.newCachedThreadPool();
    private Prefs prefs;
    private FrameLayout root;
    private WebView web;
    private TextView status;
    private TextView updatePlate;
    private PairingScreen pairing;
    private boolean failed;
    private int cornerTaps;
    private long firstTapAt;
    private ConnectivityManager.NetworkCallback netCb;
    /** Origin of the page currently shown; the bridge hands out the token only to the paired agent. */
    private volatile String pageOrigin = "";
    private volatile Updater.Latest latest;

    private final Runnable retry = () -> { if (failed) load(); };
    private final Runnable rehide = () -> { if (hasWindowFocus()) Screen.immersive(this); };
    private final Runnable report = new Runnable() {
        @Override public void run() { sendReport(); ui.postDelayed(this, REPORT_MS); }
    };
    private final Runnable updateCheck = new Runnable() {
        @Override public void run() { checkUpdate(false); ui.postDelayed(this, UPDATE_CHECK_MS); }
    };

    @Override
    @SuppressWarnings("deprecation")
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        prefs = new Prefs(this);
        Screen.keepOn(this);
        DeviceInfo.webView(this); // cache the WebView version on the UI thread
        if (BuildConfig.DEBUG) WebView.setWebContentsDebuggingEnabled(true);

        root = new FrameLayout(this);
        root.setBackgroundColor(PairingScreen.BG);
        status = new TextView(this);
        status.setTextColor(PairingScreen.MUTE);
        status.setTextSize(16);
        status.setGravity(Gravity.CENTER);
        status.setPadding(48, 48, 48, 48);
        status.setBackgroundColor(PairingScreen.BG);
        status.setVisibility(View.GONE);
        // small pill at the bottom edge next to the page dots: sits in the gap below the blocks
        updatePlate = new TextView(this);
        updatePlate.setTextColor(Color.rgb(11, 13, 18));
        GradientDrawable pill = new GradientDrawable();
        pill.setColor(PairingScreen.ACC);
        pill.setCornerRadius(dp(10));
        updatePlate.setBackground(pill);
        updatePlate.setTextSize(10);
        updatePlate.setIncludeFontPadding(false);
        updatePlate.setPadding(dp(9), dp(2), dp(9), dp(2));
        updatePlate.setVisibility(View.GONE);
        updatePlate.setOnClickListener(v -> installUpdate(latest));
        setContentView(root);
        // the navigation bar may come back (keyboard, dialogs, an edge swipe): hide it again
        getWindow().getDecorView().setOnSystemUiVisibilityChangeListener(vis -> {
            if ((vis & View.SYSTEM_UI_FLAG_HIDE_NAVIGATION) == 0) {
                ui.removeCallbacks(rehide);
                ui.postDelayed(rehide, 2500);
            }
        });

        takeProvisioning(getIntent());
        if (prefs.paired()) showPanel(); else showPairing(null);
        watchNetwork();
    }

    // ---------- screens ----------

    private void showPairing(String notice) {
        stopPanel();
        if (pairing == null) pairing = new PairingScreen(this, prefs, io, ui, this::showPanel);
        root.removeAllViews();
        root.addView(pairing.view(), match());
        pairing.showChoose(notice);
    }

    private void showPanel() {
        if (pairing != null) pairing.stop();
        // the address field may have left the keyboard and the navigation bar on screen
        View focus = getCurrentFocus();
        InputMethodManager imm = (InputMethodManager) getSystemService(Context.INPUT_METHOD_SERVICE);
        if (focus != null && imm != null) imm.hideSoftInputFromWindow(focus.getWindowToken(), 0);
        Screen.immersive(this);
        root.removeAllViews();
        createWeb();
        root.addView(status, match());
        FrameLayout.LayoutParams lp = new FrameLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT, Gravity.BOTTOM | Gravity.CENTER_HORIZONTAL);
        lp.setMargins(dp(130), 0, 0, 0); // to the right of the page dots
        root.addView(updatePlate, lp);
        load();
        ui.removeCallbacks(report);
        ui.postDelayed(report, 3000);
        ui.removeCallbacks(updateCheck);
        ui.postDelayed(updateCheck, 5000);
    }

    private void stopPanel() {
        ui.removeCallbacks(report);
        ui.removeCallbacks(updateCheck);
        ui.removeCallbacks(retry);
        if (web != null) {
            root.removeView(web);
            web.destroy();
            web = null;
        }
        pageOrigin = "";
        failed = false;
    }

    private static FrameLayout.LayoutParams match() {
        return new FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT);
    }

    private void createWeb() {
        if (web != null) {
            root.removeView(web);
            web.destroy();
        }
        web = new WebView(this);
        web.setBackgroundColor(PairingScreen.BG);
        WebSettings s = web.getSettings();
        s.setJavaScriptEnabled(true);
        s.setDomStorageEnabled(true);
        s.setMediaPlaybackRequiresUserGesture(false);
        s.setTextZoom(100);
        s.setCacheMode(WebSettings.LOAD_NO_CACHE);
        s.setAllowFileAccess(false);
        s.setAllowContentAccess(false);
        web.addJavascriptInterface(new Bridge(), "DockAndroid");
        web.setWebChromeClient(new WebChromeClient());
        web.setWebViewClient(new WebViewClient() {
            @Override
            public void onPageStarted(WebView v, String url, android.graphics.Bitmap icon) {
                pageOrigin = Prefs.origin(url);
            }

            @Override
            public void onPageFinished(WebView v, String url) {
                pageOrigin = Prefs.origin(url);
                if (!failed) status.setVisibility(View.GONE);
            }

            @Override
            public void onReceivedError(WebView v, WebResourceRequest req, WebResourceError err) {
                if (req.isForMainFrame()) fail("ПК не отвечает (" + err.getDescription() + ")");
            }

            @Override
            public boolean shouldOverrideUrlLoading(WebView v, WebResourceRequest req) {
                // the panel never navigates away from the agent
                return !Prefs.origin(req.getUrl().toString()).equals(Prefs.origin(prefs.baseUrl()));
            }

            @Override
            public boolean onRenderProcessGone(WebView v, RenderProcessGoneDetail d) {
                // Android 8+: renderer crashed or was killed for memory — rebuild instead of dying
                ui.post(() -> { if (prefs.paired()) { createWeb(); load(); } });
                return true;
            }
        });
        root.addView(web, 0, match());
    }

    private void load() {
        ui.removeCallbacks(retry);
        if (web == null) return;
        failed = false;
        web.loadUrl(prefs.baseUrl());
    }

    private void fail(String why) {
        failed = true;
        status.setText(why + "\n\nПовтор каждые 5 секунд…");
        status.setVisibility(View.VISIBLE);
        ui.removeCallbacks(retry);
        ui.postDelayed(retry, RETRY_MS);
    }

    // ---------- provisioning, report, update ----------

    /** url/token extras from the editor's adb wizard. */
    private void takeProvisioning(Intent i) {
        if (i == null) return;
        String base = Prefs.normalizeBase(i.getStringExtra("url"), BuildConfig.AGENT_PORT);
        String token = i.getStringExtra("token");
        if (base != null && token != null && !token.trim().isEmpty()) prefs.savePairing(base, token.trim());
        else if (base != null) prefs.saveBaseUrl(base);
        // don't keep the token in the launch intent
        i.removeExtra("token");
    }

    private void sendReport() {
        final String base = prefs.baseUrl(), token = prefs.token();
        if (base.isEmpty() || token.isEmpty()) return;
        io.execute(() -> {
            try {
                Http.Response r = Http.postJson(base + "api/v1/phone", token, DeviceInfo.report(this).toString());
                if (r.code == 401) ui.post(() -> {
                    // the PC revoked this phone
                    prefs.forgetToken();
                    showPairing("Компьютер отозвал привязку этого телефона. Привяжите заново.");
                });
            } catch (Exception ignored) {
                // PC away: the page shows "ПК не отвечает"
            }
        });
    }

    private void checkUpdate(final boolean verbose) {
        final String base = prefs.baseUrl(), token = prefs.token();
        if (base.isEmpty()) return;
        io.execute(() -> {
            Updater.Latest l = null;
            try { l = Updater.check(base, token); } catch (Exception ignored) { }
            final Updater.Latest found = l;
            ui.post(() -> {
                latest = found;
                if (found != null) {
                    updatePlate.setText("Обновление " + found.version + " · установить");
                    updatePlate.setVisibility(View.VISIBLE);
                } else {
                    updatePlate.setVisibility(View.GONE);
                    if (verbose) Toast.makeText(this, "Установлена последняя версия " + BuildConfig.VERSION_NAME, Toast.LENGTH_SHORT).show();
                }
            });
        });
    }

    private void installUpdate(final Updater.Latest l) {
        if (l == null) return;
        startInstall(l.url, l.sha256);
    }

    private void startInstall(final String url, final String sha256) {
        if (!Updater.canInstall(this)) {
            Toast.makeText(this, "Разрешите Ven4Docking устанавливать приложения и нажмите ещё раз", Toast.LENGTH_LONG).show();
            return;
        }
        Toast.makeText(this, "Скачиваю обновление…", Toast.LENGTH_SHORT).show();
        final String base = prefs.baseUrl(), token = prefs.token();
        io.execute(() -> {
            try {
                Updater.install(this, base, token, url, sha256);
            } catch (Exception e) {
                ui.post(() -> Toast.makeText(this, "Обновление не скачано: " + e.getMessage(), Toast.LENGTH_LONG).show());
            }
        });
    }

    private void watchNetwork() {
        ConnectivityManager cm = (ConnectivityManager) getSystemService(Context.CONNECTIVITY_SERVICE);
        if (cm == null) return;
        netCb = new ConnectivityManager.NetworkCallback() {
            @Override public void onAvailable(Network n) { ui.postDelayed(() -> { if (failed) load(); }, 1500); }
        };
        try { cm.registerDefaultNetworkCallback(netCb); } catch (RuntimeException ignored) { }
    }

    // ---------- activity plumbing ----------

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        String before = prefs.baseUrl() + "|" + prefs.token();
        takeProvisioning(intent);
        if (!before.equals(prefs.baseUrl() + "|" + prefs.token())) {
            if (prefs.paired()) showPanel(); else showPairing(null);
        }
    }

    @Override
    protected void onResume() {
        super.onResume();
        Screen.immersive(this);
        if (web != null) web.onResume();
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus) Screen.immersive(this);
    }

    @Override
    public boolean dispatchTouchEvent(MotionEvent e) {
        if (e.getActionMasked() == MotionEvent.ACTION_DOWN) {
            float corner = 72 * getResources().getDisplayMetrics().density;
            if (e.getX() < corner && e.getY() < corner) {
                long now = System.currentTimeMillis();
                if (now - firstTapAt > 3000) { firstTapAt = now; cornerTaps = 0; }
                if (++cornerTaps >= 5) { cornerTaps = 0; menu(); return true; }
            }
        }
        return super.dispatchTouchEvent(e);
    }

    @Override
    @SuppressWarnings("deprecation")
    public void onBackPressed() {
        // home screen: Back does nothing
    }

    private void menu() {
        String[] items = {"Привязать заново", "Перезагрузить панель", "Проверить обновление",
                "О лаунчере", "Настройки Android", "Выбрать рабочий стол", "Отмена"};
        new AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                .setTitle("Ven4Docking")
                .setItems(items, (d, which) -> {
                    switch (which) {
                        case 0: prefs.forgetToken(); showPairing(null); break;
                        case 1: if (prefs.paired()) showPanel(); break;
                        case 2: checkUpdate(true); break;
                        case 3: about(); break;
                        case 4: startActivity(new Intent(Settings.ACTION_SETTINGS)); break;
                        case 5: startActivity(new Intent(Settings.ACTION_HOME_SETTINGS)); break;
                        default: break;
                    }
                }).show().setOnDismissListener(d -> Screen.immersive(this));
    }

    private int dp(int v) { return Math.round(v * getResources().getDisplayMetrics().density); }

    private void about() {
        String msg = "Лаунчер " + BuildConfig.VERSION_NAME + " (" + BuildConfig.VERSION_CODE + ")"
                + "\nAndroid " + Build.VERSION.RELEASE + " (API " + Build.VERSION.SDK_INT + "), " + Build.MODEL
                + "\nWebView " + DeviceInfo.webView(this)
                + "\nПК: " + (prefs.baseUrl().isEmpty() ? "не задан" : prefs.baseUrl())
                + (prefs.paired() ? " (привязан)" : " (не привязан)");
        new AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
                .setTitle("О лаунчере").setMessage(msg).setPositiveButton("OK", null).show()
                .setOnDismissListener(d -> Screen.immersive(this));
    }

    @Override
    protected void onDestroy() {
        ui.removeCallbacksAndMessages(null);
        if (pairing != null) pairing.stop();
        ConnectivityManager cm = (ConnectivityManager) getSystemService(Context.CONNECTIVITY_SERVICE);
        if (cm != null && netCb != null) {
            try { cm.unregisterNetworkCallback(netCb); } catch (RuntimeException ignored) { }
        }
        if (web != null) web.destroy();
        io.shutdownNow();
        super.onDestroy();
    }

    /** Exposed to the panel page as window.DockAndroid (contract «Мост телефона»). */
    private class Bridge {
        /** 0.0–1.0, or negative to follow the system setting. */
        @JavascriptInterface
        public void setBrightness(final float level) {
            ui.post(() -> {
                WindowManager.LayoutParams lp = getWindow().getAttributes();
                lp.screenBrightness = level < 0 ? WindowManager.LayoutParams.BRIGHTNESS_OVERRIDE_NONE
                        : Math.max(0.01f, Math.min(1f, level));
                getWindow().setAttributes(lp);
            });
        }

        @JavascriptInterface
        public String version() { return BuildConfig.VERSION_NAME; }

        /** {"level","temp","charging"} */
        @JavascriptInterface
        public String battery() { return DeviceInfo.battery(MainActivity.this).toString(); }

        /** {"model","android","webview","deviceId"} */
        @JavascriptInterface
        public String info() { return DeviceInfo.info(MainActivity.this, prefs.deviceId()).toString(); }

        /** Installs the launcher APK from the agent (same origin only). */
        @JavascriptInterface
        public void openUpdate(final String url) {
            ui.post(() -> startInstall(url, latest != null && url != null && url.equals(latest.url) ? latest.sha256 : ""));
        }

        /**
         * Bearer token for the page's API calls (extension of the bridge contract). Answered only
         * while the WebView shows the paired agent, so another origin can never read it.
         */
        @JavascriptInterface
        public String token() {
            String base = prefs.baseUrl();
            if (base.isEmpty() || !pageOrigin.equals(Prefs.origin(base))) return "";
            return prefs.token();
        }
    }
}
