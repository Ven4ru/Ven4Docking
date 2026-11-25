using Ven4Docking.Core.Adb;

namespace Ven4Docking.Core.Tests;

public class AdbDevicesParserTests
{
    [Fact]
    public void Parses_all_device_kinds_and_skips_daemon_lines()
    {
        var d = AdbDevicesParser.Parse(Samples.AdbDevices);
        Assert.Equal(6, d.Count);

        Assert.Equal("emulator-5554", d[0].Serial);
        Assert.True(d[0].IsReady);
        Assert.True(d[0].IsEmulator);
        Assert.Equal("sdk_gphone64_x86_64", d[0].Model);
        Assert.Equal("2", d[0].TransportId);

        Assert.True(d[1].IsUnauthorized);
        Assert.Equal("1-4", d[1].Usb);
        Assert.Null(d[1].Model);
        Assert.False(d[1].IsWifi);

        Assert.Equal("192.168.1.50:5555", d[2].Serial);
        Assert.True(d[2].IsWifi);
        Assert.Equal("RMX2185", d[2].Model);

        Assert.True(d[3].IsWifi);
        Assert.Equal("SM A525F", d[3].DisplayName);

        Assert.Equal("offline", d[4].State);
        Assert.Equal("no", d[5].State);
        Assert.Equal("1-2", d[5].Usb);
    }

    [Fact]
    public void Empty_list_and_crlf()
    {
        Assert.Empty(AdbDevicesParser.Parse("List of devices attached\r\n\r\n"));
        var one = AdbDevicesParser.Parse("List of devices attached\r\nemulator-5556\tdevice\r\n");
        Assert.Equal("emulator-5556", Assert.Single(one).Serial);
    }
}

public class WizardCommandTests
{
    private static IReadOnlyList<WizardStep> Steps(int sdk) => WizardCommandBuilder.BuildSetup(sdk, "Europe/Moscow");

    private static IEnumerable<string> AllCommandText(int sdk) =>
        Steps(sdk).SelectMany(s => s.Commands.Concat(s.Checks.Select(c => c.Command))).Select(c => c.ToString());

    private static WizardStep Step(int sdk, string id) => Steps(sdk).Single(s => s.Id == id);

    [Fact]
    public void Android7_uses_RUN_IN_BACKGROUND_and_no_locksettings()
    {
        var all = AllCommandText(AndroidSdk.Android7).ToList();
        Assert.Contains(all, c => c.Contains(" RUN_IN_BACKGROUND allow"));
        Assert.DoesNotContain(all, c => c.Contains("RUN_ANY_IN_BACKGROUND"));
        Assert.DoesNotContain(all, c => c.Contains("locksettings"));
        Assert.DoesNotContain(all, c => c.Contains("set-standby-bucket"));
        Assert.DoesNotContain(all, c => c.Contains("add-role-holder"));
        var noLock = Step(AndroidSdk.Android7, "noLock");
        Assert.True(noLock.ManualOnly);
        Assert.Empty(noLock.Commands);
        Assert.Equal("Wizard_Manual_NoLock", noLock.ManualHintKey);
        Assert.Contains(all, c => c == "shell settings put global stay_on_while_plugged_in 7");
    }

    [Fact]
    public void Android8_uses_RUN_IN_BACKGROUND_with_locksettings()
    {
        var all = AllCommandText(AndroidSdk.Android8).ToList();
        Assert.Contains(all, c => c.Contains(" RUN_IN_BACKGROUND allow"));
        Assert.DoesNotContain(all, c => c.Contains("RUN_ANY_IN_BACKGROUND"));
        var noLock = Step(AndroidSdk.Android8, "noLock");
        Assert.False(noLock.ManualOnly);
        Assert.Equal("shell locksettings set-disabled true", Assert.Single(noLock.Commands).ToString());
        Assert.Equal("shell locksettings get-disabled", Assert.Single(noLock.Checks).Command.ToString());
        Assert.DoesNotContain(all, c => c.Contains("set-standby-bucket"));
    }

    [Fact]
    public void Android10_uses_RUN_ANY_IN_BACKGROUND_role_home_and_standby_bucket()
    {
        var all = AllCommandText(AndroidSdk.Android10).ToList();
        Assert.Contains("shell cmd appops set ru.ven4tools.docking RUN_ANY_IN_BACKGROUND allow", all);
        Assert.DoesNotContain(all, c => c.Contains(" RUN_IN_BACKGROUND"));
        Assert.Contains("shell locksettings set-disabled true", all);
        Assert.Contains("shell am set-standby-bucket ru.ven4tools.docking active", all);
        var home = Step(AndroidSdk.Android10, "home");
        Assert.Equal(new[]
        {
            "shell cmd package set-home-activity ru.ven4tools.docking/.MainActivity",
            "shell cmd role add-role-holder android.app.role.HOME ru.ven4tools.docking",
        }, home.Commands.Select(c => c.ToString()));
        Assert.Contains(all, c => c == "shell settings put global stay_on_while_plugged_in 7");
    }

    [Fact]
    public void Android14_uses_RUN_ANY_IN_BACKGROUND_and_dock_bit()
    {
        var all = AllCommandText(AndroidSdk.Android14).ToList();
        Assert.Contains("shell cmd appops set ru.ven4tools.docking RUN_ANY_IN_BACKGROUND allow", all);
        Assert.Contains("shell locksettings set-disabled true", all);
        Assert.Contains("shell settings put global stay_on_while_plugged_in 15", all);
        var check = Step(AndroidSdk.Android14, "stayOn").Checks.Single();
        Assert.True(check.IsOk("15\n"));
        Assert.False(check.IsOk("7"));
    }

