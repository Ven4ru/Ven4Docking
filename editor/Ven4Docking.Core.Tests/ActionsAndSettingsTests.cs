using System.Security.Cryptography;
using System.Text;
using System.Text.Json.Nodes;
using Ven4Docking.Core.Models;
using Ven4Docking.Core.Security;
using Ven4Docking.Core.Validation;

namespace Ven4Docking.Core.Tests;

public class HotkeyTests
{
    [Theory]
    [InlineData("Ctrl+Shift+M", "Ctrl+Shift+M")]
    [InlineData("ctrl + shift + m", "Ctrl+Shift+M")]
    [InlineData("Shift+Ctrl+m", "Ctrl+Shift+M")]
    [InlineData("Win+Alt+F24", "Alt+Win+F24")]
    [InlineData("CTRL+ALT+F9", "Ctrl+Alt+F9")]
    [InlineData("pageup", "PageUp")]
    [InlineData("Ctrl+numpad7", "Ctrl+NumPad7")]
    [InlineData("Alt+PrintScreen", "Alt+PrintScreen")]
    [InlineData("Ctrl+0", "Ctrl+0")]
    [InlineData("M+Ctrl", "Ctrl+M")]
    public void Parses_spec_combinations_into_canonical_form(string input, string canonical) =>
        Assert.Equal(canonical, Hotkey.Normalize(input));

    [Theory]
    [InlineData("")]
    [InlineData("   ")]
    [InlineData(null)]
    [InlineData("Ctrl+")]
    [InlineData("Ctrl+Shift")]
    [InlineData("M+N")]
    [InlineData("Ctrl+Ctrl+M")]
    [InlineData("Ctrl+F25")]
    [InlineData("Hyper+M")]
    [InlineData("Control+M")]
    [InlineData("Ctrl+Shift+MM")]
    [InlineData("Ctrl++")]
    [InlineData("Ctrl+M+Shift+N")]
    public void Rejects_what_the_agent_would_refuse(string? input) =>
        Assert.False(Hotkey.TryParse(input, out _));

    [Fact]
    public void Every_spec_key_is_known_and_round_trips()
    {
        var names = new List<string> { "Enter", "Esc", "Tab", "Space", "Backspace", "Delete", "Insert", "Home", "End", "PageUp",
            "PageDown", "Up", "Down", "Left", "Right", "PrintScreen", "Pause" };
        names.AddRange(Enumerable.Range('A', 26).Select(c => ((char)c).ToString()));
        names.AddRange(Enumerable.Range(0, 10).Select(i => i.ToString()));
        names.AddRange(Enumerable.Range(1, 24).Select(i => "F" + i));
        names.AddRange(Enumerable.Range(0, 10).Select(i => "NumPad" + i));
        Assert.Equal(names.Order(), Hotkey.Keys.Keys.Order());
        foreach (var n in names)
        {
            Assert.Equal("Ctrl+Alt+Shift+Win+" + n, Hotkey.Normalize("win+shift+alt+ctrl+" + n.ToLowerInvariant()));
            Assert.Equal(n, Hotkey.KeyName(Hotkey.Keys[n]));
        }
    }

    [Fact]
    public void Virtual_keys_map_to_names_and_modifiers_are_recognised()
    {
        Assert.Equal("M", Hotkey.KeyName(0x4D));
        Assert.Equal("F1", Hotkey.KeyName(0x70));
        Assert.Equal("F24", Hotkey.KeyName(0x87));
        Assert.Equal("NumPad0", Hotkey.KeyName(0x60));
        Assert.Equal("Esc", Hotkey.KeyName(0x1B));
        Assert.Null(Hotkey.KeyName(0x10)); // Shift
        Assert.Null(Hotkey.KeyName(0xBA)); // OEM ;
        foreach (int vk in new[] { 0x10, 0x11, 0x12, 0x5B, 0x5C, 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5 })
            Assert.True(Hotkey.IsModifierKey(vk));
        Assert.False(Hotkey.IsModifierKey(0x4D));
        Assert.Equal("Ctrl+Shift+", Hotkey.FormatPartial(HotkeyModifiers.Shift | HotkeyModifiers.Ctrl));
        Assert.Equal("", Hotkey.FormatPartial(HotkeyModifiers.None));
        Assert.Equal("Ctrl+Shift+D", Hotkey.Format(HotkeyModifiers.Ctrl | HotkeyModifiers.Shift, "D"));
    }
}

