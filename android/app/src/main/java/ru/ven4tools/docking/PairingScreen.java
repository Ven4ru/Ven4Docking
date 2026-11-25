package ru.ven4tools.docking;

import android.app.Activity;
import android.graphics.Color;
import android.graphics.Typeface;
import android.os.Build;
import android.os.Handler;
import android.text.Editable;
import android.text.InputType;
import android.text.TextWatcher;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.view.inputmethod.EditorInfo;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;

import org.json.JSONObject;

import java.util.List;
import java.util.concurrent.ExecutorService;

/**
 * Pairing: find the PC (UDP) or type its address → POST /pair/start → show the 4-digit code →
 * poll GET /pair/{id} until the PC accepts. The token is saved to SharedPreferences only.
 */
final class PairingScreen {
    interface Listener { void onPaired(); }

    static final int BG = Color.rgb(11, 13, 18), TEXT = Color.rgb(232, 236, 244), MUTE = Color.rgb(138, 147, 168),
            ACC = Color.rgb(90, 169, 255), WARN = Color.rgb(255, 192, 77), CARD = Color.rgb(20, 24, 35);
    private static final long POLL_MS = 1500, PAIR_TIMEOUT_MS = 150000, SCAN_MS = 2500;

    private final Activity a;
    private final Prefs prefs;
    private final ExecutorService io;
    private final Handler ui;
    private final Listener listener;
    private final ScrollView root;
    private final LinearLayout box;
    private int generation;

