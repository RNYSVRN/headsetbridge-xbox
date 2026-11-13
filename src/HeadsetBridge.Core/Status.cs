namespace HeadsetBridge.Core;

public enum Phase { Idle, Connecting, Connected, Stopping, Waiting, Error, Auth }

public enum DiagnosticLevel { Info, Success, Warning, Error }

public sealed record Diagnostic(string Time, DiagnosticLevel Level, string Message);

public sealed record License(string Name, string Text);

public sealed record StatusSnapshot
{
    public Phase Phase { get; init; } = Phase.Idle;
    public string Title { get; init; } = L.T("Status.Idle.Title");
    public string Detail { get; init; } = L.T("Status.Idle.Detail");

    /// <summary>Gerätename aus dem Engine-Protokoll; leer, solange er unbekannt ist.</summary>
    public string Device { get; init; } = "";
    public int? BufferMs { get; init; }
    public double? PacketsPerSecond { get; init; }
    public IReadOnlyList<Diagnostic> Diagnostics { get; init; } = Array.Empty<Diagnostic>();

    public string DisplayDevice => string.IsNullOrEmpty(Device) ? L.T("Device.Default") : Device;
    public bool IsRunning => Phase is not (Phase.Idle or Phase.Error);
    public bool IsBusy => Phase is Phase.Connecting or Phase.Stopping;
    public string PhaseLabel => L.T("Phase." + Phase);

    /// <summary>Vergleich inklusive Diagnose-Inhalt (Records vergleichen Listen sonst per Referenz).</summary>
    public bool SameAs(StatusSnapshot? other) =>
        other is not null
        && Phase == other.Phase && Title == other.Title && Detail == other.Detail && Device == other.Device
        && BufferMs == other.BufferMs && PacketsPerSecond == other.PacketsPerSecond
        && Diagnostics.SequenceEqual(other.Diagnostics);
}