public class ActionModelTests
{
    /// <summary>The editor always writes version, apps, servers and phones (as in 1.0); the rest must survive as is.</summary>
    private static void AssertSameJson(JsonNode? expected, string actual)
    {
        var e = expected!.AsObject();
        e["version"] ??= 1;
        foreach (var k in new[] { "apps", "servers", "phones" }) e[k] ??= new JsonArray();
        var a = JsonNode.Parse(actual);
        Assert.True(JsonNode.DeepEquals(expected, a), $"JSON differs.\nExpected: {expected?.ToJsonString()}\nActual:   {a?.ToJsonString()}");
    }

    [Fact]
    public void Spec_11_config_round_trips()
    {
        var cfg = JsonDefaults.Deserialize<DockConfig>(Samples.SpecConfig11)!;
        var expected = JsonNode.Parse(Samples.SpecConfig11)!.AsObject();
        expected["actions"]![0]!.AsObject().Remove("icon"); // null is not written
        AssertSameJson(expected, JsonDefaults.Serialize(cfg));
    }

    [Fact]
    public void Spec_11_config_is_parsed_into_typed_fields()
    {
        var cfg = JsonDefaults.Deserialize<DockConfig>(Samples.SpecConfig11)!;
        var actions = cfg.Actions!;
        Assert.Equal(new[] { "keys", "text", "macro", "mic", "audioOut" }, actions.Select(a => a.Kind));
        Assert.Equal("Ctrl+Shift+M", actions[0].Keys);
        var steps = actions[2].Steps!;
        Assert.Equal(new[] { MacroStep.KindLaunch, MacroStep.KindWait, MacroStep.KindKeys }, steps.Select(s => s.Kind));
        Assert.Equal(3000, steps[1].Wait);
        Assert.Equal(2, actions[4].Devices!.Count);
        Assert.Equal(("127.0.0.1", 4455), (cfg.Obs!.Host, cfg.Obs.Port));
        Assert.Equal(NotifySettings.ModeFull, cfg.Notify!.GetMode("Discord"));
        Assert.True(cfg.Notify.NightFlash);
        Assert.Equal("AA:BB:CC:DD:EE:FF", cfg.Wol!.Mac);
    }

    [Fact]
    public void Config_without_11_fields_does_not_get_them()
    {
        var node = JsonNode.Parse(JsonDefaults.Serialize(JsonDefaults.Deserialize<DockConfig>(Samples.SpecConfig)!))!.AsObject();
        foreach (var k in new[] { "actions", "obs", "notify", "wol" }) Assert.False(node.ContainsKey(k), k);
    }

    [Fact]
    public void Unknown_fields_of_actions_steps_and_settings_survive()
    {
        const string json = """
            {"version":1,"future":{"x":1},
             "actions":[{"id":"a","title":"A","kind":"macro","color":"red","steps":[{"wait":10,"repeat":2}]}],
             "obs":{"host":"h","port":1,"tls":true},
             "notify":{"enabled":true,"wallCountsOnly":false,"nightFlash":false,"apps":{},"sound":"ding"},
             "wol":{"mac":"AA:BB:CC:DD:EE:FF","broadcast":"10.0.0.255","port":7}}
            """;
        AssertSameJson(JsonNode.Parse(json), JsonDefaults.Serialize(JsonDefaults.Deserialize<DockConfig>(json)!));
    }

    [Fact]
    public void Macro_step_kind_requires_exactly_one_field()
    {
        Assert.Equal(MacroStep.KindText, new MacroStep { Text = "" }.Kind);
        Assert.Null(new MacroStep().Kind);
        Assert.Null(new MacroStep { Keys = "A", Wait = 5 }.Kind);
        foreach (var k in MacroStep.Kinds) Assert.Equal(k, MacroStep.Create(k).Kind);
        var node = JsonNode.Parse(JsonDefaults.Serialize(MacroStep.Create(MacroStep.KindWait)))!.AsObject();
        Assert.Equal(new[] { "wait" }, node.Select(p => p.Key));
    }

    [Fact]
    public void Changing_kind_drops_foreign_fields()
    {
        var a = new ActionEntry { Id = "x", Kind = ActionEntry.KindKeys, Keys = "A", Text = "t", Devices = new() { "d" } };
        a.KeepOnlyKindFields();
        Assert.Equal("A", a.Keys);
        Assert.Null(a.Text);
        Assert.Null(a.Devices);
    }
}

public class ActionValidatorTests
{
    private static DockConfig Config(params ActionEntry[] actions) => new()
    {
        Apps = { new AppEntry { Id = "obs", Title = "OBS" } },
        Actions = actions.ToList(),
    };

    private static IEnumerable<string> Kinds(DockConfig c) => ActionValidator.Validate(c).Select(i => i.Kind);

