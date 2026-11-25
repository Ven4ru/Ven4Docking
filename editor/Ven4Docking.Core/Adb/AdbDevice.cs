namespace Ven4Docking.Core.Adb;

/// <summary>One line of <c>adb devices -l</c>.</summary>
public sealed record AdbDevice(string Serial, string State, string? Model = null, string? Product = null,
    string? Device = null, string? TransportId = null, string? Usb = null)
{
    public bool IsReady => State == "device";
    public bool IsUnauthorized => State == "unauthorized";
    public bool IsEmulator => Serial.StartsWith("emulator-", StringComparison.Ordinal);

    /// <summary>Wi-Fi debugging: "ip:port" or an mDNS name "adb-XXXX._adb-tls-connect._tcp".</summary>
    public bool IsWifi => !IsEmulator && (Serial.Contains(':') || Serial.Contains("._adb-tls-connect."));

    public string DisplayName => (Model ?? Product ?? Serial).Replace('_', ' ');
}

public static class AdbDevicesParser
{
    public static IReadOnlyList<AdbDevice> Parse(string output)
    {
        var list = new List<AdbDevice>();
        foreach (var raw in output.Split('\n'))
        {
            string line = raw.Trim();
            if (line.Length == 0 || line.StartsWith("List of devices", StringComparison.Ordinal) || line.StartsWith('*'))
                continue;
            if (line.StartsWith("adb server", StringComparison.OrdinalIgnoreCase) || line.StartsWith("adb: ", StringComparison.Ordinal))
                continue;

            var parts = line.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries);
            if (parts.Length < 2) continue;

            string serial = parts[0];
            int i = 1;
            // State can be two words: "no permissions (…)" — take everything up to the first key:value.
            var state = new List<string>();
            while (i < parts.Length && !IsKeyValue(parts[i])) state.Add(parts[i++]);
            if (state.Count == 0) continue;

            var kv = new Dictionary<string, string>(StringComparer.Ordinal);
            for (; i < parts.Length; i++)
            {
                int c = parts[i].IndexOf(':');
                if (c > 0) kv[parts[i][..c]] = parts[i][(c + 1)..];
            }

            list.Add(new AdbDevice(serial, state[0], Get(kv, "model"), Get(kv, "product"), Get(kv, "device"),
                Get(kv, "transport_id"), Get(kv, "usb")));
        }
        return list;
    }

    private static bool IsKeyValue(string s)
    {
        int c = s.IndexOf(':');
        return c > 0 && c < s.Length - 1 && s[..c].All(ch => char.IsLetterOrDigit(ch) || ch == '_');
    }

    private static string? Get(Dictionary<string, string> kv, string key) => kv.TryGetValue(key, out var v) ? v : null;
}
