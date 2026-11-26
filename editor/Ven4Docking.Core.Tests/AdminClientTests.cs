using System.Net;
using System.Text.Json.Nodes;
using Ven4Docking.Core.Admin;
using Ven4Docking.Core.Models;

namespace Ven4Docking.Core.Tests;

public class AdminClientTests
{
    [Fact]
    public async Task Sends_admin_key_to_localhost_port()
    {
        var h = new StubHandler((_, _) => StubHandler.Json(HttpStatusCode.OK,
            """{"version":"1.0.0","port":18765,"cpuPct":0.1,"ramBytes":12000000,"activePanels":1,"blockErrors":{}}"""));
        using var c = AdminClient.Create(18765, () => "k3y", h);
        var s = await c.TryGetStatusAsync();
        Assert.Equal("1.0.0", s!.Version);
        Assert.Equal(1, s.ActivePanels);
        var req = h.Calls.Single().Request;
        Assert.Equal("http://127.0.0.1:18765/admin/status", req.RequestUri!.ToString());
        Assert.Equal("k3y", req.Headers.GetValues(AdminClient.KeyHeader).Single());
    }

    [Fact]
    public async Task Status_is_null_when_agent_is_down()
    {
        var h = new StubHandler((_, _) => throw new HttpRequestException("refused"));
        using var c = AdminClient.Create(18765, () => null, h);
        Assert.Null(await c.TryGetStatusAsync());
        var ex = await Assert.ThrowsAsync<AdminException>(() => c.GetPhonesAsync());
        Assert.True(ex.AgentUnreachable);
    }

    [Fact]
    public async Task Put_layout_overlap_400_is_returned_as_failed_save()
    {
        var h = new StubHandler((_, _) => StubHandler.Json(HttpStatusCode.BadRequest, """{"error":"overlap","blocks":["b1","b2"]}"""));
        using var c = AdminClient.Create(18765, () => "k", h);
        var r = await c.PutLayoutAsync("p1", LayoutTemplates.Create(LayoutTemplates.AppsOnly));
        Assert.False(r.Ok);
        Assert.Equal("overlap", r.Error);
        Assert.Equal(new[] { "b1", "b2" }, r.Blocks);
        var (req, body) = h.Calls.Single();
        Assert.Equal(HttpMethod.Put, req.Method);
        Assert.EndsWith("/admin/layouts/p1", req.RequestUri!.AbsolutePath);
        Assert.Equal("apps", JsonNode.Parse(body!)!["screens"]![0]!["blocks"]![0]!["type"]!.GetValue<string>());
    }

    [Fact]
    public async Task Phone_token_and_pairs()
    {
        var h = new StubHandler((req, _) => req.RequestUri!.AbsolutePath switch
        {
            "/admin/phones/token" => StubHandler.Json(HttpStatusCode.OK, """{"id":"p2","token":"secret"}"""),
            "/admin/pairs" => StubHandler.Json(HttpStatusCode.OK, """[{"pairId":"x1","model":"RMX2185","android":"10","code":"4821","ip":"192.168.1.50"}]"""),
            _ => new HttpResponseMessage(HttpStatusCode.OK),
        });
        using var c = AdminClient.Create(18765, () => "k", h);

        var t = await c.CreatePhoneTokenAsync("realme C11", "RMX2185");
        Assert.Equal(("p2", "secret"), (t.Id, t.Token));
        var tokenBody = JsonNode.Parse(h.Calls[0].Body!)!;
        Assert.Equal("realme C11", tokenBody["name"]!.GetValue<string>());
        Assert.Equal("RMX2185", tokenBody["model"]!.GetValue<string>());

        var pair = Assert.Single(await c.GetPairsAsync());
        Assert.Equal(("x1", "4821"), (pair.PairId, pair.Code));

        await c.AcceptPairAsync("x1");
        await c.DenyPairAsync("x1");
        await c.RevokePhoneAsync("p2");
        Assert.Equal(new[] { "/admin/pairs/x1/accept", "/admin/pairs/x1/deny", "/admin/phones/p2/revoke" },
            h.Calls.Skip(2).Select(x => x.Request.RequestUri!.AbsolutePath));
        Assert.All(h.Calls.Skip(2), x => Assert.Equal(HttpMethod.Post, x.Request.Method));
    }

    [Fact]
    public async Task Server_test_posts_server_and_reads_result()
    {
        var h = new StubHandler((_, _) => StubHandler.Json(HttpStatusCode.OK, """{"ok":false,"ping":null,"err":"timeout"}"""));
        using var c = AdminClient.Create(18765, () => "k", h);
        var r = await c.TestServerAsync(new ServerEntry { Id = "site", Name = "Сайт", Host = "203.0.113.10" });
        Assert.False(r.Ok);
        Assert.Null(r.Ping);
        Assert.Equal("timeout", r.Err);
        Assert.Equal("203.0.113.10", JsonNode.Parse(h.Calls[0].Body!)!["host"]!.GetValue<string>());
    }