    [Fact]
    public void Spec_example_is_valid_apart_from_placeholders()
    {
        var cfg = JsonDefaults.Deserialize<DockConfig>(Samples.SpecConfig11)!;
        cfg.Apps.Add(new AppEntry { Id = "obs" });
        Assert.Empty(ActionValidator.Validate(cfg));
    }

    [Fact]
    public void No_actions_no_issues() => Assert.Empty(ActionValidator.Validate(new DockConfig()));

    [Fact]
    public void Ids_follow_valid_id_and_are_unique()
    {
        Assert.True(ActionValidator.IsValidId("discord-mic_2"));
        Assert.False(ActionValidator.IsValidId("микрофон"));
        Assert.False(ActionValidator.IsValidId(new string('a', 65)));
        Assert.False(ActionValidator.IsValidId(""));
        var c = Config(
            new ActionEntry { Id = "a b", Kind = ActionEntry.KindMic },
            new ActionEntry { Id = "m", Kind = ActionEntry.KindMic },
            new ActionEntry { Id = "m", Kind = ActionEntry.KindMic });
        Assert.Equal(new[] { ActionIssueKinds.BadId, ActionIssueKinds.DuplicateId }, Kinds(c));
    }

    [Fact]
    public void Keys_text_and_devices_are_checked()
    {
        var c = Config(
            new ActionEntry { Id = "k1", Kind = ActionEntry.KindKeys, Keys = "Ctrl+Shift" },
            new ActionEntry { Id = "k2", Kind = ActionEntry.KindKeys },
            new ActionEntry { Id = "t1", Kind = ActionEntry.KindText, Text = "" },
            new ActionEntry { Id = "t2", Kind = ActionEntry.KindText, Text = new string('x', 2001) },
            new ActionEntry { Id = "t3", Kind = ActionEntry.KindText, Text = new string('x', 2000) },
            new ActionEntry { Id = "o1", Kind = ActionEntry.KindAudioOut, Devices = new() },
            new ActionEntry { Id = "z", Kind = "teleport" });
        var issues = ActionValidator.Validate(c);
        Assert.Equal(new[]
        {
            ("k1", ActionIssueKinds.BadKeys), ("k2", ActionIssueKinds.BadKeys), ("t1", ActionIssueKinds.EmptyText),
            ("t2", ActionIssueKinds.TextTooLong), ("o1", ActionIssueKinds.NoDevices), ("z", ActionIssueKinds.BadKind),
        }, issues.Select(i => (i.ActionId, i.Kind)));
    }

    [Fact]
    public void Macro_steps_are_checked_one_by_one()
    {
        var c = Config(
            new ActionEntry { Id = "empty", Kind = ActionEntry.KindMacro, Steps = new() },
            new ActionEntry { Id = "long", Kind = ActionEntry.KindMacro, Steps = Enumerable.Range(0, 21).Select(_ => new MacroStep { Wait = 1 }).ToList() },
            new ActionEntry
            {
                Id = "bad", Kind = ActionEntry.KindMacro, Steps = new()
                {
                    new MacroStep { Launch = "obs" },
                    new MacroStep { Wait = 0 },
                    new MacroStep { Wait = 10001 },
                    new MacroStep { Launch = "chrome" },
                    new MacroStep { Keys = "Ctrl+Q+W" },
                    new MacroStep { Text = "" },
                    new MacroStep(),
                    new MacroStep { Keys = "Ctrl+Alt+F9" },
                },
            });
        var issues = ActionValidator.Validate(c);
        Assert.Equal(new (string, string, int?)[]
        {
            ("empty", ActionIssueKinds.StepCount, null),
            ("long", ActionIssueKinds.StepCount, null),
            ("bad", ActionIssueKinds.BadWait, 1),
            ("bad", ActionIssueKinds.BadWait, 2),
            ("bad", ActionIssueKinds.UnknownApp, 3),
            ("bad", ActionIssueKinds.BadKeys, 4),
            ("bad", ActionIssueKinds.EmptyText, 5),
            ("bad", ActionIssueKinds.BadStep, 6),
        }, issues.Select(i => (i.ActionId, i.Kind, i.Step)));
    }
}

