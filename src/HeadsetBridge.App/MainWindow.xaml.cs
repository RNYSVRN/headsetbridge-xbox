using System.Runtime.InteropServices;
using HeadsetBridge.Core;
using HeadsetBridge.Pages;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.Graphics;

namespace HeadsetBridge;

public sealed partial class MainWindow : Window
{
    readonly DispatcherQueueTimer _timer;

    public MainWindow()
    {
        InitializeComponent();

        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "headsetbridge.ico"));

        var scale = GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(this)) / 96.0;
        AppWindow.Resize(new SizeInt32((int)(980 * scale), (int)(760 * scale)));
        if (AppWindow.Presenter is OverlappedPresenter presenter)
        {
            presenter.PreferredMinimumWidth = (int)(740 * scale);
            presenter.PreferredMinimumHeight = (int)(600 * scale);
        }
        CenterOnScreen();

        ApplyTheme(Services.StartTheme);

        ApplyTexts();
        // Der Einstellungen-Eintrag entsteht erst mit der Vorlage der NavigationView;
        // die Startseite wird deshalb erst nach dem Laden gewählt.
        Nav.Loaded += (_, _) =>
        {
            ApplyTexts();
            Nav.SelectedItem = ConnectionItem;
        };

        Services.StatusChanged += OnStatusChanged;
        _timer = DispatcherQueue.CreateTimer();
        _timer.Interval = TimeSpan.FromMilliseconds(650);
        _timer.Tick += (_, _) => Services.Refresh();
        _timer.Start();
        Services.Refresh();
        OnStatusChanged(Services.Status);

        Closed += (_, _) =>
        {
            _timer.Stop();
            Services.StatusChanged -= OnStatusChanged;
            Services.Engine.Shutdown(); // Schließen beendet die Übertragung
        };
    }

    public void ApplyTheme(AppTheme theme)
    {
        Root.RequestedTheme = theme switch
        {
            AppTheme.Light => ElementTheme.Light,
            AppTheme.Dark => ElementTheme.Dark,
            _ => ElementTheme.Default,
        };
        AppWindow.TitleBar.PreferredTheme = theme switch
        {
            AppTheme.Light => TitleBarTheme.Light,
            AppTheme.Dark => TitleBarTheme.Dark,
            _ => TitleBarTheme.UseDefaultAppMode,
        };
    }

    /// <summary>Sprache wechseln: Texte der Navigation setzen, aktuelle Seite neu laden, Status neu erzeugen.</summary>
    public void ApplyLanguage()
    {
        Services.ApplyLanguageSetting();
        ApplyTexts();
        var page = ContentFrame.CurrentSourcePageType;
        if (page is not null)
        {
            ContentFrame.Navigate(page, null, new Microsoft.UI.Xaml.Media.Animation.SuppressNavigationTransitionInfo());
            ContentFrame.BackStack.Clear();
        }
        Services.Refresh(force: true);
    }

    void ApplyTexts()
    {
        AppWindow.Title = L.T("App.Title");
        AppTitle.Text = L.T("App.Title");
        ConnectionItem.Content = L.T("Nav.Connection");
        DiagnosticsItem.Content = L.T("Nav.Diagnostics");
        HelpHeader.Content = L.T("Nav.HelpHeader");
        ManualItem.Content = L.T("Nav.Manual");
        LicensesItem.Content = L.T("Nav.Licenses");
        AboutItem.Content = L.T("Nav.About");
        if (Nav.SettingsItem is NavigationViewItem settings) settings.Content = L.T("Nav.Settings");
        OnStatusChanged(Services.Status);
    }

    public void NavigateTo(string tag)
    {
        var item = Nav.MenuItems.OfType<NavigationViewItem>().FirstOrDefault(i => (string)i.Tag == tag);
        if (item is not null) Nav.SelectedItem = item;
    }

    void Nav_SelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {        var page = args.IsSettingsSelected
            ? typeof(SettingsPage)
            : (args.SelectedItem as NavigationViewItem)?.Tag switch
            {
                "diagnostics" => typeof(DiagnosticsPage),
                "manual" => typeof(ManualPage),
                "licenses" => typeof(LicensesPage),
                "about" => typeof(AboutPage),
                _ => typeof(ConnectionPage),
            };
        if (ContentFrame.CurrentSourcePageType != page)
            ContentFrame.Navigate(page, null, args.RecommendedNavigationTransitionInfo);
    }

    void OnStatusChanged(StatusSnapshot s)
    {
        ConnectionBadge.Visibility = s.Phase == Phase.Connected ? Visibility.Visible : Visibility.Collapsed;
        TitleStatus.Text = s.IsRunning || s.Phase == Phase.Error ? "· " + s.PhaseLabel : "";
    }

    void CenterOnScreen()
    {
        var area = DisplayArea.GetFromWindowId(AppWindow.Id, DisplayAreaFallback.Nearest).WorkArea;
        var size = AppWindow.Size;
        AppWindow.Move(new PointInt32(area.X + Math.Max(0, (area.Width - size.Width) / 2),
                                      area.Y + Math.Max(0, (area.Height - size.Height) / 2)));
    }

    [DllImport("user32.dll")]
    static extern uint GetDpiForWindow(IntPtr hwnd);
}
