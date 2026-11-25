using System.ComponentModel;
using System.Diagnostics;
using System.Net;
using Ven4Docking.Core.Admin;
using Ven4Docking.Core.Models;

namespace Ven4Docking.Core.Tests;

public class FirewallTests
{
    private const string Exe = @"C:\Programs\Ven4Docking\ven4docking-agent.exe";

    private static async Task<AgentStatus?> StatusFrom(string json)
    {
        var h = new StubHandler((_, _) => StubHandler.Json(HttpStatusCode.OK, json));
        using var c = AdminClient.Create(18765, () => "k", h);
        return await c.TryGetStatusAsync();
    }

    [Theory]
    [InlineData("ok", FirewallState.Ok)]
    [InlineData("missing", FirewallState.Missing)]
    [InlineData("blocked", FirewallState.Blocked)]
    [InlineData("unknown", FirewallState.Unknown)]
    [InlineData("Blocked", FirewallState.Blocked)]
    [InlineData("something-new", FirewallState.Unknown)]
    public async Task Status_firewall_field_is_parsed(string value, FirewallState expected)
    {
        var s = await StatusFrom($$"""{"version":"1.0.0","port":18765,"activePanels":0,"firewall":"{{value}}"}""");
        Assert.Equal(value, s!.Firewall);
        Assert.Equal(expected, s.FirewallState);
    }

    [Fact]
    public async Task Older_agent_without_field_is_unknown_and_shows_nothing()
    {
        var s = await StatusFrom("""{"version":"0.9.0","port":18765,"cpuPct":0.1,"ramBytes":1,"activePanels":0,"blockErrors":{}}""");
        Assert.Null(s!.Firewall);
        Assert.Equal(FirewallState.Unknown, s.FirewallState);
        var n = new FirewallNotice(new FakeElevated());
        n.Update(s);
        Assert.False(n.ShowWarning);
        Assert.False(n.CanFix);
    }

    [Fact]
    public async Task Null_firewall_value_is_unknown()
    {
        var s = await StatusFrom("""{"version":"1.0.0","port":18765,"activePanels":0,"firewall":null}""");
        Assert.Equal(FirewallState.Unknown, s!.FirewallState);
    }

    [Theory]
    [InlineData(FirewallState.Ok, false)]
    [InlineData(FirewallState.Unknown, false)]
    [InlineData(FirewallState.Missing, true)]
    [InlineData(FirewallState.Blocked, true)]
    public void Warning_only_for_missing_and_blocked(FirewallState state, bool warn) =>
        Assert.Equal(warn, FirewallStates.NeedsFix(state));

    [Fact]
    public void Notice_follows_status_and_hides_when_agent_goes_offline()
    {
        var n = new FirewallNotice(new FakeElevated());
        int changes = 0;
        n.Changed += () => changes++;

        n.Update(Status("missing"));
        Assert.True(n.ShowWarning);
        Assert.True(n.CanFix);
        Assert.Equal(FirewallState.Missing, n.State);

        n.Update(Status("missing"));
        Assert.Equal(1, changes);

        n.Update(Status("blocked"));
        Assert.Equal(FirewallState.Blocked, n.State);

        n.Update(null);
        Assert.False(n.ShowWarning);
        Assert.Equal(3, changes);
    }

    [Fact]
    public void Start_info_runs_agent_elevated_with_firewall_add()
    {
        var psi = FirewallRepair.StartInfo(Exe);
        Assert.Equal(Exe, psi.FileName);
        Assert.Equal("--firewall add", psi.Arguments);
        Assert.True(psi.UseShellExecute);
        Assert.Equal("runas", psi.Verb);
        Assert.Equal(@"C:\Programs\Ven4Docking", psi.WorkingDirectory);
    }

    [Fact]
    public async Task Fix_success_repolls_until_the_warning_goes_away()
    {
        var runner = new FakeElevated { Exit = 0 };
        var n = new FirewallNotice(runner) { PollDelay = TimeSpan.Zero };
        n.Update(Status("missing"));
        var answers = new Queue<string>(new[] { "missing", "ok" });
        int polls = 0;

        var r = await n.FixAsync(Exe, () => { polls++; return Task.FromResult<AgentStatus?>(Status(answers.Dequeue())); });

        Assert.Equal(FirewallFixOutcome.Fixed, r!.Outcome);
        Assert.Equal(2, polls);
        Assert.Equal(Exe, runner.Started.Single().FileName);
        Assert.False(n.ShowWarning);
        Assert.False(n.IsFixing);
        Assert.Null(n.LastResult);
    }

