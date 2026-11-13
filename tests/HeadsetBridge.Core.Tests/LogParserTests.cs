using HeadsetBridge.Core;
using Xunit;

namespace HeadsetBridge.Core.Tests;

// Ursprünglich aus den Rust-Tests des Desktop-Hosts 1.2.0. Die Logzeilen entsprechen
// dem, was engine/src (englisch) schreibt; ändert sich dort eine Zeile, muss sie hier mit.
public class LogParserTests
{
    public LogParserTests() => L.SetLanguage("de");

    [Fact]
    public void EnglishTextsAreUsedWhenSelected()
    {
        L.SetLanguage("en-US");
        try
        {
            var s = LogParser.ParseStatus("[     1 ms] first audio 100 ms after /play");
            Assert.Equal("Playing Xbox audio", s.Title);
            Assert.Equal("Connected", s.PhaseLabel);
            Assert.Equal("Windows default device", s.DisplayDevice);
            Assert.Equal("First audio received. Playback started.", s.Diagnostics[0].Message);
            Assert.Equal("Audio reception: 50 packets/s, 80.0 kbit/s.",
                LogParser.LocalizedDiagnostics("[     0 ms] audio: 50 pkt/s, 80.0 kbps")[0].Message);
        }
        finally { L.SetLanguage("de"); }
    }

    [Fact]
    public void GermanVariantsMapToGermanAndOthersToEnglish()
    {
        L.SetLanguage("de-AT"); Assert.Equal("de", L.Language);
        L.SetLanguage("fr-FR"); Assert.Equal("en", L.Language);
        L.SetLanguage("de");
    }

    [Fact]
    public void EveryPhaseHasALabel()
    {
        foreach (var p in Enum.GetValues<Phase>())
            Assert.DoesNotContain("Phase.", new StatusSnapshot { Phase = p }.PhaseLabel);
    }

    [Fact]
    public void RealAudioMarksConnected()
    {
        var s = LogParser.ParseStatus(
            "[     1 ms] audio output: Kopfhörer (Test) (shared mode)\n" +
            "[     2 ms] first audio 100 ms after /play\n" +
            "[     3 ms] audio: 50 pkt/s, 80.0 kbps, CELT FB stereo 50 of 50, other sources 0, redundant 49, buffer 20 ms (target 20), Counters { played: 50 }");
        Assert.Equal(Phase.Connected, s.Phase);
        Assert.Equal("Kopfhörer (Test)", s.Device);
        Assert.Equal(20, s.BufferMs);
        Assert.Equal(50.0, s.PacketsPerSecond);
    }

    [Fact]
    public void OtherOutputLinesDoNotChangeTheDevice()
    {
        var s = LogParser.ParseStatus(
            "[     1 ms] audio output: Speakers (shared mode)\n" +
            "[     2 ms] audio output: the device asks for 480 samples (10.0 ms) per callback\n" +
            "[     3 ms] audio output: error: device removed");
        Assert.Equal("Speakers", s.Device);
    }

    [Fact]
    public void SilenceBeforeConnectIsNotConnected() =>
        Assert.Equal(Phase.Connecting, LogParser.ParseStatus("[  10000 ms] audio: 0 pkt/s, 0.0 kbps").Phase);

    [Fact]
    public void ReconnectCanRecover() =>
        Assert.Equal(Phase.Connected, LogParser.ParseStatus(
            "[  1000 ms] reconnecting in 1 s (the console failed: AgentCommandError)\n" +
            "[  3000 ms] authenticated; region UkSouth\n" +
            "[  5000 ms] first audio 200 ms after /play").Phase);

    [Fact]
    public void AgentCommandErrorHasItsOwnHint()
    {
        var s = LogParser.ParseStatus("[  1000 ms] reconnecting in 1 s (the console failed: AgentCommandError)");
        Assert.Equal(Phase.Waiting, s.Phase);
        Assert.Contains("Remotefeatures", s.Detail);
    }

    [Fact]
    public void StopOverridesOldAudio() =>
        Assert.Equal(Phase.Stopping, LogParser.ParseStatus(
            "[   100 ms] first audio 50 ms after /play\n[   400 ms] stop requested\n[   500 ms] stopped").Phase);

    [Fact]
    public void LoginHasDistinctState() =>
        Assert.Equal(Phase.Auth, LogParser.ParseStatus(
            "[     0 ms] sign-in: no saved sign-in; sign in at https://www.microsoft.com/link with the code from the sign-in window").Phase);

    [Fact]
    public void CrlfLinesAreHandled() =>
        Assert.Equal(Phase.Stopping, LogParser.ParseStatus("[   100 ms] first audio 1 ms after /play\r\n[   500 ms] stopped\r\n").Phase);

