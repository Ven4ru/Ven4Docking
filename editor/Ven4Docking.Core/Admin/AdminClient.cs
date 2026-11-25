using System.Net;
using System.Net.Http.Json;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Ven4Docking.Core.Models;

namespace Ven4Docking.Core.Admin;

public sealed class AdminException : Exception
{
    public AdminException(HttpStatusCode? status, string message, Exception? inner = null) : base(message, inner) => Status = status;

    /// <summary>Null when the agent could not be reached at all.</summary>
    public HttpStatusCode? Status { get; }

    public bool AgentUnreachable => Status is null;
}

/// <summary>
/// Client of the editor API of the agent (spec §3.5, plan «API редактора»): http://127.0.0.1:{port}/admin/*,
/// header <c>X-Admin-Key</c> = content of admin.key.
/// </summary>
public sealed class AdminClient : IDisposable
{
    public const string KeyHeader = "X-Admin-Key";

    private readonly HttpClient _http;
    private readonly Func<string?> _key;

    public AdminClient(HttpClient http, Func<string?> keyProvider)
    {
        _http = http;
        _key = keyProvider;
    }

    public static AdminClient Create(int port, Func<string?> keyProvider, HttpMessageHandler? handler = null)
    {
        var http = handler is null ? new HttpClient() : new HttpClient(handler, disposeHandler: false);
        http.BaseAddress = new Uri($"http://127.0.0.1:{port}/");
        http.Timeout = TimeSpan.FromSeconds(10);
        return new AdminClient(http, keyProvider);
    }

    public Uri BaseAddress => _http.BaseAddress!;

    /// <summary>Null when the agent is not running (no exception: the editor polls this).</summary>
    public async Task<AgentStatus?> TryGetStatusAsync(CancellationToken ct = default)
    {
        try { return await GetAsync<AgentStatus>("admin/status", ct).ConfigureAwait(false); }
        catch (AdminException) { return null; }
    }

    public Task<DockConfig?> GetConfigAsync(CancellationToken ct = default) => GetAsync<DockConfig>("admin/config", ct);

    public Task PutConfigAsync(DockConfig config, CancellationToken ct = default) =>
        SendAsync(HttpMethod.Put, "admin/config", config, ct);

    public async Task<PanelLayout?> GetLayoutAsync(string phoneId, CancellationToken ct = default)
    {
        try { return await GetAsync<PanelLayout>("admin/layouts/" + Uri.EscapeDataString(phoneId), ct).ConfigureAwait(false); }
        catch (AdminException e) when (e.Status == HttpStatusCode.NotFound) { return null; }
    }

    /// <summary>PUT /admin/layouts/{phone}. 400 {"error":"overlap","blocks":[…]} is returned as a failed <see cref="SaveResult"/>.</summary>
    public async Task<SaveResult> PutLayoutAsync(string phoneId, PanelLayout layout, CancellationToken ct = default)
    {
        using var resp = await RawAsync(HttpMethod.Put, "admin/layouts/" + Uri.EscapeDataString(phoneId), layout, ct).ConfigureAwait(false);
        string body = await resp.Content.ReadAsStringAsync(ct).ConfigureAwait(false);
        if (resp.IsSuccessStatusCode)
        {
            int? version = null;
            try
            {
                if (!string.IsNullOrWhiteSpace(body) && JsonNode.Parse(body) is JsonObject o && o["version"] is JsonValue v && v.TryGetValue<int>(out int ver))
                    version = ver;
            }
            catch (JsonException) { }
            return SaveResult.Success(version);
        }
        if (resp.StatusCode == HttpStatusCode.BadRequest)
        {
            try
            {
                var err = JsonNode.Parse(body) as JsonObject;
                var blocks = (err?["blocks"] as JsonArray)?.Select(n => n?.GetValue<string>() ?? "").ToList();
                return new SaveResult(false, err?["error"]?.GetValue<string>() ?? "bad-request", blocks);
            }
            catch (JsonException) { return new SaveResult(false, "bad-request"); }
        }
        throw new AdminException(resp.StatusCode, $"PUT layout: {(int)resp.StatusCode} {body}");
    }

    public async Task<IReadOnlyList<PhoneInfo>> GetPhonesAsync(CancellationToken ct = default) =>
        await GetAsync<List<PhoneInfo>>("admin/phones", ct).ConfigureAwait(false) ?? new List<PhoneInfo>();

