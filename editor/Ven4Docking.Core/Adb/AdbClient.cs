namespace Ven4Docking.Core.Adb;

/// <summary>
/// Thin adb wrapper. Every per-device call goes through <c>-s serial</c>.
/// <para>
/// Safety: if <c>VEN4DOCKING_ADB_ONLY</c> is set (for example to <c>emulator-</c>), calls to any serial that
/// does not start with it are refused. Development runs set it so the owner's live wall phone is never touched.
/// </para>
/// </summary>
public sealed class AdbClient
{
    public const string SerialGuardVariable = "VEN4DOCKING_ADB_ONLY";

    private readonly IProcessRunner _runner;
    private readonly string? _serialPrefix;

    public AdbClient(string adbPath, IProcessRunner runner, string? serialPrefix = null)
    {
        AdbPath = adbPath;
        _runner = runner;
        _serialPrefix = string.IsNullOrWhiteSpace(serialPrefix) ? null : serialPrefix;
    }

    public static AdbClient FromEnvironment(string adbPath) =>
        new(adbPath, new ProcessRunner(), Environment.GetEnvironmentVariable(SerialGuardVariable));

    public string AdbPath { get; }

    public bool IsAvailable => File.Exists(AdbPath);

    public static readonly TimeSpan ShortTimeout = TimeSpan.FromSeconds(20);
    public static readonly TimeSpan InstallTimeout = TimeSpan.FromMinutes(3);

    public bool IsAllowed(string serial) => _serialPrefix is null || serial.StartsWith(_serialPrefix, StringComparison.Ordinal);

    public async Task<IReadOnlyList<AdbDevice>> DevicesAsync(CancellationToken ct = default)
    {
        var r = await _runner.RunAsync(AdbPath, new[] { "devices", "-l" }, ShortTimeout, ct).ConfigureAwait(false);
        return AdbDevicesParser.Parse(r.StdOut);
    }

    public Task<ProcessResult> RunAsync(string serial, AdbCommand command, TimeSpan? timeout = null, CancellationToken ct = default)
    {
        if (!IsAllowed(serial))
            throw new InvalidOperationException($"adb: serial {serial} is blocked by {SerialGuardVariable}");
        var args = new List<string> { "-s", serial };
        args.AddRange(command.Args);
        return _runner.RunAsync(AdbPath, args, timeout ?? ShortTimeout, ct);
    }

    /// <summary><c>adb pair host:port code</c> (Android 11+ wireless debugging).</summary>
    public Task<ProcessResult> PairAsync(string hostPort, string code, CancellationToken ct = default) =>
        _runner.RunAsync(AdbPath, new[] { "pair", hostPort, code }, ShortTimeout, ct);

    public Task<ProcessResult> ConnectAsync(string hostPort, CancellationToken ct = default)
    {
        if (!IsAllowed(hostPort))
            throw new InvalidOperationException($"adb: {hostPort} is blocked by {SerialGuardVariable}");
        return _runner.RunAsync(AdbPath, new[] { "connect", hostPort }, ShortTimeout, ct);
    }

    public async Task<DeviceInfo?> GetDeviceInfoAsync(string serial, CancellationToken ct = default)
    {
        var r = await RunAsync(serial, AdbCommand.Shell("getprop"), ct: ct).ConfigureAwait(false);
        return WizardCommandBuilder.ToDeviceInfo(WizardCommandBuilder.ParseGetprop(r.StdOut));
    }
}
