namespace HeadsetBridge.Core;

/// <summary>
/// Vorschau-Modus (<c>HeadsetBridge.exe --demo</c>): spielt einen typischen Verbindungsablauf
/// als Engine-Log nach, ohne etwas zu starten oder sich anzumelden. Für Screenshots und
/// zum Ausprobieren der Oberfläche. Die Zeilen laufen durch denselben <see cref="LogParser"/>.
/// </summary>
public sealed class DemoEngine : IEngine
{
    // (Sekunden nach dem Start, Logzeile) – wie engine/src sie schreibt.
    static readonly (double At, string Line)[] Script =
    {
        (0.0, "startup: headsetbridge-engine 2.0.0"),
        (0.0, "route: WebRTC"),
        (0.3, "audio output: Headphones (USB Audio) (shared mode)"),
        (1.2, "authenticated; region WestEurope"),
        (1.6, "session created"),
        (1.9, "console state: Provisioning (+300 ms after /play)"),
        (3.4, "console state: Provisioned (+1800 ms after /play)"),
        (3.5, "audio answer: m=audio 9 UDP/TLS/RTP/SAVPF 63 111 | a=rtpmap:63 red/48000/2 | a=rtpmap:111 opus/48000/2"),
        (3.9, "connection: connected"),
        (4.3, "first audio 2700 ms after /play"),
        (10.0, "audio: 50 pkt/s, 82.4 kbps, CELT FB stereo 290 of 290, other sources 0, redundant 288, buffer 20 ms (target 20)"),
    };

    DateTime? _started;
    DateTime? _stopping;
    StatusSnapshot _last = new();

    public bool EngineAvailable => true;

    public void Start() { _started = DateTime.UtcNow; _stopping = null; }

    public void Stop() { if (_started is not null) _stopping ??= DateTime.UtcNow; }

    public void Shutdown() { _started = null; _stopping = null; }

    public void Dispose() => Shutdown();

    public StatusSnapshot Snapshot()
    {
        if (_started is not { } started)
            return new StatusSnapshot { Device = _last.Device, Diagnostics = _last.Diagnostics };

        var now = DateTime.UtcNow;
        var elapsed = (now - started).TotalSeconds;
        var lines = Script.Where(s => s.At <= elapsed).Select(s => s.Line).ToList();
        // Nach 10 s zur Laufzeit wie die echte Engine: alle 10 s eine Statistikzeile.
        for (var t = 20.0; t <= elapsed; t += 10) lines.Add("audio: 50 pkt/s, 81.9 kbps, buffer 20 ms (target 20)");

        if (_stopping is { } stopping)
        {
            lines.Add("stop requested");
            if ((now - stopping).TotalSeconds > 1)
            {
                lines.Add("connection: closed");
                lines.Add("stopped");
                _last = LogParser.ParseStatus(ToLog(lines, elapsed));
                _started = null;
                _stopping = null;
                return new StatusSnapshot { Device = _last.Device, Diagnostics = _last.Diagnostics };
            }
        }

        _last = LogParser.ParseStatus(ToLog(lines, elapsed));
        return _last;
    }

    static string ToLog(List<string> lines, double elapsed)
    {
        // Zeitstempel gleichmäßig bis "jetzt" verteilen; die Reihenfolge zählt, nicht die Genauigkeit.
        var script = Script.ToDictionary(s => s.Line, s => s.At);
        var result = new System.Text.StringBuilder();
        var last = 0.0;
        foreach (var line in lines)
        {
            var at = script.TryGetValue(line, out var a) ? a : Math.Max(last, elapsed);
            last = at;
            result.Append($"[{(long)(at * 1000),6} ms] {line}\n");
        }
        return result.ToString();
    }
}
