using HeadsetBridge.Core;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.ApplicationModel.DataTransfer;

namespace HeadsetBridge.Pages;

public sealed class DiagnosticItem
{
    public required string Time { get; init; }
    public required string Message { get; init; }
    public required string Glyph { get; init; }
    public required string LevelName { get; init; }
    public required Brush Foreground { get; init; }
}

public sealed partial class DiagnosticsPage : Page
{
    public DiagnosticsPage()
    {
        InitializeComponent();
        Loaded += (_, _) => { Services.StatusChanged += Update; Update(Services.Status); };
        Unloaded += (_, _) => Services.StatusChanged -= Update;
        ActualThemeChanged += (_, _) => Update(Services.Status);
    }

    void Update(StatusSnapshot s)
    {
        StateText.Text = s.Title;
        DeviceText.Text = s.DisplayDevice;
        var items = s.Diagnostics.Select(ToItem).ToList();
        EventList.ItemsSource = items;
        EventList.Visibility = items.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
        EmptyState.Visibility = items.Count > 0 ? Visibility.Collapsed : Visibility.Visible;
        CopyButton.IsEnabled = items.Count > 0;
    }

    DiagnosticItem ToItem(Diagnostic d)
    {
        var (glyph, name, key) = d.Level switch
        {
            DiagnosticLevel.Success => ("\uEC61", L.T("Diag.Level.Success"), "SystemFillColorSuccessBrush"),
            DiagnosticLevel.Warning => ("\uE7BA", L.T("Diag.Level.Warning"), "SystemFillColorCautionBrush"),
            DiagnosticLevel.Error => ("\uEB90", L.T("Diag.Level.Error"), "SystemFillColorCriticalBrush"),
            _ => ("\uE946", L.T("Diag.Level.Info"), "TextFillColorSecondaryBrush"),
        };
        return new DiagnosticItem { Time = d.Time, Message = d.Message, Glyph = glyph, LevelName = name, Foreground = ThemeBrush(key) };
    }

    /// <summary>Holt einen Theme-Pinsel passend zum tatsächlichen Design dieser Seite (auch bei manuell gewähltem Hell/Dunkel).</summary>
    Brush ThemeBrush(string key)
    {
        var themeKey = ActualTheme == ElementTheme.Light ? "Light" : "Default";
        foreach (var dict in Application.Current.Resources.MergedDictionaries)
        {
            if (dict.ThemeDictionaries.TryGetValue(themeKey, out var t) && t is ResourceDictionary rd && rd.TryGetValue(key, out var b) && b is Brush brush)
                return brush;
        }
        return (Brush)Application.Current.Resources[key];
    }

    async void CopyButton_Click(object sender, RoutedEventArgs e)
    {
        var s = Services.Status;
        var lines = new List<string>
        {
            L.F("Diag.CopyHeader", Services.Version),
            L.F("Diag.CopyStatus", s.Title),
            L.F("Diag.CopyOutput", s.DisplayDevice),
            "",
        };
        lines.AddRange(s.Diagnostics.Select(r => $"{r.Time}  {r.Message}"));

        var package = new DataPackage();
        package.SetText(string.Join("\r\n", lines));
        Clipboard.SetContent(package);

        CopyGlyph.Glyph = "\uE73E";
        CopyText.Text = L.T("Diag.Copied");
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(CopyButton, CopyText.Text);
        await Task.Delay(2200);
        CopyGlyph.Glyph = "\uE8C8";
        CopyText.Text = L.T("Diag.Copy");
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(CopyButton, CopyText.Text);
    }
}
