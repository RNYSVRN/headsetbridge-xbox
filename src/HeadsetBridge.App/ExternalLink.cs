using System.Diagnostics;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace HeadsetBridge;

/// <summary>
/// Link-Schaltfläche, die eine Adresse (https:, ms-settings:) über die Windows-Shell öffnet.
/// HyperlinkButton.NavigateUri öffnet in Apps ohne MSIX-Paket nichts, daher dieser Ersatz.
/// </summary>
public sealed class ExternalLink : HyperlinkButton
{
    public static readonly DependencyProperty TargetProperty =
        DependencyProperty.Register(nameof(Target), typeof(string), typeof(ExternalLink), new PropertyMetadata(""));

    public string Target { get => (string)GetValue(TargetProperty); set => SetValue(TargetProperty, value); }

    public ExternalLink()
    {
        DefaultStyleKey = typeof(HyperlinkButton);
        Click += (_, _) => Open(Target);
    }

    public static void Open(string target)
    {
        if (string.IsNullOrWhiteSpace(target)) return;
        try { Process.Start(new ProcessStartInfo(target) { UseShellExecute = true }); }
        catch (Exception) { /* Kein Handler für die Adresse: nichts zu tun. */ }
    }
}