public class IntegrationSettingsTests
{
    [Fact]
    public void Obs_password_dpapi_round_trip()
    {
        const string password = "пароль-OBS 123!";
        string b64 = Dpapi.Protect(password);
        Assert.DoesNotContain(password, b64);
        Assert.DoesNotContain(password, Encoding.UTF8.GetString(Convert.FromBase64String(b64)));
        Assert.True(Dpapi.TryUnprotect(b64, out var back));
        Assert.Equal(password, back);

        // contract with the agent (CryptUnprotectData, no entropy): UTF-8 bytes, CurrentUser
        byte[] raw = ProtectedData.Unprotect(Convert.FromBase64String(b64), null, DataProtectionScope.CurrentUser);
        Assert.Equal(password, Encoding.UTF8.GetString(raw));

        Assert.NotEqual(b64, Dpapi.Protect(password)); // salted: the same password never gives the same text
    }

    [Fact]
    public void Dpapi_refuses_foreign_text()
    {
        Assert.False(Dpapi.TryUnprotect(null, out _));
        Assert.False(Dpapi.TryUnprotect("not base64!", out _));
        Assert.False(Dpapi.TryUnprotect(Convert.ToBase64String(new byte[] { 1, 2, 3, 4 }), out _));
        Assert.False(Dpapi.TryUnprotect("AQAAANCMnd8B", out _));
    }

    [Fact]
    public void Obs_settings_serialize_only_the_ciphertext()
    {
        var cfg = new DockConfig { Obs = new ObsSettings { PasswordDpapi = Dpapi.Protect("s3cret-pass") } };
        string json = JsonDefaults.Serialize(cfg);
        Assert.DoesNotContain("s3cret-pass", json);
        var obs = JsonNode.Parse(json)!["obs"]!.AsObject();
        Assert.Equal(new[] { "host", "port", "passwordDpapi" }, obs.Select(p => p.Key));
        Assert.False(JsonNode.Parse(JsonDefaults.Serialize(new ObsSettings()))!.AsObject().ContainsKey("passwordDpapi"));
    }

    [Theory]
    [InlineData("127.0.0.1", true)]
    [InlineData("localhost", true)]
    [InlineData("::1", true)]
    [InlineData("[::1]", true)]
    [InlineData("192.168.1.20", true)]
    [InlineData("10.0.0.5", true)]
    [InlineData("172.20.1.1", true)]
    [InlineData("100.100.1.1", true)]
    [InlineData("172.32.0.1", false)]
    [InlineData("203.0.113.10", false)]
    [InlineData("obs.example.com", false)]
    [InlineData("192.168.1", false)]
    [InlineData("", false)]
    public void Obs_host_is_this_pc_or_private_network(string host, bool ok) => Assert.Equal(ok, ObsSettings.IsAllowedHost(host));

    [Fact]
    public void Notify_modes_hidden_means_absent()
    {
        var n = new NotifySettings();
        Assert.Equal(NotifySettings.ModeHidden, n.GetMode("Discord"));
        n.SetMode("Discord", NotifySettings.ModeCount);
        n.SetMode("TelegramDesktop", NotifySettings.ModeFull);
        Assert.Equal(NotifySettings.ModeCount, n.GetMode("Discord"));
        n.SetMode("Discord", NotifySettings.ModeHidden);
        Assert.Equal(new[] { "TelegramDesktop" }, n.Apps.Keys);
        n.SetMode("TelegramDesktop", "loud");
        Assert.Empty(n.Apps);
        n.Apps["X"] = "weird";
        Assert.Equal(NotifySettings.ModeHidden, n.GetMode("X"));

        var node = JsonNode.Parse(JsonDefaults.Serialize(new NotifySettings { Apps = { ["Discord"] = "title" } }))!.AsObject();
        Assert.Equal(new[] { "enabled", "wallCountsOnly", "nightFlash", "apps" }, node.Select(p => p.Key));
        Assert.Equal("title", node["apps"]!["Discord"]!.GetValue<string>());
        Assert.Equal(new[] { "hidden", "count", "title", "full" }, NotifySettings.Modes);
    }

    [Theory]
    [InlineData("aa-bb-cc-dd-ee-ff", "AA:BB:CC:DD:EE:FF")]
    [InlineData("AABBCCDDEEFF", "AA:BB:CC:DD:EE:FF")]
    [InlineData(" 0a:1b:2c:3d:4e:5f ", "0A:1B:2C:3D:4E:5F")]
    [InlineData("AA:BB-CC:DD:EE:FF", null)]
    [InlineData("AA:BB:CC:DD:EE", null)]
    [InlineData("GG:BB:CC:DD:EE:FF", null)]
    [InlineData("00:00:00:00:00:00", null)]
    [InlineData("FF:FF:FF:FF:FF:FF", null)]
    [InlineData("", null)]
    public void Mac_is_normalized(string input, string? expected) => Assert.Equal(expected, WolSettings.NormalizeMac(input));

