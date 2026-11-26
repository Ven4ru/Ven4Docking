namespace Ven4Docking.Core.Tests;

/// <summary>JSON copied verbatim from docs/specs §4 and docs/plans «Контракты между частями».</summary>
internal static class Samples
{
    public const string SpecConfig = """
        {
          "version": 1,
          "port": 8765,
          "theme": "teal",
          "language": "ru",
          "weather": {"name": "Москва", "lat": 55.75, "lon": 37.62},
          "apps": [{"id": "telegram", "title": "Telegram", "kind": "shortcut|exe|url|command", "target": "...", "args": "", "icon": "icons/telegram.png"}],
          "servers": [{"id": "site", "name": "Сайт", "host": "203.0.113.10", "port": 22, "user": "root", "key": "auto"}],
          "ai": {"enabled": ["claude-code", "claude-desktop", "codex", "ollama", "lmstudio"]},
          "phones": [{"id": "p1", "name": "realme C11", "tokenHash": "...", "night": {"from": "23:00", "to": "07:00", "brightness": 0.02}, "dimWhenPcOffline": true, "burnInShift": true}]
        }
        """;

    /// <summary>docs/specs 1.1 §3 (new config fields).</summary>
    public const string SpecConfig11 = """
        {
          "actions": [
            {"id": "mute-discord", "title": "Discord: микрофон", "icon": null, "kind": "keys", "keys": "Ctrl+Shift+M"},
            {"id": "sign", "title": "Подпись", "kind": "text", "text": "С уважением, …"},
            {"id": "stream-start", "title": "Начать день", "kind": "macro", "steps": [
              {"launch": "obs"}, {"wait": 3000}, {"keys": "Ctrl+Alt+F9"}]},
            {"id": "mic", "title": "Микрофон", "kind": "mic"},
            {"id": "out", "title": "Наушники / колонки", "kind": "audioOut", "devices": ["{0.0.0.00000000}.{…}", "{…}"]}
          ],
          "obs": {"host": "127.0.0.1", "port": 4455, "passwordDpapi": "AQAAANCMnd8B…"},
          "notify": {"enabled": false, "wallCountsOnly": false, "nightFlash": true,
                     "apps": {"TelegramDesktop": "title", "Discord": "full", "Microsoft.Office.OUTLOOK.EXE.15": "count"}},
          "wol": {"mac": "AA:BB:CC:DD:EE:FF", "broadcast": "192.168.1.255"}
        }
        """;

    /// <summary>docs/specs 1.1 §4: a program screen.</summary>
    public const string SpecProgramScreenLayout = """
        {"version": 3, "orientation": "landscape", "grid": {"cols": 8, "rows": 4},
         "screens": [{"id": "s1", "name": "Главный", "blocks": []},
                     {"id": "s3", "name": "OBS", "forApp": "obs64.exe", "blocks": [
                       {"id": "b1", "type": "obs", "x": 0, "y": 0, "w": 4, "h": 2, "opts": {"stream": true, "record": true, "scenes": true, "audio": true, "stats": true}}]}]}
        """;

    public const string SpecLayout = """
        {"version": 7, "orientation": "landscape", "grid": {"cols": 8, "rows": 4},
         "screens": [{"id": "s1", "name": "Главный", "blocks": [
           {"id": "b1", "type": "clock", "x": 0, "y": 0, "w": 3, "h": 2, "opts": {"weather": true}},
           {"id": "b2", "type": "apps", "x": 0, "y": 3, "w": 5, "h": 1, "opts": {"apps": ["telegram","chrome"]}},
           {"id": "b3", "type": "servers", "x": 0, "y": 0, "w": 8, "h": 4, "opts": {"servers": ["site"]}}]}]}
        """;

    /// <summary>GET /api/v1/layout from the plan: layout plus phone/theme fields the agent merges in.</summary>
    public const string ApiLayout = """
        {"version":7,"orientation":"landscape","grid":{"cols":8,"rows":4},"screens":[{"id":"s1","name":"Главный","blocks":[{"id":"b1","type":"cpu","x":0,"y":0,"w":2,"h":2,"opts":{}}]}],"night":{"from":"23:00","to":"07:00","brightness":0.02},"dimWhenPcOffline":true,"burnInShift":true,"theme":"teal","weather":{"name":"Москва","lat":55.75,"lon":37.62},"apps":[{"id":"telegram","title":"Telegram","icon":"/icons/telegram.png"}]}
        """;

    public const string AdbDevices = """
        * daemon not running; starting now at tcp:5037
        * daemon started successfully
        List of devices attached
        emulator-5554          device product:sdk_gphone64_x86_64 model:sdk_gphone64_x86_64 device:emu64xa transport_id:2
        R58M123ABCD            unauthorized usb:1-4 transport_id:3
        192.168.1.50:5555      device product:RMX2185 model:RMX2185 device:RMX2185 transport_id:4
        adb-R58M123ABCD-abc123._adb-tls-connect._tcp device product:a52qnsxx model:SM_A525F device:a52q transport_id:5
        0123456789ABCDEF       offline transport_id:6
        ZY22ABC                no permissions (user in plugdev group; are your udev rules wrong?); see [http://developer.android.com/tools/device.html] usb:1-2 transport_id:7

        """;

    public const string WebViewDumpsys = """
        Current WebView package (name, version): (com.google.android.webview, 80.0.3987.99)
        Minimum targetSdkVersion: 29
        WebView packages:
          Valid package com.google.android.webview (versionName: 80.0.3987.99, versionCode: 398709900, targetSdkVersion: 29) is installed/enabled for all users
        """;
}
