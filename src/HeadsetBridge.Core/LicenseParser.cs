using System.Text;

namespace HeadsetBridge.Core;

/// <summary>Zerlegt die Lizenzhinweise an Zeilen der Form "===== Name =====". Texte bleiben unverändert.</summary>
public static class LicenseParser
{
    public static IReadOnlyList<License> Parse(string text)
    {
        var records = new List<License>();
        var name = L.T("License.General");
        var body = new StringBuilder();

        foreach (var raw in text.Split('\n'))
        {
            var line = raw.TrimEnd('\r');
            if (line.StartsWith("===== ", StringComparison.Ordinal) && line.EndsWith(" =====", StringComparison.Ordinal) && line.Length >= 12)
            {
                Flush();
                name = line[6..^6];
                body.Clear();
            }
            else
            {
                body.Append(line).Append('\n');
            }
        }
        Flush();
        return records;

        void Flush()
        {
            var t = body.ToString().Trim();
            if (t.Length > 0) records.Add(new License(name, t));
        }
    }
}
