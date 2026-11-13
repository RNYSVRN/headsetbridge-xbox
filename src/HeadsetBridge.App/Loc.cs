using HeadsetBridge.Core;
using Microsoft.UI.Xaml.Markup;

namespace HeadsetBridge;

/// <summary>XAML-Übersetzung: Text="{local:Loc Key=Nav.Connection}". Wird beim Laden der Seite ausgewertet.</summary>
[MarkupExtensionReturnType(ReturnType = typeof(string))]
public sealed class Loc : MarkupExtension
{
    public string Key { get; set; } = "";
    protected override object ProvideValue() => L.T(Key);
}
