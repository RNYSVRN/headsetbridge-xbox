namespace HeadsetBridge.Core;

/// <summary>Was die Oberfläche von der Audio-Engine braucht.</summary>
public interface IEngine : IDisposable
{
    bool EngineAvailable { get; }
    void Start();
    void Stop();
    StatusSnapshot Snapshot();
    void Shutdown();
}
