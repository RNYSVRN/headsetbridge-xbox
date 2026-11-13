using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace HeadsetBridge;

/// <summary>Zeile im Stil der Windows-11-Einstellungen: Symbol, Titel, Beschreibung, Inhalt rechts.</summary>
public sealed class SettingsRow : ContentControl
{
    public SettingsRow() => DefaultStyleKey = typeof(SettingsRow);

    public static readonly DependencyProperty GlyphProperty =
        DependencyProperty.Register(nameof(Glyph), typeof(string), typeof(SettingsRow), new PropertyMetadata(""));
    public static readonly DependencyProperty HeaderProperty =
        DependencyProperty.Register(nameof(Header), typeof(string), typeof(SettingsRow), new PropertyMetadata(""));
    public static readonly DependencyProperty DescriptionProperty =
        DependencyProperty.Register(nameof(Description), typeof(string), typeof(SettingsRow), new PropertyMetadata(""));

    public string Glyph { get => (string)GetValue(GlyphProperty); set => SetValue(GlyphProperty, value); }
    public string Header { get => (string)GetValue(HeaderProperty); set => SetValue(HeaderProperty, value); }
    public string Description { get => (string)GetValue(DescriptionProperty); set => SetValue(DescriptionProperty, value); }
}
