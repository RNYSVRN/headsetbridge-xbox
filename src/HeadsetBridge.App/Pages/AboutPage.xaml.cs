using HeadsetBridge.Core;
using Microsoft.UI.Xaml.Controls;

namespace HeadsetBridge.Pages;

public sealed partial class AboutPage : Page
{
    public AboutPage()
    {
        InitializeComponent();
        VersionText.Text = L.F("About.Version", Services.Version);
    }
}
