using System.Globalization;

namespace HeadsetBridge.Core;

/// <summary>
/// Wertet das Protokoll der Audio-Engine (%LOCALAPPDATA%\HeadsetBridge\engine\engine.log) aus.
/// Die Engine schreibt englische Meldungen (engine/src/log.rs); hier entstehen daraus Status und übersetzte Diagnose.
/// Anmeldecodes und andere Rohdaten werden nie übernommen.
/// </summary>
public static class LogParser
{
    public const string StartMarker = "startup: headsetbridge-engine";
    const string OutputPrefix = "audio output: ";
    const string OutputSuffix = " (shared mode)";
    const string PacketsSuffix = " pkt/s,";

    public static StatusSnapshot ParseStatus(string text)
    {
        var phase = Phase.Connecting;
        var title = L.T("Status.Preparing.Title");
        var detail = L.T("Status.Preparing.Detail");
        var device = "";
        int? bufferMs = null;
        double? pps = null;

        foreach (var line in Lines(text))
        {
            if (After(line, OutputPrefix) is { } o && Before(o, OutputSuffix) is { } name) device = name;
            if (After(line, "audio: ") is { } v)
            {
                if (Before(v, PacketsSuffix) is { } n) pps = ParseDouble(n);
            }
            if (After(line, "(target ") is { } target)
                bufferMs = int.TryParse(target.Split(')')[0], NumberStyles.None, CultureInfo.InvariantCulture, out var b) ? b : null;

            if (line.Contains("sign in at"))
            {
                phase = Phase.Auth;
                title = L.T("Status.Auth.Title");
                detail = L.T("Status.Auth.Detail");
            }
            if (line.Contains("authenticated;") || line.Contains("session created"))
            {
                phase = Phase.Connecting;
                title = L.T("Status.Authenticated.Title");
                detail = L.T("Status.Authenticated.Detail");
            }
            if (line.Contains("connection: connected"))
            {
                phase = Phase.Connecting;
                title = L.T("Status.Linked.Title");
            }
            if (line.Contains("first audio") || (line.Contains(PacketsSuffix) && pps > 0))
            {
                phase = Phase.Connected;
                title = L.T("Status.Playing.Title");
                detail = L.T("Status.Playing.Detail");
            }
            if (line.Contains("reconnecting") || line.Contains("waiting for the console"))
            {
                phase = Phase.Waiting;
                title = L.T("Status.Waiting.Title");
                detail = line.Contains("AgentCommandError")
                    ? L.T("Status.Waiting.AgentError")
                    : L.T("Status.Waiting.Detail");
            }
            if (line.Contains("stop requested") || line.EndsWith(" stopped", StringComparison.Ordinal))
            {
                phase = Phase.Stopping;
                title = L.T("Status.Stopping.Title");
                detail = L.T("Status.Stopping.Detail");
            }
        }

        return new StatusSnapshot
        {
            Phase = phase, Title = title, Detail = detail, Device = device,
            BufferMs = bufferMs, PacketsPerSecond = pps,
            Diagnostics = LocalizedDiagnostics(text),
        };
    }

