using HeadsetBridge.Core;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace HeadsetBridge.Pages;

public sealed partial class ConnectionPage : Page
{
    bool _pending;

    public ConnectionPage()
    {
        InitializeComponent();
        Loaded += (_, _) => { Services.StatusChanged += Update; Update(Services.Status); };
        Unloaded += (_, _) => Services.StatusChanged -= Update;
    }

    void ConnectButton_Click(object sender, RoutedEventArgs e)
    {
        _pending = true;
        ConnectButton.IsEnabled = false;
        if (Services.Status.IsRunning) Services.Engine.Stop(); else Services.Engine.Start();
        Services.Refresh();
        _pending = false;
        Update(Services.Status);
    }

    void Update(StatusSnapshot s)
    {
        var connected = s.Phase == Phase.Connected;

        StatusTitle.Text = s.Title;
        StatusDetail.Text = s.Detail;
        StatusRing.IsActive = s.IsBusy || s.Phase == Phase.Waiting;

        StatusIcon.Glyph = s.Phase switch
        {
            Phase.Connected => "\uE767",
            Phase.Auth => "\uE77B",
            Phase.Waiting => "\uE895",
            Phase.Error => "\uEA39",
            _ => "\uE7F6",
        };
        VisualStateManager.GoToState(this, connected ? "Connected" : s.Phase == Phase.Error ? "Failed" : "Idle", true);

        // Schaltfläche: Akzent zum Verbinden, neutral zum Trennen (wie in Windows üblich)
        var running = s.IsRunning;
        ConnectButton.Style = (Style)Application.Current.Resources[running ? "DefaultButtonStyle" : "AccentButtonStyle"];
        ConnectGlyph.Glyph = running ? "\uE71A" : "\uE768";
        ConnectText.Text = L.T(s.Phase == Phase.Stopping ? "Conn.Disconnecting" : running ? "Conn.Disconnect" : "Conn.Connect");
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(ConnectButton, ConnectText.Text);
        ConnectButton.IsEnabled = !_pending && s.Phase != Phase.Stopping && (running || Services.Engine.EngineAvailable);

        DeviceText.Text = s.DisplayDevice;
        ToolTipService.SetToolTip(DeviceText, s.DisplayDevice);
        FormatText.Text = connected ? "Opus · 48 kHz · Stereo" : L.T("Conn.NotConnected");
        FormatRow.Description = connected && s.BufferMs is { } ms
            ? L.F("Conn.Buffer", ms) + (s.PacketsPerSecond is { } p ? " · " + L.F("Conn.Packets", p) : "")
            : L.T("Conn.FormatUnavailable");

        if (!Services.Engine.EngineAvailable && s.Phase == Phase.Idle)
            ShowInfo(InfoBarSeverity.Error, L.T("Info.EngineMissing.Title"),
                L.T("Info.EngineMissing.Text"));
        else switch (s.Phase)
        {
            case Phase.Auth:
                ShowInfo(InfoBarSeverity.Informational, L.T("Info.Auth.Title"),
                    L.T("Info.Auth.Text"));
                break;
            case Phase.Waiting:
                ShowInfo(InfoBarSeverity.Warning, L.T("Info.Waiting.Title"), L.T("Info.Waiting.Text"));
                break;
            case Phase.Error:
                ShowInfo(InfoBarSeverity.Error, L.T("Info.Error.Title"), L.T("Info.Error.Text"));
                break;
            default:
                StatusInfo.IsOpen = false;
                break;
        }
    }

    void ShowInfo(InfoBarSeverity severity, string title, string message)
    {
        StatusInfo.Severity = severity;
        StatusInfo.Title = title;
        StatusInfo.Message = message;
        StatusInfo.IsOpen = true;
    }
}
