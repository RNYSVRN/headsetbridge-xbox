using System.Globalization;

namespace HeadsetBridge.Core;

/// <summary>
/// Texte der App auf Deutsch und Englisch. Eine Zeile pro Schlüssel hält beide Sprachen synchron.
/// </summary>
public static class L
{
    public static string Language { get; private set; } = "de";

    /// <summary>"de", "en" oder ein Kulturname wie "de-AT"; alles außer Deutsch wird Englisch.</summary>
    public static void SetLanguage(string code) =>
        Language = code.StartsWith("de", StringComparison.OrdinalIgnoreCase) ? "de" : "en";

    public static string SystemLanguage => CultureInfo.CurrentUICulture.Name;

    public static string T(string key) =>
        Table.TryGetValue(key, out var v) ? (Language == "de" ? v.De : v.En) : key;

    public static string F(string key, params object?[] args) =>
        string.Format(Language == "de" ? CultureInfo.GetCultureInfo("de-DE") : CultureInfo.GetCultureInfo("en-US"), T(key), args);

    public static IEnumerable<string> Keys => Table.Keys;

    static readonly Dictionary<string, (string De, string En)> Table = new (string Key, string De, string En)[]
    {
        // Allgemein und Navigation
        ("App.Title", "HeadsetBridge für Xbox", "HeadsetBridge for Xbox"),
        ("Nav.Connection", "Verbindung", "Connection"),
        ("Nav.Diagnostics", "Diagnose", "Diagnostics"),
        ("Nav.HelpHeader", "Hilfe und Informationen", "Help and information"),
        ("Nav.Manual", "Anleitung", "Guide"),
        ("Nav.Licenses", "Lizenzen", "Licenses"),
        ("Nav.About", "Über HeadsetBridge", "About HeadsetBridge"),
        ("Nav.Settings", "Einstellungen", "Settings"),
        ("Common.Open", "Öffnen", "Open"),

        // Phasen
        ("Phase.Idle", "Bereit", "Ready"),
        ("Phase.Connecting", "Verbinden", "Connecting"),
        ("Phase.Connected", "Verbunden", "Connected"),
        ("Phase.Stopping", "Trennen", "Disconnecting"),
        ("Phase.Waiting", "Erneuter Versuch", "Retrying"),
        ("Phase.Error", "Fehler", "Error"),
        ("Phase.Auth", "Anmeldung", "Sign-in"),

        // Status
        ("Status.Idle.Title", "Bereit für Xbox-Audio", "Ready for Xbox audio"),
        ("Status.Idle.Detail", "Schalte deine Xbox ein und wähle in Windows das gewünschte Ausgabegerät.", "Turn on your Xbox and choose your preferred output device in Windows."),
        ("Status.Preparing.Title", "Verbindung wird aufgebaut …", "Connecting …"),
        ("Status.Preparing.Detail", "Anmeldung und Xbox-Verbindung werden vorbereitet.", "Preparing sign-in and the Xbox connection."),
        ("Status.Auth.Title", "Microsoft-Anmeldung erforderlich", "Microsoft sign-in required"),
        ("Status.Auth.Detail", "Gib den Code aus dem Anmeldefenster auf der geöffneten Microsoft-Seite ein.", "Enter the code from the sign-in window on the Microsoft page that opened."),
        ("Status.Authenticated.Title", "Deine Xbox wird verbunden …", "Connecting to your Xbox …"),
        ("Status.Authenticated.Detail", "Angemeldet. Die Xbox bereitet deinen Audiostream vor.", "Signed in. Your Xbox is preparing the audio stream."),
        ("Status.Linked.Title", "Verbunden. Gleich kommt der Ton.", "Connected. Audio is on its way."),
        ("Status.Playing.Title", "Xbox-Audio wird wiedergegeben", "Playing Xbox audio"),
        ("Status.Playing.Detail", "Der Ton läuft über dein Windows-Ausgabegerät. Bild und Controller bleiben an der Xbox.", "Audio plays through your Windows output device. Video and controller stay on the Xbox."),
        ("Status.Waiting.Title", "Verbindung wird erneut aufgebaut", "Reconnecting"),
        ("Status.Waiting.AgentError", "Die Xbox konnte den Stream nicht starten. Prüfe die Remotefeatures und beende andere Remote-Play-Verbindungen.", "The Xbox could not start the stream. Check the remote features and end other Remote Play sessions."),
        ("Status.Waiting.Detail", "Die Verbindung wurde unterbrochen. HeadsetBridge versucht es automatisch erneut.", "The connection was interrupted. HeadsetBridge will retry automatically."),
        ("Status.Stopping.Title", "Verbindung wird beendet …", "Disconnecting …"),
        ("Status.Stopping.Detail", "Deine Xbox-Sitzung wird geschlossen.", "Closing your Xbox session."),
        ("Status.Error.Title", "Verbindung konnte nicht gestartet werden", "The connection could not be started"),
        ("Error.EngineMissing", "Die Audio-Engine wurde nicht gefunden: {0}", "The audio engine was not found: {0}"),
        ("Error.EngineStart", "Die Audio-Engine konnte nicht gestartet werden: {0}", "The audio engine could not be started: {0}"),
        ("Device.Default", "Windows-Standardgerät", "Windows default device"),

        // Diagnose-Ereignisse (aus dem Engine-Protokoll)
        ("Event.Started", "Audio-Dienst gestartet.", "Audio service started."),
        ("Event.WebRtc", "Verbindungsverfahren: WebRTC.", "Connection method: WebRTC."),
        ("Event.Output", "Audioausgabe: {0}.", "Audio output: {0}."),
        ("Event.SignIn", "Microsoft-Anmeldung im Browser erforderlich. Bitte folge dem Anmeldefenster.", "Microsoft sign-in required in the browser. Please follow the sign-in window."),
        ("Event.Authenticated", "Microsoft-/Xbox-Anmeldung erfolgreich.", "Microsoft/Xbox sign-in successful."),
        ("Event.SessionCreated", "Remote-Play-Sitzung angefordert.", "Remote Play session requested."),
        ("Event.Provisioning", "Xbox bereitet die Übertragung vor.", "The Xbox is preparing the stream."),
        ("Event.Provisioned", "Xbox ist für die Übertragung bereit.", "The Xbox is ready to stream."),
        ("Event.ProvisionFailed", "Xbox konnte die Übertragung nicht vorbereiten.", "The Xbox could not prepare the stream."),
        ("Event.Format", "Audioformat ausgehandelt: Opus, 48 kHz, Stereo.", "Audio format negotiated: Opus, 48 kHz, stereo."),
        ("Event.Connected", "Verbindung zur Xbox hergestellt.", "Connected to the Xbox."),
        ("Event.FirstAudio", "Erste Audiodaten empfangen. Wiedergabe gestartet.", "First audio received. Playback started."),
        ("Event.AgentError", "Xbox hat den Streamstart abgebrochen (AgentCommandError). Die Verbindung wird erneut versucht.", "The Xbox aborted the stream start (AgentCommandError). Retrying the connection."),
        ("Event.Interrupted", "Verbindung unterbrochen. Ein erneuter Verbindungsaufbau wird versucht.", "Connection interrupted. Trying to reconnect."),
        ("Event.StopRequested", "Beenden der Übertragung angefordert.", "Stopping the stream was requested."),
        ("Event.Closed", "Xbox-Verbindung geschlossen.", "Xbox connection closed."),
        ("Event.Stopped", "Audio-Dienst beendet.", "Audio service stopped."),
        ("Event.Stats", "Audioempfang: {0} Pakete/s, {1} kbit/s.", "Audio reception: {0} packets/s, {1} kbit/s."),

        ("License.General", "Projekt und allgemeine Hinweise", "Project and general notices"),

        // Seite Verbindung
        ("Conn.Description", "Xbox-Audio über die Windows-Audioausgabe deiner Wahl – Lautsprecher, Kopfhörer, HDMI oder Audio-Interface.", "Xbox audio through the Windows output of your choice – speakers, headphones, HDMI or an audio interface."),
        ("Conn.Connect", "Mit Xbox verbinden", "Connect to Xbox"),
        ("Conn.Disconnect", "Verbindung trennen", "Disconnect"),
        ("Conn.Disconnecting", "Wird getrennt …", "Disconnecting …"),
        ("Conn.PathOutput", "Audioausgabe", "Audio output"),
        ("Conn.AudioOnly", "Nur Audio – Bild und Controller bleiben an der Xbox", "Audio only – video and controller stay on the Xbox"),
        ("Conn.AudioSection", "Audio", "Audio"),
        ("Conn.Output", "Audioausgabe", "Audio output"),
        ("Conn.OutputDesc", "Windows-Standardgerät – ein Wechsel wird automatisch übernommen", "Windows default device – changes are picked up automatically"),
        ("Conn.Format", "Audioformat", "Audio format"),
        ("Conn.FormatUnavailable", "Bei aktiver Verbindung verfügbar", "Available while connected"),
        ("Conn.NotConnected", "Nicht verbunden", "Not connected"),
        ("Conn.Buffer", "{0} ms Audiopuffer", "{0} ms audio buffer"),
        ("Conn.Packets", "{0:0} Pakete/s", "{0:0} packets/s"),
        ("Conn.Volume", "Lautstärke und Ausgabegerät", "Volume and output device"),
        ("Conn.VolumeDesc", "Die Lautstärke regelst du über Windows oder dein Ausgabegerät", "Adjust the volume in Windows or on your output device"),
        ("Conn.OpenSound", "Soundeinstellungen öffnen", "Open sound settings"),
        ("Conn.Hint", "Minimieren lässt Audio weiterlaufen. Schließen beendet die Übertragung.", "Minimizing keeps audio playing. Closing ends the stream."),
        ("Info.EngineMissing.Title", "Audio-Engine fehlt", "Audio engine missing"),
        ("Info.EngineMissing.Text", "headsetbridge-engine.exe wurde im Programmordner nicht gefunden. Bitte HeadsetBridge neu installieren.", "headsetbridge-engine.exe was not found in the program folder. Please reinstall HeadsetBridge."),
        ("Info.Auth.Title", "Anmeldung im Browser", "Sign in in your browser"),
        ("Info.Auth.Text", "Ein Anmeldefenster mit Code wurde geöffnet. Gib den Code auf der Microsoft-Seite ein und melde dich mit dem Konto deiner Xbox an.", "A sign-in window with a code has opened. Enter the code on the Microsoft page and sign in with your Xbox account."),
        ("Info.Waiting.Title", "Erneuter Verbindungsversuch", "Retrying the connection"),
        ("Info.Waiting.Text", "Prüfe bei Bedarf die Remotefeatures der Xbox.", "If needed, check the remote features on your Xbox."),
        ("Info.Error.Title", "Fehler", "Error"),
        ("Info.Error.Text", "Details findest du unter „Diagnose“.", "See “Diagnostics” for details."),

        // Seite Diagnose
        ("Diag.Description", "Status und wichtige Ereignisse der aktuellen oder letzten Verbindung – verständlich zusammengefasst.", "Status and key events of the current or last connection – summarized in plain language."),
        ("Diag.Connection", "Verbindung", "Connection"),
        ("Diag.Output", "Audioausgabe", "Audio output"),
        ("Diag.Events", "Verbindungsereignisse", "Connection events"),
        ("Diag.Copy", "Protokoll kopieren", "Copy log"),
        ("Diag.Copied", "Kopiert", "Copied"),
        ("Diag.EmptyTitle", "Noch keine Verbindungsereignisse", "No connection events yet"),
        ("Diag.EmptyText", "Starte eine Verbindung, um den Ablauf hier zu sehen.", "Start a connection to see what happens here."),
        ("Diag.Footer", "Die Ansicht fasst relevante technische Ereignisse zusammen und aktualisiert sich automatisch. Gerätebezeichnungen und Fehlercodes werden unverändert angezeigt. Anmeldecodes und Zugangsdaten werden hier nicht ausgegeben.", "This view summarizes relevant technical events and updates automatically. Device names and error codes are shown unchanged. Sign-in codes and credentials are never shown here."),
        ("Diag.Level.Success", "Erfolg", "Success"),
        ("Diag.Level.Warning", "Hinweis", "Warning"),
        ("Diag.Level.Error", "Fehler", "Error"),
        ("Diag.Level.Info", "Information", "Information"),
        ("Diag.CopyHeader", "HeadsetBridge {0} – Diagnose", "HeadsetBridge {0} – Diagnostics"),
        ("Diag.CopyStatus", "Status: {0}", "Status: {0}"),
        ("Diag.CopyOutput", "Audioausgabe: {0}", "Audio output: {0}"),

        // Seite Anleitung
        ("Man.Description", "Xbox-Audio über deinen PC wiedergeben – auf dem Ausgabegerät deiner Wahl.", "Play Xbox audio through your PC – on the output device of your choice."),
        ("Man.Setup", "Einrichten", "Setup"),
        ("Man.Step1.Title", "1. Xbox vorbereiten", "1. Prepare your Xbox"),
        ("Man.Step1.Text", "Konsole einschalten. Unter Einstellungen › Geräte und Verbindungen › Remotefeatures die Remotefeatures aktivieren.", "Turn on the console. Enable remote features under Settings › Devices & connections › Remote features."),
        ("Man.Step2.Title", "2. Im selben Heimnetz verbinden", "2. Use the same home network"),
        ("Man.Step2.Text", "PC und Xbox mit demselben Netzwerk verbinden. Andere Remote-Play-Sitzungen, etwa im Browser, beenden.", "Connect your PC and Xbox to the same network. End other Remote Play sessions, for example in your browser."),
        ("Man.Step3.Title", "3. Audioausgabe wählen", "3. Choose the audio output"),
        ("Man.Step3.Text", "In den Windows-Soundeinstellungen das Standard-Ausgabegerät wählen: Lautsprecher, Kopfhörer, HDMI oder Audio-Interface.", "Pick the default output device in the Windows sound settings: speakers, headphones, HDMI or an audio interface."),
        ("Man.Step4.Title", "4. Verbinden", "4. Connect"),
        ("Man.Step4.Text", "Unter „Verbindung“ auf „Mit Xbox verbinden“ klicken. Beim ersten Mal den Code aus dem Anmeldefenster auf der Microsoft-Seite eingeben und mit dem Xbox-Konto anmelden.", "On “Connection”, click “Connect to Xbox”. The first time, enter the code from the sign-in window on the Microsoft page and sign in with your Xbox account."),
        ("Man.SoundSettings", "Soundeinstellungen", "Sound settings"),
        ("Man.Faq", "Fragen und Hinweise", "Questions and tips"),
        ("Man.Q1.Title", "Anmelden und verbinden", "Signing in and connecting"),
        ("Man.Q1.Text", "Öffne die Seite „Verbindung“ und klicke auf „Mit Xbox verbinden“. Beim ersten Start öffnet sich eine Microsoft-Anmeldeseite. Gib dort den Code aus dem Anmeldefenster ein und melde dich mit dem Konto deiner Xbox an. Die Anmeldung wird auf diesem PC gespeichert. Die App fragt nie nach einem Passwort.", "Open the “Connection” page and click “Connect to Xbox”. On first use, a Microsoft sign-in page opens. Enter the code from the sign-in window there and sign in with your Xbox account. The sign-in is saved on this PC. The app never asks for a password."),
        ("Man.Q2.Title", "Audioausgabe und Lautstärke", "Audio output and volume"),
        ("Man.Q2.Text", "HeadsetBridge folgt dem Windows-Standardgerät und übernimmt einen Gerätewechsel automatisch. Die Lautstärke regelst du über Windows oder dein Ausgabegerät. Es wird nur Ton übertragen; Bild und Controller bleiben an der Xbox.", "HeadsetBridge follows the Windows default device and picks up device changes automatically. Adjust the volume in Windows or on your output device. Only audio is streamed; video and controller stay on the Xbox."),
        ("Man.Q3.Title", "Minimieren, trennen und schließen", "Minimizing, disconnecting and closing"),
        ("Man.Q3.Text", "Minimieren lässt den Ton weiterlaufen. „Verbindung trennen“ beendet die Übertragung. Beim Schließen des Fensters wird die Xbox-Sitzung ebenfalls beendet.", "Minimizing keeps audio playing. “Disconnect” ends the stream. Closing the window also ends the Xbox session."),
        ("Man.Q4.Title", "Wenn kein Ton ankommt", "If there is no sound"),
        ("Man.Q4.Text", "Prüfe zuerst die Windows-Audioausgabe und die Lautstärke. Unter „Diagnose“ findest du den aktuellen Status und die wichtigsten Ereignisse. Scheitert der Verbindungsaufbau, teste offizielles Xbox Remote Play im Browser (xbox.com/remoteplay) und beende diese Sitzung danach wieder.", "First check the Windows audio output and volume. “Diagnostics” shows the current status and key events. If connecting fails, test official Xbox Remote Play in your browser (xbox.com/remoteplay) and end that session afterwards."),
        ("Man.Q5.Title", "Nach einer Windows-Neuinstallation", "After reinstalling Windows"),
        ("Man.Q5.Text", "Sichere das Setup außerhalb deiner Windows-Installation, etwa auf einem USB-Stick. Führe es nach der Neuinstallation aus und melde dich erneut mit deinem Xbox-Konto an. Xbox- und Router-Einstellungen bleiben erhalten, sofern du diese Geräte nicht zurücksetzt. Zusätzliche Laufzeitumgebungen werden nicht benötigt.", "Keep a copy of the setup outside your Windows installation, for example on a USB drive. Run it after reinstalling and sign in again with your Xbox account. Xbox and router settings are kept unless you reset those devices. No additional runtimes are required."),
        ("Man.Q6.Title", "Gespeicherte Anmeldung und Daten", "Saved sign-in and data"),
        ("Man.Q6.Text", "Die Xbox-Anmeldung liegt unter %LOCALAPPDATA%\\HeadsetBridge\\engine\\account.json. Diese Datei enthält Zugangsdaten und sollte nicht weitergegeben werden. Die App speichert ihre Einstellungen unter %LOCALAPPDATA%\\HeadsetBridge. Die Installationsdateien enthalten keine persönlichen Kontodaten.", "The Xbox sign-in is stored in %LOCALAPPDATA%\\HeadsetBridge\\engine\\account.json. This file contains credentials and should not be shared. The app stores its settings in %LOCALAPPDATA%\\HeadsetBridge. The installation files contain no personal account data."),

        // Seite Einstellungen
        ("Set.Description", "Sprache, Darstellung und Verknüpfungen zu Windows.", "Language, appearance and shortcuts to Windows."),
        ("Set.Appearance", "Darstellung", "Appearance"),
        ("Set.Language", "Sprache", "Language"),
        ("Set.LanguageDesc", "„System“ folgt der Windows-Anzeigesprache", "“System” follows the Windows display language"),
        ("Set.Theme", "App-Design", "App theme"),
        ("Set.ThemeDesc", "„System“ folgt dem Windows-App-Modus", "“System” follows the Windows app mode"),
        ("Set.ThemeSystem", "System", "System"),
        ("Set.ThemeLight", "Hell", "Light"),
        ("Set.ThemeDark", "Dunkel", "Dark"),
        ("Set.Windows", "Windows", "Windows"),
        ("Set.Sound", "Sound", "Sound"),
        ("Set.SoundDesc", "Standard-Ausgabegerät und Lautstärke festlegen", "Set the default output device and volume"),
        ("Set.Colors", "Farben", "Colors"),
        ("Set.ColorsDesc", "Windows-Modus und Akzentfarbe, die HeadsetBridge übernimmt", "Windows mode and accent color used by HeadsetBridge"),

        // Seite Lizenzen
        ("Lic.Description", "Verwendete Open-Source-Komponenten und ihre Lizenzbedingungen. HeadsetBridge selbst steht unter der MIT-Lizenz. Die rechtlichen Originaltexte bleiben in ihrer Originalsprache, überwiegend Englisch.", "Open-source components used and their license terms. HeadsetBridge itself is licensed under the MIT License. The legal texts are shown in their original language."),
        ("Lic.Search", "Komponente oder Lizenz suchen", "Search components or licenses"),
        ("Lic.Loading", "Lizenztexte werden geladen …", "Loading license texts …"),
        ("Lic.NoMatches", "Keine passenden Einträge.", "No matching entries."),

        // Seite Über
        ("About.Description", "Überträgt Xbox-Audio an die Windows-Audioausgabe deiner Wahl.", "Streams Xbox audio to the Windows output of your choice."),
        ("About.Body", "Basierend auf dem Open-Source-Projekt HeadsetBridge for Xbox. Microsoft- und Xbox-Dienste werden für Anmeldung und Verbindung benötigt. Deine gespeicherte Anmeldung bleibt auf diesem PC.", "Based on the open-source project HeadsetBridge for Xbox. Microsoft and Xbox services are required for sign-in and connection. Your saved sign-in stays on this PC."),
        ("About.Version", "Version {0} · Windows App SDK · WinUI 3", "Version {0} · Windows App SDK · WinUI 3"),
        ("About.Links", "Links", "Links"),
        ("About.Upstream", "Originalprojekt", "Original project"),
        ("About.UpstreamDesc", "HeadsetBridge for Xbox auf GitHub", "HeadsetBridge for Xbox on GitHub"),
        ("About.RemotePlay", "Xbox Remote Play", "Xbox Remote Play"),
        ("About.RemotePlayDesc", "Verbindung im Browser testen", "Test the connection in your browser"),
        ("About.Disclaimer", "Keine offizielle Microsoft-Anwendung. Xbox ist eine Marke von Microsoft.", "Not an official Microsoft application. Xbox is a trademark of Microsoft."),
    }.ToDictionary(e => e.Key, e => (e.De, e.En));
}