    [Theory]
    [InlineData(AndroidSdk.Android7)]
    [InlineData(AndroidSdk.Android8)]
    [InlineData(AndroidSdk.Android9)]
    [InlineData(AndroidSdk.Android10)]
    [InlineData(AndroidSdk.Android11)]
    [InlineData(AndroidSdk.Android14)]
    public void Every_automatic_step_has_a_check_and_common_items(int sdk)
    {
        var steps = Steps(sdk);
        Assert.Equal(new[] { "home", "stayOn", "timeout", "noLock", "background", "timeZone", "animations" }, steps.Select(s => s.Id));
        foreach (var s in steps.Where(s => !s.ManualOnly))
        {
            Assert.NotEmpty(s.Commands);
            Assert.NotEmpty(s.Checks);
            Assert.All(s.Commands, c => Assert.Equal("shell", c.Args[0]));
        }
        Assert.Contains(AllCommandText(sdk), c => c == "shell settings put system screen_off_timeout 1800000");
        Assert.Contains(AllCommandText(sdk), c => c == "shell settings put global animator_duration_scale 0.5");
    }

    [Fact]
    public void Wifi_pairing_only_on_11_plus()
    {
        Assert.False(WizardCommandBuilder.SupportsWifiPairing(AndroidSdk.Android10));
        Assert.True(WizardCommandBuilder.SupportsWifiPairing(AndroidSdk.Android11));
    }

    [Fact]
    public void Launch_passes_url_and_token_as_extras()
    {
        var cmd = WizardCommandBuilder.Launch("http://192.168.1.10:8765/", "tok123");
        Assert.Equal(new[]
        {
            "shell", "am", "start", "-n", "ru.ven4tools.docking/.MainActivity",
            "--es", "url", "http://192.168.1.10:8765/", "--es", "token", "tok123",
        }, cmd.Args);
    }

    [Fact]
    public void WebView_version_is_parsed()
    {
        var wv = WizardCommandBuilder.ParseWebView(Samples.WebViewDumpsys);
        Assert.Equal(("com.google.android.webview", "80.0.3987.99"), wv);
        Assert.Equal(80, WizardCommandBuilder.MajorVersion(wv!.Value.Version));
        Assert.Equal(51, WizardCommandBuilder.MajorVersion("51.0.2704.81"));
        Assert.Null(WizardCommandBuilder.ParseWebView("nothing"));
    }

    [Fact]
    public void Permission_denial_is_recognised()
    {
        Assert.True(WizardCommandBuilder.IsPermissionDenied(
            "Exception occurred while executing 'put':\njava.lang.SecurityException: Permission denial: writing to settings requires:android.permission.WRITE_SECURE_SETTINGS"));
        Assert.False(WizardCommandBuilder.IsPermissionDenied(""));
        Assert.True(WizardCommandBuilder.LooksLikeError("Failure [INSTALL_FAILED_USER_RESTRICTED: Install canceled by user]"));
        Assert.False(WizardCommandBuilder.LooksLikeError("Success"));
    }

    [Fact]
    public void Getprop_is_parsed_into_device_info()
    {
        const string props = "[ro.build.version.release]: [10]\n[ro.build.version.sdk]: [29]\n[ro.product.brand]: [realme]\n[ro.product.manufacturer]: [realme]\n[ro.product.model]: [RMX2185]\n";
        var info = WizardCommandBuilder.ToDeviceInfo(WizardCommandBuilder.ParseGetprop(props))!;
        Assert.Equal(new DeviceInfo("realme", "realme", "RMX2185", "10", 29), info);
        Assert.Null(WizardCommandBuilder.ToDeviceInfo(new Dictionary<string, string>()));
    }
}

public class AdbClientTests
{
    [Fact]
    public async Task Per_device_commands_use_serial_and_guard_blocks_other_phones()
    {
        var runner = new FakeRunner();
        var adb = new AdbClient(@"C:\pt\adb.exe", runner, serialPrefix: "emulator-");

        await adb.RunAsync("emulator-5554", AdbCommand.Shell("getprop", "ro.build.version.sdk"));
        Assert.Equal(new[] { "-s", "emulator-5554", "shell", "getprop", "ro.build.version.sdk" }, runner.Calls.Single().Args);

        await Assert.ThrowsAsync<InvalidOperationException>(() => adb.RunAsync("REALPHONE123", AdbCommand.Shell("id")));
        await Assert.ThrowsAsync<InvalidOperationException>(() => adb.ConnectAsync("192.168.1.5:5555"));
        Assert.Single(runner.Calls); // nothing reached the process runner
    }

    [Fact]
    public async Task Devices_runs_devices_l()
    {
        var runner = new FakeRunner { Respond = _ => new ProcessResult(0, Samples.AdbDevices, "") };
        var adb = new AdbClient("adb.exe", runner);
        var list = await adb.DevicesAsync();
        Assert.Equal(new[] { "devices", "-l" }, runner.Calls.Single().Args);
        Assert.Equal(6, list.Count);
    }

    [Fact]
    public async Task Pair_uses_host_port_and_code()
    {
        var runner = new FakeRunner();
        await new AdbClient("adb.exe", runner).PairAsync("192.168.1.5:37000", "123456");
        Assert.Equal(new[] { "pair", "192.168.1.5:37000", "123456" }, runner.Calls.Single().Args);
    }
}