    [Fact]
    public async Task Fix_success_but_agent_still_reports_missing_keeps_warning_with_result()
    {
        var n = new FirewallNotice(new FakeElevated { Exit = 0 }) { PollDelay = TimeSpan.Zero, PollAttempts = 3 };
        n.Update(Status("blocked"));
        int polls = 0;

        await n.FixAsync(Exe, () => { polls++; return Task.FromResult<AgentStatus?>(Status("blocked")); });

        Assert.Equal(3, polls);
        Assert.True(n.ShowWarning);
        Assert.Equal(FirewallFixOutcome.Fixed, n.LastResult!.Outcome);
        Assert.True(n.CanFix);
    }

    [Fact]
    public async Task Declined_uac_does_not_throw_and_keeps_the_warning()
    {
        var runner = new FakeElevated { Throw = new Win32Exception(FirewallRepair.ErrorCancelled) };
        var n = new FirewallNotice(runner);
        n.Update(Status("missing"));
        int polls = 0;

        var r = await n.FixAsync(Exe, () => { polls++; return Task.FromResult<AgentStatus?>(null); });

        Assert.Equal(FirewallFixOutcome.Declined, r!.Outcome);
        Assert.Equal(0, polls);
        Assert.True(n.ShowWarning);
        Assert.True(n.CanFix);
        Assert.Equal(FirewallFixOutcome.Declined, n.LastResult!.Outcome);
    }

    [Fact]
    public async Task Nonzero_exit_and_other_errors_are_failures()
    {
        var r1 = await FirewallRepair.RunAsync(Exe, new FakeElevated { Exit = 5 });
        Assert.Equal((FirewallFixOutcome.Failed, (int?)5), (r1.Outcome, r1.ExitCode));

        var r2 = await FirewallRepair.RunAsync(Exe, new FakeElevated { Throw = new Win32Exception(2) });
        Assert.Equal(FirewallFixOutcome.Failed, r2.Outcome);
        Assert.NotNull(r2.Error);

        var r3 = await FirewallRepair.RunAsync(null, new FakeElevated());
        Assert.Equal(FirewallFixOutcome.AgentNotFound, r3.Outcome);
    }

    [Fact]
    public async Task Fix_is_ignored_when_there_is_nothing_to_fix_or_already_running()
    {
        var gate = new TaskCompletionSource<int>();
        var runner = new FakeElevated { Pending = gate.Task };
        var n = new FirewallNotice(runner) { PollDelay = TimeSpan.Zero };

        Assert.Null(await n.FixAsync(Exe, () => Task.FromResult<AgentStatus?>(null)));
        Assert.Empty(runner.Started);

        n.Update(Status("missing"));
        var first = n.FixAsync(Exe, () => Task.FromResult<AgentStatus?>(Status("ok")));
        Assert.True(n.IsFixing);
        Assert.False(n.CanFix);
        Assert.Null(await n.FixAsync(Exe, () => Task.FromResult<AgentStatus?>(null)));

        gate.SetResult(0);
        Assert.Equal(FirewallFixOutcome.Fixed, (await first)!.Outcome);
        Assert.Single(runner.Started);
        Assert.False(n.IsFixing);
    }

    private static AgentStatus Status(string firewall) => new() { Version = "1.0.0", Port = 18765, Firewall = firewall };

    private sealed class FakeElevated : IElevatedRunner
    {
        public int Exit { get; init; }
        public Exception? Throw { get; init; }
        public Task<int>? Pending { get; init; }
        public List<ProcessStartInfo> Started { get; } = new();

        public Task<int> RunAsync(ProcessStartInfo startInfo, CancellationToken ct)
        {
            Started.Add(startInfo);
            if (Throw is not null) return Task.FromException<int>(Throw);
            return Pending ?? Task.FromResult(Exit);
        }
    }
}