    [Theory]
    [InlineData("192.168.1.255", "192.168.1.255")]
    [InlineData(" 255.255.255.255 ", "255.255.255.255")]
    [InlineData("192.168.1", null)]
    [InlineData("::1", null)]
    [InlineData("router.local", null)]
    [InlineData("300.1.1.1", null)]
    public void Broadcast_is_ipv4(string input, string? expected) => Assert.Equal(expected, WolSettings.NormalizeBroadcast(input));

    [Fact]
    public void Wol_is_created_only_when_both_fields_are_valid()
    {
        var w = WolSettings.Create("aa-bb-cc-dd-ee-ff", "192.168.1.255")!;
        Assert.Equal(("AA:BB:CC:DD:EE:FF", "192.168.1.255"), (w.Mac, w.Broadcast));
        Assert.Null(WolSettings.Create("aa-bb-cc-dd-ee-ff", ""));
        Assert.Null(WolSettings.Create("", "192.168.1.255"));
    }
}

public class ProgramScreenTests
{
    [Fact]
    public void For_app_round_trips_and_is_typed()
    {
        var l = JsonDefaults.Deserialize<PanelLayout>(Samples.SpecProgramScreenLayout)!;
        Assert.False(l.Screens[0].IsProgramScreen);
        Assert.Equal("obs64.exe", l.Screens[1].ForApp);
        Assert.True(l.Screens[1].IsProgramScreen);
        Assert.True(JsonNode.DeepEquals(JsonNode.Parse(Samples.SpecProgramScreenLayout), JsonNode.Parse(JsonDefaults.Serialize(l))));
        Assert.False(JsonNode.Parse(JsonDefaults.Serialize(l))!["screens"]![0]!.AsObject().ContainsKey("forApp"));
        Assert.Empty(LayoutValidator.Validate(l));
    }

    [Theory]
    [InlineData(@"C:\Program Files\obs-studio\bin\64bit\obs64.exe", "obs64.exe")]
    [InlineData("  \"OBS64.EXE\" ", "OBS64.EXE")]
    [InlineData("tools/app.exe", "app.exe")]
    [InlineData("   ", null)]
    [InlineData(null, null)]
    public void For_app_is_an_exe_name_without_path(string? input, string? expected) =>
        Assert.Equal(expected, LayoutScreen.NormalizeForApp(input));

    private static PanelLayout Layout(int normal, int program)
    {
        var l = new PanelLayout();
        for (int i = 0; i < normal; i++) l.Screens.Add(new LayoutScreen { Id = "n" + i, Name = "N" });
        for (int i = 0; i < program; i++) l.Screens.Add(new LayoutScreen { Id = "p" + i, Name = "P", ForApp = $"app{i}.exe" });
        return l;
    }

    [Fact]
    public void At_most_five_program_screens_and_one_normal_screen()
    {
        Assert.Empty(LayoutValidator.Validate(Layout(1, 5)));
        var six = LayoutValidator.Validate(Layout(1, 6));
        Assert.Equal(IssueKinds.ProgramScreens, Assert.Single(six).Kind);
        Assert.Equal("p5", six[0].ScreenId);
        Assert.Equal(IssueKinds.NoMainScreen, Assert.Single(LayoutValidator.Validate(Layout(0, 2))).Kind);
        Assert.Contains(LayoutValidator.Validate(Layout(5, 6)), i => i.Kind == IssueKinds.Screens);
    }

    [Fact]
    public void New_block_types_have_spec_defaults()
    {
        var expect = new Dictionary<string, (int W, int H, string Opts)>
        {
            ["power"] = (4, 1, """{"buttons":["lock","sleep","restart","shutdown"]}"""),
            ["keys"] = (4, 1, """{"actions":[]}"""),
            ["mixer"] = (3, 2, """{"output":true,"mic":true,"apps":true}"""),
            ["obs"] = (4, 2, """{"stream":true,"record":true,"scenes":true,"audio":true,"stats":true}"""),
            ["notify"] = (3, 2, """{"max":5}"""),
        };
        foreach (var (id, e) in expect)
        {
            var t = BlockTypes.Find(id)!;
            Assert.Equal((e.W, e.H), (t.DefaultW, t.DefaultH));
            Assert.True(JsonNode.DeepEquals(JsonNode.Parse(e.Opts), t.CreateDefaultOpts()), id);
            Assert.False(string.IsNullOrEmpty(t.Glyph));
        }
        Assert.Equal(BlockTypes.All.Count, BlockTypes.All.Select(t => t.Id).Distinct().Count());
    }
}