    PairingScreen(Activity a, Prefs prefs, ExecutorService io, Handler ui, Listener listener) {
        this.a = a; this.prefs = prefs; this.io = io; this.ui = ui; this.listener = listener;
        root = new ScrollView(a);
        root.setBackgroundColor(BG);
        root.setFillViewport(true);
        box = new LinearLayout(a);
        box.setOrientation(LinearLayout.VERTICAL);
        box.setPadding(dp(32), dp(20), dp(32), dp(20));
        root.addView(box, new ScrollView.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
    }

    View view() { return root; }

    /** Stops polling (screen closed). */
    void stop() { generation++; }

    private int dp(int v) { return Math.round(v * a.getResources().getDisplayMetrics().density); }

    private TextView text(String s, int sp, int color) {
        TextView t = new TextView(a);
        t.setText(s);
        t.setTextSize(sp);
        t.setTextColor(color);
        t.setPadding(0, dp(4), 0, dp(4));
        return t;
    }

    private Button button(String s, View.OnClickListener l) {
        Button b = new Button(a);
        b.setText(s);
        b.setAllCaps(false);
        b.setOnClickListener(l);
        return b;
    }

    private void header(String subtitle) {
        box.removeAllViews();
        TextView t = text("Ven4Docking", 24, TEXT);
        t.setTypeface(Typeface.DEFAULT_BOLD);
        box.addView(t);
        box.addView(text(subtitle, 15, MUTE));
    }

    // ---------- step 1: choose the PC ----------

    void showChoose(String notice) { showChoose(notice, null); }

    /** retryBase: the PC of the attempt that just failed — offers a new code for it in one tap. */
    private void showChoose(String notice, final String retryBase) {
        final int gen = ++generation;
        header("Привязка к компьютеру. На ПК должна работать программа Ven4Docking.");
        if (notice != null) box.addView(text(notice, 15, WARN));
        if (retryBase != null) {
            box.addView(button("Запросить новый код для " + shortAddr(retryBase), v -> startPair(retryBase)));
        }

        box.addView(text("Компьютеры в этой сети:", 16, TEXT));
        final LinearLayout list = new LinearLayout(a);
        list.setOrientation(LinearLayout.VERTICAL);
        box.addView(list);
        final TextView scanState = text("Ищу…", 14, MUTE);
        list.addView(scanState);
        box.addView(button("Искать снова", v -> showChoose(null)));

        box.addView(text("Или введите адрес ПК (он показан в Ven4Docking на ПК):", 16, TEXT));
        LinearLayout row = new LinearLayout(a);
        row.setOrientation(LinearLayout.HORIZONTAL);
        row.setGravity(Gravity.CENTER_VERTICAL);
        final EditText addr = new EditText(a);
        addr.setSingleLine(true);
        addr.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_URI);
        // landscape panel: no full-screen keyboard editor that hides the "Подключить" button
        addr.setImeOptions(EditorInfo.IME_ACTION_GO | EditorInfo.IME_FLAG_NO_EXTRACT_UI | EditorInfo.IME_FLAG_NO_FULLSCREEN);
        addr.setHint("192.168.1.10 или 192.168.1.10:" + BuildConfig.AGENT_PORT);
        addr.setHintTextColor(MUTE);
        addr.setTextColor(TEXT);
        // what the user typed survives a new code, "Искать снова", "Отмена" and app restarts
        String typed = prefs.typedAddress();
        String old = prefs.baseUrl();
        if (!typed.isEmpty()) addr.setText(typed);
        else if (!old.isEmpty()) addr.setText(shortAddr(old));
        addr.setSelection(addr.getText().length());
        addr.addTextChangedListener(new TextWatcher() {
            @Override public void beforeTextChanged(CharSequence s, int start, int count, int after) { }
            @Override public void onTextChanged(CharSequence s, int start, int before, int count) { }
            @Override public void afterTextChanged(Editable s) { prefs.saveTypedAddress(s.toString()); }
        });
        row.addView(addr, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        final Runnable connect = () -> {
            String base = Prefs.normalizeBase(addr.getText().toString(), BuildConfig.AGENT_PORT);
            if (base == null) { addr.setError("Например 192.168.1.10"); return; }
            startPair(base);
        };
        row.addView(button("Подключить", v -> connect.run()));
        addr.setOnEditorActionListener((v, id, ev) -> { connect.run(); return true; });
        box.addView(row);

        io.execute(() -> {
            final List<Discovery.Found> found = Discovery.scan(BuildConfig.DISCOVERY_PORT, (int) SCAN_MS);
            ui.post(() -> {
                if (gen != generation) return;
                list.removeAllViews();
                if (found.isEmpty()) {
                    list.addView(text("Не нашёл. Проверьте, что телефон в той же Wi-Fi-сети, или введите адрес вручную.", 14, MUTE));
                }
                for (final Discovery.Found f : found) {
                    list.addView(button(f.name + "  ·  " + f.host + ":" + f.port, v -> startPair(f.baseUrl())));
                }
            });
        });
    }

    // ---------- step 2: code and waiting ----------

    /** "http://192.168.1.10:8765/" → "192.168.1.10:8765" */
    private static String shortAddr(String base) {
        return base.replace("http://", "").replaceAll("/$", "");
    }

    private void startPair(final String base) {
        final int gen = ++generation;
        header("Подключаюсь к " + shortAddr(base) + "…");
        final TextView status = text("", 15, MUTE);
        box.addView(status);
        box.addView(button("Отмена", v -> showChoose(null)));
        io.execute(() -> {
            String err;
            try {
                JSONObject body = new JSONObject();
                body.put("model", Build.MODEL);
                body.put("android", Build.VERSION.RELEASE);
                body.put("deviceId", prefs.deviceId());
                Http.Response r = Http.postJson(base + "pair/start", null, body.toString());
                if (r.ok()) {
                    JSONObject j = new JSONObject(r.body);
                    final String pairId = j.getString("pairId"), code = j.getString("code");
                    ui.post(() -> { if (gen == generation) showCode(base, pairId, code, gen); });
                    return;
                }
                err = r.code == 429 ? "Слишком много попыток — подождите минуту." : r.code == 403
                        ? "ПК не принимает подключения из этой сети." : "ПК ответил ошибкой " + r.code + ".";
            } catch (Exception e) {
                err = "Нет связи с ПК по этому адресу. Проверьте адрес и что Ven4Docking запущен.";
            }
            final String msg = err;
            ui.post(() -> { if (gen == generation) showChoose(msg, base); });
        });
    }

    private void showCode(final String base, final String pairId, String code, final int gen) {
        header("Подтвердите подключение на компьютере");
        TextView c = text(code, 64, ACC);
        c.setTypeface(Typeface.DEFAULT_BOLD);
        c.setLetterSpacing(0.2f);
        c.setGravity(Gravity.CENTER);
        c.setBackgroundColor(CARD);
        c.setPadding(dp(16), dp(8), dp(16), dp(8));
        box.addView(c, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        box.addView(text("В Ven4Docking на ПК появится запрос «Подключить " + Build.MODEL + "?» с этим кодом. Проверьте, что код совпадает, и нажмите «Подключить».", 15, TEXT));
        final TextView status = text("Жду подтверждения…", 14, MUTE);
        box.addView(status);
        box.addView(button("Отмена", v -> showChoose(null)));
        final long deadline = System.currentTimeMillis() + PAIR_TIMEOUT_MS;

        final Runnable[] poll = new Runnable[1];
        poll[0] = () -> io.execute(() -> {
            String done = null, fail = null, wait = null;
            try {
                Http.Response r = Http.get(base + "pair/" + pairId, null);
                if (r.code == 404) fail = "Код устарел (2 минуты). Запросите новый.";
                else if (r.ok()) {
                    JSONObject j = new JSONObject(r.body);
                    String st = j.optString("status");
                    if ("ok".equals(st)) {
                        String token = j.optString("token", "");
                        if (token.isEmpty()) fail = "ПК уже выдал ключ этому запросу. Начните заново.";
                        else done = token;
                    } else if ("denied".equals(st)) fail = "Подключение отклонено на ПК.";
                    else wait = "Жду подтверждения…";
                } else wait = "ПК ответил " + r.code + ", повторяю…";
            } catch (Exception e) {
                wait = "Нет связи с ПК, повторяю…";
            }
            final String token = done, failMsg = fail, waitMsg = wait;
            ui.post(() -> {
                if (gen != generation) return;
                if (token != null) {
                    generation++;
                    prefs.savePairing(base, token);
                    listener.onPaired();
                } else if (failMsg != null) {
                    showChoose(failMsg, base);
                } else if (System.currentTimeMillis() > deadline) {
                    showChoose("ПК не подтвердил подключение. Попробуйте ещё раз.", base);
                } else {
                    status.setText(waitMsg);
                    ui.postDelayed(poll[0], POLL_MS);
                }
            });
        });
        ui.postDelayed(poll[0], POLL_MS);
    }
}
