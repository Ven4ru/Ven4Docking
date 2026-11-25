using System.ComponentModel;
using System.Globalization;
using System.Resources;
using System.Windows.Data;
using System.Windows.Markup;

namespace Ven4Docking.Editor.Services;

/// <summary>
/// RU/EN strings from Resources/Strings.resx (neutral = Russian) and Strings.en.resx. XAML binds through the
/// indexer, so switching the language updates every label at once.
/// </summary>
public sealed class Loc : INotifyPropertyChanged
{
    private static readonly ResourceManager Resources =
        new("Ven4Docking.Editor.Resources.Strings", typeof(Loc).Assembly);

    public static Loc Instance { get; } = new();

    private CultureInfo _culture = CultureInfo.GetCultureInfo("ru");

    public event PropertyChangedEventHandler? PropertyChanged;

    /// <summary>"ru" or "en".</summary>
    public string Language => _culture.TwoLetterISOLanguageName == "en" ? "en" : "ru";

    public event Action? LanguageChanged;

    public void SetLanguage(string? language)
    {
        var next = CultureInfo.GetCultureInfo(language == "en" ? "en" : "ru");
        if (Equals(next, _culture)) return;
        _culture = next;
        CultureInfo.DefaultThreadCurrentUICulture = next;
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(Binding.IndexerName));
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(Language)));
        LanguageChanged?.Invoke();
    }

    public string this[string key] => Resources.GetString(key, _culture) ?? $"[{key}]";

    public static string T(string key) => Instance[key];

    public static string F(string key, params object?[] args) =>
        string.Format(CultureInfo.CurrentCulture, Instance[key], args);
}

/// <summary><c>{l:L Some_Key}</c> — a live, language-aware binding.</summary>
[MarkupExtensionReturnType(typeof(object))]
public sealed class L : MarkupExtension
{
    public L() { }

    public L(string key) => Key = key;

    [ConstructorArgument("key")]
    public string Key { get; set; } = "";

    public override object ProvideValue(IServiceProvider serviceProvider) =>
        new Binding($"[{Key}]") { Source = Loc.Instance, Mode = BindingMode.OneWay }.ProvideValue(serviceProvider);
}