    public Task RevokePhoneAsync(string id, CancellationToken ct = default) =>
        SendAsync(HttpMethod.Post, $"admin/phones/{Uri.EscapeDataString(id)}/revoke", null, ct);

    /// <summary>POST /admin/phones/token {name, model} → {id, token}: pairing without a code, for the adb wizard.</summary>
    public async Task<PhoneTokenResult> CreatePhoneTokenAsync(string name, string model, CancellationToken ct = default) =>
        await SendAsync<PhoneTokenResult>(HttpMethod.Post, "admin/phones/token", new { name, model }, ct).ConfigureAwait(false)
        ?? throw new AdminException(null, "Empty token response");

    public async Task<IReadOnlyList<PairRequest>> GetPairsAsync(CancellationToken ct = default) =>
        await GetAsync<List<PairRequest>>("admin/pairs", ct).ConfigureAwait(false) ?? new List<PairRequest>();

    public Task AcceptPairAsync(string pairId, CancellationToken ct = default) =>
        SendAsync(HttpMethod.Post, $"admin/pairs/{Uri.EscapeDataString(pairId)}/accept", null, ct);

    public Task DenyPairAsync(string pairId, CancellationToken ct = default) =>
        SendAsync(HttpMethod.Post, $"admin/pairs/{Uri.EscapeDataString(pairId)}/deny", null, ct);

    public async Task<ServerTestResult> TestServerAsync(ServerEntry server, CancellationToken ct = default) =>
        await SendAsync<ServerTestResult>(HttpMethod.Post, "admin/servers/test", server, ct).ConfigureAwait(false)
        ?? new ServerTestResult { Ok = false, Err = "empty response" };

    public async Task<IReadOnlyList<AiSourceInfo>> GetAiSourcesAsync(CancellationToken ct = default) =>
        await GetAsync<List<AiSourceInfo>>("admin/ai/sources", ct).ConfigureAwait(false) ?? new List<AiSourceInfo>();

    public Task ReloadAsync(CancellationToken ct = default) => SendAsync(HttpMethod.Post, "admin/reload", null, ct);

    // ---- plumbing ----

    private async Task<T?> GetAsync<T>(string path, CancellationToken ct)
    {
        using var resp = await RawAsync(HttpMethod.Get, path, null, ct).ConfigureAwait(false);
        await EnsureOk(resp, ct).ConfigureAwait(false);
        return await resp.Content.ReadFromJsonAsync<T>(JsonDefaults.Options, ct).ConfigureAwait(false);
    }

    private async Task SendAsync(HttpMethod method, string path, object? body, CancellationToken ct)
    {
        using var resp = await RawAsync(method, path, body, ct).ConfigureAwait(false);
        await EnsureOk(resp, ct).ConfigureAwait(false);
    }

    private async Task<T?> SendAsync<T>(HttpMethod method, string path, object? body, CancellationToken ct)
    {
        using var resp = await RawAsync(method, path, body, ct).ConfigureAwait(false);
        await EnsureOk(resp, ct).ConfigureAwait(false);
        return await resp.Content.ReadFromJsonAsync<T>(JsonDefaults.Options, ct).ConfigureAwait(false);
    }

    private async Task<HttpResponseMessage> RawAsync(HttpMethod method, string path, object? body, CancellationToken ct)
    {
        using var req = new HttpRequestMessage(method, path);
        string? key = _key();
        if (!string.IsNullOrEmpty(key)) req.Headers.TryAddWithoutValidation(KeyHeader, key);
        if (body is not null)
        {
            string json = body switch
            {
                string s => s,
                _ => JsonSerializer.Serialize(body, body.GetType(), JsonDefaults.Compact),
            };
            req.Content = new StringContent(json, Encoding.UTF8, "application/json");
        }
        try
        {
            return await _http.SendAsync(req, ct).ConfigureAwait(false);
        }
        catch (HttpRequestException e) { throw new AdminException(null, e.Message, e); }
        catch (TaskCanceledException e) when (!ct.IsCancellationRequested) { throw new AdminException(null, "timeout", e); }
    }

    private static async Task EnsureOk(HttpResponseMessage resp, CancellationToken ct)
    {
        if (resp.IsSuccessStatusCode) return;
        string text = await resp.Content.ReadAsStringAsync(ct).ConfigureAwait(false);
        throw new AdminException(resp.StatusCode, $"{(int)resp.StatusCode}: {text}");
    }

    public void Dispose() => _http.Dispose();
}
