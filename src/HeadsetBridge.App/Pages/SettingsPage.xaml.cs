using Microsoft.UI.Xaml.Controls;

namespace HeadsetBridge.Pages;

public sealed partial class SettingsPage : Page
{
    bool _loading = true;

    public SettingsPage()
    {
        InitializeComponent();
        ThemeBox.SelectedIndex = Settings.Theme switch { AppTheme.Light => 1, AppTheme.Dark => 2, _ => 0 };
        LanguageBox.SelectedIndex = Settings.Language switch { "de" => 1, "en" => 2, _ => 0 };
        _loading = false;
    }

    void ThemeBox_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_loading) return;
        var theme = ThemeBox.SelectedIndex switch { 1 => AppTheme.Light, 2 => AppTheme.Dark, _ => AppTheme.System };
        Settings.Theme = theme;
        App.Window?.ApplyTheme(theme);
    }

    void LanguageBox_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_loading || LanguageBox.SelectedItem is not ComboBoxItem { Tag: string tag }) return;
        if (tag == Settings.Language) return;
        Settings.Language = tag;
        // Asynchron, damit die ComboBox ihr Ereignis abschließen kann, bevor die Seite neu geladen wird.
        DispatcherQueue.TryEnqueue(() => App.Window?.ApplyLanguage());
    }
}