    [Fact]
    public void DiagnosticsTranslateEventsWithoutExposingLoginCodes()
    {
        var records = LogParser.LocalizedDiagnostics(
            "[     0 ms] sign-in: no saved sign-in; sign in at https://www.microsoft.com/link with the code SECRET123\n" +
            "[  1000 ms] authenticated; region UkSouth\n" +
            "[  2500 ms] first audio 1200 ms after /play");
        var text = string.Join("\n", records.Select(r => r.Message));
        Assert.Contains("Anmeldung erfolgreich", text);
        Assert.Contains("Erste Audiodaten", text);
        Assert.DoesNotContain("SECRET123", text);
        Assert.DoesNotContain("sign-in:", text);
        Assert.Equal("00:02", records[2].Time);
    }

    [Fact]
    public void FullSessionProducesTheExpectedEvents()
    {
        var records = LogParser.LocalizedDiagnostics(
            "[     0 ms] startup: headsetbridge-engine 2.0.0\n" +
            "[     0 ms] route: WebRTC\n" +
            "[   300 ms] audio output: Speakers (shared mode)\n" +
            "[  1500 ms] authenticated; region WestEurope\n" +
            "[  2000 ms] session created\n" +
            "[  2300 ms] console state: Provisioning (+300 ms after /play)\n" +
            "[  3800 ms] console state: Provisioned (+1800 ms after /play)\n" +
            "[  3900 ms] audio answer: m=audio 9 UDP/TLS/RTP/SAVPF 63 111 | a=rtpmap:111 opus/48000/2\n" +
            "[  4200 ms] connection: connected\n" +
            "[  4600 ms] first audio 2600 ms after /play\n" +
            "[ 60000 ms] stop requested\n" +
            "[ 60200 ms] connection: closed\n" +
            "[ 60300 ms] stopped");
        Assert.Equal(new[]
        {
            "Audio-Dienst gestartet.", "Verbindungsverfahren: WebRTC.", "Audioausgabe: Speakers.",
            "Microsoft-/Xbox-Anmeldung erfolgreich.", "Remote-Play-Sitzung angefordert.",
            "Xbox bereitet die Übertragung vor.", "Xbox ist für die Übertragung bereit.",
            "Audioformat ausgehandelt: Opus, 48 kHz, Stereo.", "Verbindung zur Xbox hergestellt.",
            "Erste Audiodaten empfangen. Wiedergabe gestartet.", "Beenden der Übertragung angefordert.",
            "Xbox-Verbindung geschlossen.", "Audio-Dienst beendet.",
        }, records.Select(r => r.Message));
    }

    [Fact]
    public void RepeatedAudioMetricsAreCompactedAndOrdered()
    {
        var records = LogParser.LocalizedDiagnostics(
            "[     0 ms] startup: headsetbridge-engine 2.0.0\n" +
            "[ 10000 ms] audio: 50 pkt/s, 80.0 kbps, buffer 20 ms\n" +
            "[ 20000 ms] audio: 49 pkt/s, 70.0 kbps, buffer 20 ms\n" +
            "[ 21000 ms] stopped");
        Assert.Equal(3, records.Count);
        Assert.Equal("Audioempfang: 49 Pakete/s, 70.0 kbit/s.", records[1].Message);
        Assert.Equal("Audio-Dienst beendet.", records[^1].Message);
    }

    [Fact]
    public void RecentLogStartsAtTheLastEngineStart()
    {
        var path = Path.Combine(Path.GetTempPath(), $"hb-log-{Guid.NewGuid():N}.log");
        File.WriteAllText(path, "[ 0 ms] startup: headsetbridge-engine 1\n[ 1 ms] old\n[ 0 ms] startup: headsetbridge-engine 2\n[ 1 ms] new\n");
        try { Assert.DoesNotContain("old", LogParser.ReadRecentLog(path, 0)); }
        finally { File.Delete(path); }
    }

    [Fact]
    public void LicensesKeepOriginalTextAndDoNotDropLastSection()
    {
        var records = LicenseParser.Parse("MIT License\nEnglish original\n===== library-a / LICENSE =====\nPermission is hereby granted.\n===== library-b / NOTICE =====\nCopyright example");
        Assert.Equal(3, records.Count);
        Assert.Equal("library-a / LICENSE", records[1].Name);
        Assert.Equal("Permission is hereby granted.", records[1].Text);
        Assert.Equal("Copyright example", records[2].Text);
    }

    [Fact]
    public void SnapshotEqualityComparesDiagnostics()
    {
        var a = new StatusSnapshot { Diagnostics = new[] { new Diagnostic("00:01", DiagnosticLevel.Info, "x") } };
        var b = new StatusSnapshot { Diagnostics = new[] { new Diagnostic("00:01", DiagnosticLevel.Info, "x") } };
        Assert.True(a.SameAs(b));
        Assert.False(a.SameAs(b with { Title = "y" }));
    }
}
