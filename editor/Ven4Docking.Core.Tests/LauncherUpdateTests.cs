using Ven4Docking.Core.Adb;

namespace Ven4Docking.Core.Tests;

public class LauncherUpdateTests
{
    private const string DumpsysPackage = """
        Activity Resolver Table:
          Non-Data Actions:
              android.intent.action.MAIN:
                5f2a1b3 ru.ven4tools.docking/.MainActivity filter 8c1d2e4
        Packages:
          Package [ru.ven4tools.docking] (a1b2c3d):
            userId=10143
            pkg=Package{e5f6a7b ru.ven4tools.docking}
            versionCode=11 minSdk=24 targetSdk=34
            versionName=1.1.1
            splits=[base]
        """;

    [Fact]
    public void Commands_for_the_update_are_built_like_the_wizard_ones()
    {
        Assert.Equal(new[] { "install", "-r", @"C:\p\apk\ven4docking.apk" }, WizardCommandBuilder.Install(@"C:\p\apk\ven4docking.apk").Args);
        Assert.Equal("shell am start -n ru.ven4tools.docking/.MainActivity", WizardCommandBuilder.Start().ToString());
        Assert.Equal("shell dumpsys package ru.ven4tools.docking", WizardCommandBuilder.PackageInfoQuery().ToString());
    }

    [Fact]
    public void Version_name_is_read_from_dumpsys_package()
    {
        Assert.Equal("1.1.1", WizardCommandBuilder.ParseVersionName(DumpsysPackage));
        Assert.Equal("1.0.0-debug", WizardCommandBuilder.ParseVersionName("    versionName=1.0.0-debug\r\n"));
        Assert.Null(WizardCommandBuilder.ParseVersionName("Unable to find package: ru.ven4tools.docking"));
        Assert.Null(WizardCommandBuilder.ParseVersionName(""));
    }

    [Theory]
    [InlineData("1.1.0", "1.1.1", true)]
    [InlineData("1.1.1", "1.1.1", false)]
    [InlineData("1.2.0", "1.1.1", false)]
    [InlineData("1.0.0-debug", "1.1.1", true)]
    [InlineData("v1.1.1", "1.1.1", false)]
    [InlineData(null, "1.1.1", true)]
    [InlineData("", "1.1.1", true)]
    [InlineData("1.1.1", null, true)]
    [InlineData("garbage", "1.1.1", true)]
    public void Update_is_offered_for_older_or_unknown_versions(string? phone, string? apk, bool offered) =>
        Assert.Equal(offered, LauncherApk.IsUpdateOffered(phone, apk));

    [Fact]
    public void Apk_version_comes_from_latest_json_next_to_it()
    {
        using var tmp = new TempDir();
        string apk = Path.Combine(tmp.Path, "ven4docking.apk");
        Assert.Null(LauncherApk.Locate(apk));
        Assert.Null(LauncherApk.Locate(null));

        File.WriteAllText(apk, "apk");
        Assert.Equal(new LauncherApk(apk, null, null), LauncherApk.Locate(apk));

        // as build.ps1 writes it
        File.WriteAllText(Path.Combine(tmp.Path, "latest.json"),
            """{"version":"1.1.1","versionCode":11,"url":"/apk/ven4docking.apk","sha256":"ab"}""");
        Assert.Equal(new LauncherApk(apk, "1.1.1", 11), LauncherApk.Locate(apk));

        File.WriteAllText(Path.Combine(tmp.Path, "latest.json"), "{ broken");
        Assert.Equal(new LauncherApk(apk, null, null), LauncherApk.Locate(apk));
    }

    [Fact]
    public void Device_is_picked_when_there_is_one_or_the_model_matches()
    {
        var a = new AdbDevice("R58M1", "device", Model: "SM_A525F");
        var b = new AdbDevice("192.168.1.50:5555", "device", Model: "RMX2185");
        var off = new AdbDevice("X1", "unauthorized");
        Assert.Equal(a, LauncherInstaller.PickDevice(new[] { a, off }, null));
        Assert.Equal(b, LauncherInstaller.PickDevice(new[] { a, b }, "RMX2185"));
        Assert.Equal(a, LauncherInstaller.PickDevice(new[] { a, b }, "sm a525f"));
        Assert.Null(LauncherInstaller.PickDevice(new[] { a, b }, null));
        Assert.Null(LauncherInstaller.PickDevice(new[] { a, b }, "Pixel 7"));
        Assert.Null(LauncherInstaller.PickDevice(new[] { off }, "RMX2185"));
    }

