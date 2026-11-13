using System.Text.Json;
using HeadsetBridge.Core;

namespace HeadsetBridge;

public enum AppTheme { System, Light, Dark }

/// <summary>Gemeinsamer Zustand der App: Engine, aktueller Status, Lizenzen.</summary>
public static class Services
{
    public const string Version = "2.0.0";

    public static readonly string DataDirectory = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "HeadsetBridge");

    /// <summary>Vorschau-Modus (<c>--demo</c>): simulierte Verbindung, nichts wird gestartet.</summary>
    public static bool DemoMode { get; } = Environment.GetCommandLineArgs().Contains("--demo", StringComparer.OrdinalIgnoreCase);

    public static IEngine Engine { get; } = DemoMode
        ? new DemoEngine()
        : new EngineController(Path.Combine(AppContext.BaseDirectory, "engine", "headsetbridge-engine.exe"));

    public static StatusSnapshot Status { get; private set; } = new();

    public static event Action<StatusSnapshot>? StatusChanged;

    public static void Refresh(bool force = false)
    {
        var next = Engine.Snapshot();
        if (!force && next.SameAs(Status)) return;
        Status = next;
        StatusChanged?.Invoke(next);
    }

    static (string Lang, IReadOnlyList<License> Items)? _licenses;

    /// <summary>Lizenztexte; der Name des ersten Abschnitts hängt von der Sprache ab.</summary>
    public static IReadOnlyList<License> Licenses
    {
        get
        {
            if (_licenses is { } c && c.Lang == L.Language) return c.Items;
            var path = Path.Combine(AppContext.BaseDirectory, "Lizenzhinweise.txt");
            var items = File.Exists(path) ? LicenseParser.Parse(File.ReadAllText(path)) : Array.Empty<License>();
            _licenses = (L.Language, items);
            return items;
        }
    }

    /// <summary>Wert einer Startoption <c>--name=wert</c> (überschreibt die gespeicherte Einstellung für diesen Start).</summary>
    public static string? Option(string name) =>
        Environment.GetCommandLineArgs().Select(a => a.StartsWith($"--{name}=", StringComparison.OrdinalIgnoreCase) ? a[(name.Length + 3)..] : null)
            .FirstOrDefault(v => v is not null);

    public static void ApplyLanguageSetting() =>
        L.SetLanguage(Option("lang") ?? (Settings.Language == "system" ? L.SystemLanguage : Settings.Language));

    public static AppTheme StartTheme => Option("theme") switch
    {
        "light" => AppTheme.Light,
        "dark" => AppTheme.Dark,
        _ => Settings.Theme,
    };
}

/// <summary>Einstellungen unter %LOCALAPPDATA%\HeadsetBridge\settings.json.</summary>
public static class Settings
{
    static readonly string FilePath = Path.Combine(Services.DataDirectory, "settings.json");

    sealed class Data
    {
        public string Theme { get; set; } = "system";
        public string Language { get; set; } = "system";
    }

    static Data Load()
    {
        try { return JsonSerializer.Deserialize<Data>(File.ReadAllText(FilePath)) ?? new(); }
        catch { return new(); }
    }

    static void Save(Data d)
    {
        try
        {
            Directory.CreateDirectory(Services.DataDirectory);
            File.WriteAllText(FilePath, JsonSerializer.Serialize(d));
        }
        catch { /* Einstellungen sind Komfort, kein Fehlerfall */ }
    }

    public static AppTheme Theme
    {
        get => Load().Theme switch { "light" => AppTheme.Light, "dark" => AppTheme.Dark, _ => AppTheme.System };
        set
        {
            var d = Load();
            d.Theme = value switch { AppTheme.Light => "light", AppTheme.Dark => "dark", _ => "system" };
            Save(d);
        }
    }

    /// <summary>"system", "de" oder "en".</summary>
    public static string Language
    {
        get => Load().Language is "de" or "en" ? Load().Language : "system";
        set
        {
            var d = Load();
            d.Language = value is "de" or "en" ? value : "system";
            Save(d);
        }
    }
}
