using HeadsetBridge.Core;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace HeadsetBridge.Pages;

public sealed partial class LicensesPage : Page
{
    IReadOnlyList<License> _all = Array.Empty<License>();

    public LicensesPage()
    {
        InitializeComponent();
        Loaded += async (_, _) =>
        {
            LicenseTitle.Text = L.T("Lic.Loading");
            _all = await Task.Run(() => Services.Licenses);
            Filter("");
        };
    }

    void SearchBox_TextChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args) => Filter(sender.Text);

    void Filter(string query)
    {
        var visible = string.IsNullOrWhiteSpace(query)
            ? _all.ToList()
            : _all.Where(l => l.Name.Contains(query.Trim(), StringComparison.CurrentCultureIgnoreCase)).ToList();
        LicenseList.ItemsSource = visible;
        NoMatches.Visibility = visible.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        LicenseList.Visibility = TextScroller.Visibility = visible.Count == 0 ? Visibility.Collapsed : Visibility.Visible;
        if (visible.Count > 0) LicenseList.SelectedIndex = 0;
    }

    void LicenseList_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (LicenseList.SelectedItem is not License l) return;
        LicenseTitle.Text = l.Name;
        LicenseText.Text = l.Text;
        TextScroller.ChangeView(null, 0, null, true);
    }
}