    [Theory]
    [InlineData("Performing Streamed Install\nadb: failed to install x.apk: Failure [INSTALL_FAILED_UPDATE_INCOMPATIBLE: Package ru.ven4tools.docking signatures do not match]", AdbErrors.SignatureMismatch)]
    [InlineData("Failure [INSTALL_FAILED_VERSION_DOWNGRADE]", AdbErrors.Downgrade)]
    [InlineData("Failure [INSTALL_FAILED_INSUFFICIENT_STORAGE]", AdbErrors.Storage)]
    [InlineData("Failure [INSTALL_FAILED_USER_RESTRICTED: Install canceled by user]", AdbErrors.UserRestricted)]
    [InlineData("Failure [INSTALL_FAILED_ABORTED: User rejected permissions]", AdbErrors.Aborted)]
    [InlineData("adb: device unauthorized.\nThis adb server's $ADB_VENDOR_KEYS is not set", AdbErrors.Unauthorized)]
    [InlineData("adb: device offline", AdbErrors.Offline)]
    [InlineData("adb: device 'R58M1' not found", AdbErrors.NotFound)]
    [InlineData("timeout", AdbErrors.Timeout)]
    [InlineData("java.lang.SecurityException: Permission denial", AdbErrors.PermissionDenied)]
    [InlineData("Failure [INSTALL_FAILED_INVALID_APK]", AdbErrors.Other)]
    [InlineData("Performing Streamed Install\nSuccess\n", AdbErrors.None)]
    public void Adb_errors_are_classified(string output, string kind) => Assert.Equal(kind, AdbErrors.Classify(output));

    private static (AdbClient Adb, FakeRunner Runner) Adb(Func<IReadOnlyList<string>, ProcessResult> respond)
    {
        var runner = new FakeRunner { Respond = respond };
        return (new AdbClient(@"C:\pt\adb.exe", runner), runner);
    }

    [Fact]
    public async Task Install_starts_the_launcher_and_reads_its_version()
    {
        var (adb, runner) = Adb(args => args[2] switch
        {
            "install" => new ProcessResult(0, "Performing Streamed Install\nSuccess\n", ""),
            _ when args.Contains("dumpsys") => new ProcessResult(0, DumpsysPackage, ""),
            _ => new ProcessResult(0, "Starting: Intent { cmp=ru.ven4tools.docking/.MainActivity }", ""),
        });
        var r = await LauncherInstaller.InstallAsync(adb, "R58M1", new LauncherApk(@"C:\p\apk\ven4docking.apk", "1.1.1", 11));
        Assert.True(r.Ok);
        Assert.Equal("1.1.1", r.InstalledVersion);
        Assert.Equal(new[]
        {
            "-s R58M1 install -r C:\\p\\apk\\ven4docking.apk",
            "-s R58M1 shell am start -n ru.ven4tools.docking/.MainActivity",
            "-s R58M1 shell dumpsys package ru.ven4tools.docking",
        }, runner.Calls.Select(c => string.Join(' ', c.Args)));
    }

    [Fact]
    public async Task Failed_install_stops_and_explains()
    {
        var (adb, runner) = Adb(_ => new ProcessResult(1, "Performing Streamed Install\n",
            "adb: failed to install x.apk: Failure [INSTALL_FAILED_UPDATE_INCOMPATIBLE: signatures do not match]"));
        var r = await LauncherInstaller.InstallAsync(adb, "R58M1", new LauncherApk("x.apk", null, null));
        Assert.False(r.Ok);
        Assert.Equal(AdbErrors.SignatureMismatch, r.ErrorKind);
        Assert.StartsWith("adb: failed to install", r.Detail);
        Assert.Single(runner.Calls);

        var (adb2, _) = Adb(_ => new ProcessResult(0, "", ""));
        var silent = await LauncherInstaller.InstallAsync(adb2, "R58M1", new LauncherApk("x.apk", null, null));
        Assert.Equal((false, AdbErrors.Other), (silent.Ok, silent.ErrorKind));
    }

    [Fact]
    public async Task Serial_guard_still_protects_other_phones()
    {
        var runner = new FakeRunner();
        var adb = new AdbClient("adb.exe", runner, serialPrefix: "emulator-");
        await Assert.ThrowsAsync<InvalidOperationException>(() =>
            LauncherInstaller.InstallAsync(adb, "REALPHONE", new LauncherApk("x.apk", "1.1.1", 11)));
        Assert.Empty(runner.Calls);
    }
}
