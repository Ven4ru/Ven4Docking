using System.ComponentModel;
using System.Diagnostics;
using Ven4Docking.Core.Models;

namespace Ven4Docking.Core.Admin;

public enum FirewallFixOutcome
{
    /// <summary>The agent exited with 0: the rules were recreated.</summary>
    Fixed,
    /// <summary>The user answered «Нет» in the UAC prompt (Win32 error 1223).</summary>
    Declined,
    Failed,
    AgentNotFound,
}

public sealed record FirewallFixResult(FirewallFixOutcome Outcome, int? ExitCode = null, string? Error = null);

/// <summary>Starts a program elevated and waits for it. Throws <see cref="Win32Exception"/> 1223 when UAC is declined.</summary>
public interface IElevatedRunner
{
    Task<int> RunAsync(ProcessStartInfo startInfo, CancellationToken ct);
}

public sealed class ShellElevatedRunner : IElevatedRunner
{
    public async Task<int> RunAsync(ProcessStartInfo startInfo, CancellationToken ct)
    {
        // ShellExecuteEx with the "runas" verb blocks the calling thread until the UAC prompt is answered,
        // so it never runs on the UI thread.
        using var process = await Task.Run(() => Process.Start(startInfo), ct).ConfigureAwait(false)
            ?? throw new InvalidOperationException("The process did not start.");
        await process.WaitForExitAsync(ct).ConfigureAwait(false);
        return process.ExitCode;
    }
}

/// <summary>
/// «Исправить»: <c>ven4docking-agent.exe --firewall add</c> run elevated recreates the inbound rule «Ven4Docking»
/// and removes the Block rules Windows created for the agent program (spec §13, installer step).
/// </summary>
public static class FirewallRepair
{
    public const string Arguments = "--firewall add";

    /// <summary>ERROR_CANCELLED: the user declined the UAC prompt.</summary>
    public const int ErrorCancelled = 1223;

    public static readonly TimeSpan Timeout = TimeSpan.FromMinutes(2);

    public static ProcessStartInfo StartInfo(string agentExe) => new(agentExe, Arguments)
    {
        UseShellExecute = true,
        Verb = "runas",
        WindowStyle = ProcessWindowStyle.Hidden,
        WorkingDirectory = Path.GetDirectoryName(agentExe) ?? "",
    };

    public static async Task<FirewallFixResult> RunAsync(string? agentExe, IElevatedRunner runner, CancellationToken ct = default)
    {
        if (string.IsNullOrEmpty(agentExe)) return new FirewallFixResult(FirewallFixOutcome.AgentNotFound);
        using var cts = CancellationTokenSource.CreateLinkedTokenSource(ct);
        cts.CancelAfter(Timeout);
        try
        {
            int code = await runner.RunAsync(StartInfo(agentExe), cts.Token).ConfigureAwait(false);
            return code == 0
                ? new FirewallFixResult(FirewallFixOutcome.Fixed, 0)
                : new FirewallFixResult(FirewallFixOutcome.Failed, code);
        }
        catch (Win32Exception e) when (e.NativeErrorCode == ErrorCancelled)
        {
            return new FirewallFixResult(FirewallFixOutcome.Declined);
        }
        catch (Win32Exception e)
        {
            return new FirewallFixResult(FirewallFixOutcome.Failed, Error: e.Message);
        }
        catch (InvalidOperationException e)
        {
            return new FirewallFixResult(FirewallFixOutcome.Failed, Error: e.Message);
        }
        catch (OperationCanceledException) when (!ct.IsCancellationRequested)
        {
            return new FirewallFixResult(FirewallFixOutcome.Failed, Error: "timeout");
        }
    }
}

/// <summary>
/// State behind the editor's firewall warning: what the agent reports, whether «Исправить» is running and how the
/// last attempt ended. UI-free so it is unit-tested; the editor maps it to localized texts.
/// </summary>
public sealed class FirewallNotice
{
    private readonly IElevatedRunner _runner;

    public FirewallNotice(IElevatedRunner? runner = null) => _runner = runner ?? new ShellElevatedRunner();

    public FirewallState State { get; private set; }
    public bool IsFixing { get; private set; }

    /// <summary>Outcome of the last «Исправить»; cleared when the problem goes away.</summary>
    public FirewallFixResult? LastResult { get; private set; }

    public bool ShowWarning => FirewallStates.NeedsFix(State);
    public bool CanFix => ShowWarning && !IsFixing;

    /// <summary>How often to re-read the status after a successful fix (the agent may need a moment).</summary>
    public int PollAttempts { get; init; } = 5;
    public TimeSpan PollDelay { get; init; } = TimeSpan.FromSeconds(1);

    public event Action? Changed;

    /// <summary>Agent offline (null) or an older agent without the field → <see cref="FirewallState.Unknown"/>, no warning.</summary>
    public void Update(AgentStatus? status)
    {
        var next = status?.FirewallState ?? FirewallState.Unknown;
        if (next == State) return;
        State = next;
        if (!ShowWarning) LastResult = null;
        Changed?.Invoke();
    }

    /// <param name="agentExe">Path of ven4docking-agent.exe, null when it was not found.</param>
    /// <param name="poll">Re-reads <c>GET /admin/status</c>.</param>
    public async Task<FirewallFixResult?> FixAsync(string? agentExe, Func<Task<AgentStatus?>> poll, CancellationToken ct = default)
    {
        if (!CanFix) return null;
        IsFixing = true;
        LastResult = null;
        Changed?.Invoke();
        try
        {
            var result = await FirewallRepair.RunAsync(agentExe, _runner, ct);
            if (result.Outcome == FirewallFixOutcome.Fixed)
            {
                for (int i = 0; i < PollAttempts; i++)
                {
                    Update(await poll());
                    if (!ShowWarning) break;
                    if (i + 1 < PollAttempts) await Task.Delay(PollDelay, ct);
                }
            }
            LastResult = ShowWarning ? result : null;
            return result;
        }
        finally
        {
            IsFixing = false;
            Changed?.Invoke();
        }
    }
}
