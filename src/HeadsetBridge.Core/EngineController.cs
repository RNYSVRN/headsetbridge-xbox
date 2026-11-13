using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

namespace HeadsetBridge.Core;

/// <summary>
/// Startet und beendet die Audio-Engine (headsetbridge-engine.exe) und liefert den aktuellen Status.
/// Die Engine meldet sich über die Named Pipe "headsetbridge-engine"; "stop" beendet sie geordnet.
/// </summary>
public sealed class EngineController : IEngine
{
    const string PipeName = @"\\.\pipe\headsetbridge-engine";
    static readonly TimeSpan StopTimeout = TimeSpan.FromSeconds(3);

    readonly string _enginePath;
    readonly string _logPath;
    Process? _child;
    DateTime? _stopping;
    long _offset;
    Func<string>? _error;
    StatusSnapshot _last = new();

    public EngineController(string enginePath, string? logPath = null)
    {
        _enginePath = enginePath;
        _logPath = logPath ?? Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "HeadsetBridge", "engine", "engine.log");
    }

    public string EnginePath => _enginePath;
    public bool EngineAvailable => File.Exists(_enginePath);

    bool ChildRunning => _child is { HasExited: false };
    bool Running => PipeActive() || ChildRunning;

    public void Start()
    {
        if (Running) return;
        _error = null;
        _last = _last with { Diagnostics = Array.Empty<Diagnostic>() };
        _offset = File.Exists(_logPath) ? new FileInfo(_logPath).Length : 0;

        if (!EngineAvailable)
        {
            _error = () => L.F("Error.EngineMissing", _enginePath);
            return;
        }
        try
        {
            _child?.Dispose();
            _child = Process.Start(new ProcessStartInfo(_enginePath, $"--lang={L.Language}")
            {
                UseShellExecute = false,
                CreateNoWindow = true,
                WorkingDirectory = Path.GetDirectoryName(_enginePath)!,
                RedirectStandardInput = false,
                RedirectStandardOutput = false,
                RedirectStandardError = false,
            });
            _stopping = null;
        }
        catch (Exception e)
        {
            var msg = e.Message; _error = () => L.F("Error.EngineStart", msg);
        }
    }

    public void Stop()
    {
        SendStop();
        _stopping = DateTime.UtcNow;
    }

    public StatusSnapshot Snapshot()
    {
        var text = LogParser.ReadRecentLog(_logPath, _offset);
        var diagnostics = LogParser.LocalizedDiagnostics(text);
        if (diagnostics.Count > 0) _last = _last with { Diagnostics = diagnostics };

        if (_stopping is { } t && DateTime.UtcNow - t > StopTimeout)
        {
            KillChild();
            _stopping = null;
        }

        if (_error is not null)
            return new StatusSnapshot
            {
                Phase = Phase.Error,
                Title = L.T("Status.Error.Title"),
                Detail = _error(),
                Diagnostics = _last.Diagnostics,
            };

        if (!Running)
        {
            _stopping = null;
            return new StatusSnapshot { Device = _last.Device, Diagnostics = _last.Diagnostics };
        }

        if (_stopping is not null)
            return new StatusSnapshot
            {
                Phase = Phase.Stopping,
                Title = L.T("Status.Stopping.Title"),
                Detail = L.T("Status.Stopping.Detail"),
                Device = _last.Device,
                Diagnostics = _last.Diagnostics,
            };

        var current = LogParser.ParseStatus(text);
        if (string.IsNullOrEmpty(current.Device)) current = current with { Device = _last.Device };
        _last = current;
        return current;
    }

    /// <summary>Beim Schließen: geordnet beenden, nach 3 s hart.</summary>
    public void Shutdown()
    {
        SendStop();
        if (_child is null) return;
        try
        {
            if (!_child.WaitForExit(StopTimeout)) KillChild();
        }
        catch (InvalidOperationException) { }
        _child.Dispose();
        _child = null;
    }

    public void Dispose() => Shutdown();

    void KillChild()
    {
        try
        {
            if (ChildRunning) { _child!.Kill(); _child.WaitForExit(); }
        }
        catch (Exception) { }
    }

    static bool PipeActive() => WaitNamedPipeW(PipeName, 1);

    static void SendStop()
    {
        try
        {
            using var pipe = new FileStream(PipeName, FileMode.Open, FileAccess.Write);
            pipe.Write(Encoding.ASCII.GetBytes("stop\n"));
        }
        catch (Exception) { }
    }

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    static extern bool WaitNamedPipeW(string name, uint timeout);
}
