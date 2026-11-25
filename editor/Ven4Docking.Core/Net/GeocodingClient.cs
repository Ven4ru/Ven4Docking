using System.Net.Http.Json;
using System.Text.Json.Serialization;
using Ven4Docking.Core.Models;

namespace Ven4Docking.Core.Net;

public sealed record GeoPlace(string Name, string? Admin1, string? Country, double Lat, double Lon)
{
    public string Display => string.Join(", ", new[] { Name, Admin1, Country }.Where(s => !string.IsNullOrWhiteSpace(s)));

    public WeatherPlace ToWeather() => new() { Name = Name, Lat = Math.Round(Lat, 4), Lon = Math.Round(Lon, 4) };
}

/// <summary>City search for the weather setting (open-meteo geocoding, no key, spec §10).</summary>
public sealed class GeocodingClient
{
    public const string Endpoint = "https://geocoding-api.open-meteo.com/v1/search";

    private readonly HttpClient _http;

    public GeocodingClient(HttpClient http) => _http = http;

    public async Task<IReadOnlyList<GeoPlace>> SearchAsync(string query, string language, CancellationToken ct = default)
    {
        query = query.Trim();
        if (query.Length < 2) return Array.Empty<GeoPlace>();
        string lang = language == "en" ? "en" : "ru";
        string url = $"{Endpoint}?name={Uri.EscapeDataString(query)}&count=10&language={lang}&format=json";
        var resp = await _http.GetFromJsonAsync<Response>(url, JsonDefaults.Options, ct).ConfigureAwait(false);
        return resp?.Results?.Select(r => new GeoPlace(r.Name ?? "", r.Admin1, r.Country, r.Latitude, r.Longitude)).ToList()
               ?? (IReadOnlyList<GeoPlace>)Array.Empty<GeoPlace>();
    }

    private sealed class Response
    {
        [JsonPropertyName("results")] public List<Result>? Results { get; set; }
    }

    private sealed class Result
    {
        [JsonPropertyName("name")] public string? Name { get; set; }
        [JsonPropertyName("latitude")] public double Latitude { get; set; }
        [JsonPropertyName("longitude")] public double Longitude { get; set; }
        [JsonPropertyName("country")] public string? Country { get; set; }
        [JsonPropertyName("admin1")] public string? Admin1 { get; set; }
    }
}