    public static IReadOnlyList<Diagnostic> LocalizedDiagnostics(string text)
    {
        var records = new List<(long Ms, Diagnostic D)>();
        (long Ms, Diagnostic D)? stats = null;

        foreach (var line in Lines(text))
        {
            var ms = Milliseconds(line);
            var level = DiagnosticLevel.Info;
            string? message;

            if (line.Contains(StartMarker)) message = L.T("Event.Started");
            else if (line.Contains("route: WebRTC")) message = L.T("Event.WebRtc");
            else if (After(line, OutputPrefix) is { } a && Before(a, OutputSuffix) is { } dev) message = L.F("Event.Output", dev);
            else if (line.Contains("sign in at")) message = L.T("Event.SignIn");
            else if (line.Contains("authenticated;")) { level = DiagnosticLevel.Success; message = L.T("Event.Authenticated"); }
            else if (line.Contains("session created")) message = L.T("Event.SessionCreated");
            else if (line.Contains("console state: Provisioning")) message = L.T("Event.Provisioning");
            else if (line.Contains("console state: Provisioned")) message = L.T("Event.Provisioned");
            else if (line.Contains("console state: Failed")) { level = DiagnosticLevel.Error; message = L.T("Event.ProvisionFailed"); }
            else if (line.Contains("audio answer:") && line.Contains("opus/48000/2")) message = L.T("Event.Format");
            else if (line.Contains("connection: connected")) { level = DiagnosticLevel.Success; message = L.T("Event.Connected"); }
            else if (line.Contains("first audio")) { level = DiagnosticLevel.Success; message = L.T("Event.FirstAudio"); }
            else if (line.Contains("reconnecting") || line.Contains("waiting for the console"))
            {
                level = DiagnosticLevel.Warning;
                message = line.Contains("AgentCommandError")
                    ? L.T("Event.AgentError")
                    : L.T("Event.Interrupted");
            }
            else if (line.Contains("stop requested")) message = L.T("Event.StopRequested");
            else if (line.Contains("connection: closed")) message = L.T("Event.Closed");
            else if (line.EndsWith(" stopped", StringComparison.Ordinal)) message = L.T("Event.Stopped");
            else message = null;

            var time = FormatTime(ms);
            if (message is not null) records.Add((ms / 1000, new Diagnostic(time, level, message)));

            // Regelmäßige Audio-Statistik: nur der jüngste Wert wird angezeigt.
            if (After(line, "audio: ") is { } s && Before(s, PacketsSuffix) is { } rate && ParseDouble(rate) is not null)
            {
                var rest = s[(rate.Length + PacketsSuffix.Length)..].Trim();
                var idx = rest.IndexOf(" kbps", StringComparison.Ordinal);
                var bitrate = idx >= 0 ? rest[..idx] : rest;
                stats = (ms / 1000, new Diagnostic(time, DiagnosticLevel.Info, L.F("Event.Stats", rate, bitrate)));
            }
        }

        if (stats is { } st) records.Add(st);
        var ordered = records.OrderBy(r => r.Ms).Select(r => r.D).ToList(); // stabil wie im Original
        return ordered.Count > 100 ? ordered.GetRange(ordered.Count - 100, 100) : ordered;
    }

    /// <summary>Liest das Ende des Protokolls ab dem letzten Engine-Start.</summary>
    public static string ReadRecentLog(string path, long offset)
    {
        try
        {
            using var file = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
            var len = file.Length;
            var start = offset > len ? 0 : offset;
            file.Seek(Math.Max(start, Math.Max(0, len - 24576)), SeekOrigin.Begin);
            using var reader = new StreamReader(file, new System.Text.UTF8Encoding(false, false));
            var text = reader.ReadToEnd();
            var i = text.LastIndexOf(StartMarker, StringComparison.Ordinal);
            return i >= 0 ? text[i..] : text;
        }
        catch (IOException) { return ""; }
        catch (UnauthorizedAccessException) { return ""; }
    }

    static IEnumerable<string> Lines(string text) =>
        text.Split('\n').Select(l => l.TrimEnd('\r'));

    static string? After(string s, string sep)
    {
        var i = s.IndexOf(sep, StringComparison.Ordinal);
        return i < 0 ? null : s[(i + sep.Length)..];
    }

    static string? Before(string s, string sep)
    {
        var i = s.IndexOf(sep, StringComparison.Ordinal);
        return i < 0 ? null : s[..i];
    }

    static double? ParseDouble(string s) =>
        double.TryParse(s, NumberStyles.AllowDecimalPoint | NumberStyles.AllowLeadingSign, CultureInfo.InvariantCulture, out var v) ? v : null;

    static long Milliseconds(string line)
    {
        var close = line.IndexOf(']');
        if (close < 0) return 0;
        var prefix = line[..close].TrimStart('[').Trim();
        if (!prefix.EndsWith("ms", StringComparison.Ordinal)) return 0;
        return long.TryParse(prefix[..^2].Trim(), NumberStyles.None, CultureInfo.InvariantCulture, out var ms) ? ms : 0;
    }

    static string FormatTime(long ms) => $"{ms / 60000:00}:{ms / 1000 % 60:00}";
}