    [Fact]
    public async Task Admin_api_11_endpoints_are_parsed()
    {
        var h = new StubHandler((req, _) => req.RequestUri!.AbsolutePath switch
        {
            "/admin/notify/apps" => StubHandler.Json(HttpStatusCode.OK,
                """{"access":"denied","apps":[{"aumid":"TelegramDesktop","name":"Telegram","lastSeen":1764150000.5}]}"""),
            "/admin/audio/devices" => StubHandler.Json(HttpStatusCode.OK,
                """{"outputs":[{"id":"{0.0.0.00000000}.{a}","name":"Динамики","default":true},{"id":"{b}","name":"Наушники","default":false}],"inputs":[{"id":"{c}","name":"Микрофон","default":true}]}"""),
            "/admin/processes" => StubHandler.Json(HttpStatusCode.OK, """[{"name":"obs64.exe","title":"OBS 30.2"},{"name":"notepad.exe","title":""}]"""),
            "/admin/wol" => StubHandler.Json(HttpStatusCode.OK, """{"mac":"AA:BB:CC:DD:EE:FF","broadcast":"192.168.1.255","adapter":"Ethernet","wakeEnabled":null}"""),
            _ => new HttpResponseMessage(HttpStatusCode.NotFound),
        });
        using var c = AdminClient.Create(18765, () => "k", h);

        var n = await c.GetNotifyAppsAsync();
        Assert.Equal(NotifyAccess.Denied, n.Access);
        var app = Assert.Single(n.Apps);
        Assert.Equal(("TelegramDesktop", "Telegram", 1764150000.5), (app.Aumid, app.Name, app.LastSeen));

        var a = await c.GetAudioDevicesAsync();
        Assert.Equal(new[] { "Динамики", "Наушники" }, a.Outputs.Select(o => o.Name));
        Assert.True(a.Outputs[0].Default);
        Assert.Equal("{c}", Assert.Single(a.Inputs).Id);

        var p = await c.GetProcessesAsync();
        Assert.Equal(new[] { "obs64.exe", "notepad.exe" }, p.Select(x => x.Name));

        var w = await c.GetWolAsync();
        Assert.Equal(("AA:BB:CC:DD:EE:FF", "192.168.1.255", "Ethernet"), (w.Mac, w.Broadcast, w.Adapter));
        Assert.Null(w.WakeEnabled);

        Assert.All(h.Calls, x => Assert.Equal(HttpMethod.Get, x.Request.Method));
        Assert.All(h.Calls, x => Assert.Equal("k", x.Request.Headers.GetValues(AdminClient.KeyHeader).Single()));
    }

    [Fact]
    public async Task Obs_test_sends_only_the_encrypted_password()
    {
        var h = new StubHandler((_, _) => StubHandler.Json(HttpStatusCode.OK, """{"ok":false,"error":"неверный пароль"}"""));
        using var c = AdminClient.Create(18765, () => "k", h);
        var r = await c.TestObsAsync("127.0.0.1", 4455, "AQAAANCMnd8B");
        Assert.False(r.Ok);
        Assert.Equal("неверный пароль", r.Error);
        var (req, body) = h.Calls.Single();
        Assert.Equal(HttpMethod.Post, req.Method);
        Assert.Equal("/admin/obs/test", req.RequestUri!.AbsolutePath);
        var o = JsonNode.Parse(body!)!.AsObject();
        Assert.Equal(new[] { "host", "port", "passwordDpapi" }, o.Select(x => x.Key));
        Assert.Equal(4455, o["port"]!.GetValue<int>());

        await c.TestObsAsync("127.0.0.1", 4455, null);
        Assert.False(JsonNode.Parse(h.Calls[1].Body!)!.AsObject().ContainsKey("passwordDpapi"));
    }

    [Fact]
    public async Task Older_agent_404_is_reported_as_too_old()
    {
        var h = new StubHandler((_, _) => StubHandler.Json(HttpStatusCode.NotFound, """{"error":"no such admin api"}"""));
        using var c = AdminClient.Create(18765, () => "k", h);
        foreach (var call in new Func<Task>[]
                 {
                     () => c.GetNotifyAppsAsync(), () => c.GetAudioDevicesAsync(), () => c.GetProcessesAsync(),
                     () => c.GetWolAsync(), () => c.TestObsAsync("127.0.0.1", 4455, null),
                 })
        {
            var e = await Assert.ThrowsAsync<AdminException>(call);
            Assert.True(e.AgentTooOld);
            Assert.False(e.AgentUnreachable);
        }

        var down = new StubHandler((_, _) => throw new HttpRequestException("refused"));
        using var c2 = AdminClient.Create(18765, () => "k", down);
        var e2 = await Assert.ThrowsAsync<AdminException>(() => c2.GetWolAsync());
        Assert.True(e2.AgentUnreachable);
        Assert.False(e2.AgentTooOld);
    }
}
