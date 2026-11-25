using System.Net;
using System.Text;
using Ven4Docking.Core.Adb;

namespace Ven4Docking.Core.Tests;

internal sealed class StubHandler : HttpMessageHandler
{
    private readonly Func<HttpRequestMessage, string?, HttpResponseMessage> _respond;

    public StubHandler(Func<HttpRequestMessage, string?, HttpResponseMessage> respond) => _respond = respond;

    public List<(HttpRequestMessage Request, string? Body)> Calls { get; } = new();

    protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
    {
        string? body = request.Content is null ? null : await request.Content.ReadAsStringAsync(cancellationToken);
        Calls.Add((request, body));
        return _respond(request, body);
    }

    public static HttpResponseMessage Json(HttpStatusCode code, string json) =>
        new(code) { Content = new StringContent(json, Encoding.UTF8, "application/json") };
}

internal sealed class FakeRunner : IProcessRunner
{
    public List<(string Exe, IReadOnlyList<string> Args)> Calls { get; } = new();
    public Func<IReadOnlyList<string>, ProcessResult> Respond { get; set; } = _ => new ProcessResult(0, "", "");

    public Task<ProcessResult> RunAsync(string exe, IReadOnlyList<string> args, TimeSpan timeout, CancellationToken ct = default)
    {
        Calls.Add((exe, args));
        return Task.FromResult(Respond(args));
    }
}

internal sealed class TempDir : IDisposable
{
    public TempDir()
    {
        Path = System.IO.Path.Combine(System.IO.Path.GetTempPath(), "v4d-tests-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(Path);
    }

    public string Path { get; }

    public void Dispose()
    {
        try { Directory.Delete(Path, recursive: true); } catch (IOException) { }
    }
}
