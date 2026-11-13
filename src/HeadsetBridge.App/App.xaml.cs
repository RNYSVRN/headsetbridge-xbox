using System.Runtime.InteropServices;
using Microsoft.UI.Xaml;

namespace HeadsetBridge;

public partial class App : Application
{
    static Mutex? _instanceMutex;
    public static MainWindow? Window { get; private set; }

    public App()
    {
        InitializeComponent();
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        // Nur eine Instanz: ein zweiter Start holt das vorhandene Fenster nach vorn.
        _instanceMutex = new Mutex(true, @"Local\HeadsetBridgeXbox", out var first);
        if (!first)
        {
            var existing = FindWindowW(null, "HeadsetBridge für Xbox");
            if (existing == IntPtr.Zero) existing = FindWindowW(null, "HeadsetBridge for Xbox");
            if (existing != IntPtr.Zero)
            {
                ShowWindow(existing, 9 /* SW_RESTORE */);
                SetForegroundWindow(existing);
            }
            Exit();
            return;
        }

        Services.ApplyLanguageSetting();
        Window = new MainWindow();
        Window.Activate();
    }

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern IntPtr FindWindowW(string? className, string windowName);

    [DllImport("user32.dll")]
    static extern bool ShowWindow(IntPtr hWnd, int cmd);

    [DllImport("user32.dll")]
    static extern bool SetForegroundWindow(IntPtr hWnd);
}
