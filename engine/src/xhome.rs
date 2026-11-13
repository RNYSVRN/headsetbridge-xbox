//! xHome HTTP signalling: start a Remote Play session on the console, wait until
//! it is provisioned, exchange SDP and ICE, keep it alive and close it.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

const DEVICE_INFO: &str = r#"{"appInfo":{"env":{"clientAppId":"Microsoft.GamingApp","clientAppType":"native","clientAppVersion":"2203.1001.4.0","clientSdkVersion":"8.5.2","httpEnvironment":"prod","sdkInstallId":""}},"dev":{"hw":{"make":"Microsoft","model":"Surface Pro","sdktype":"native"},"os":{"name":"Windows 11","ver":"22631.2715","platform":"desktop"},"displayInfo":{"dimensions":{"widthInPixels":1920,"heightInPixels":1080},"pixelDensity":{"dpiX":1,"dpiY":1}}}}"#;

/// What auth.rs returns. Never logged.
pub struct Credentials {
    pub host: String,
    pub region: String,
    pub gs_token: String,
    pub lpt: String,
    /// The console chosen by auth.rs when `--console` is not given.
    pub console: Option<String>,
    /// The console was in standby and auth.rs woke it up (Power/WakeUp).
    pub woken_up: bool,
    /// Authorization for xccs (`XBL3.0 x=uhs;token`): checking whether the console is on.
    pub xccs: Option<String>,
}

/// Cloneable, so requests can run in parallel without blocking the session loop.
#[derive(Clone)]
pub struct XHome {
    http: crate::http::Http,
    base: String,
    token: String,
    lpt: String,
    pub session: Option<String>,
}

impl XHome {
    pub fn new(c: Credentials) -> Result<Self> {
        Ok(Self {
            http: crate::http::Http::new(Duration::from_secs(15))?,
            base: format!("https://{}", c.host),
            token: c.gs_token,
            lpt: c.lpt,
            session: None,
        })
    }

    async fn request(&self, method: &str, path: &str, body: Option<Value>, device_info: bool) -> Result<Value> {
        // 204 means "not yet": repeat after 200 ms for up to 30 s.
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let authorization = format!("Bearer {}", self.token);
            let mut headers = vec![("Content-Type", "application/json"), ("Authorization", authorization.as_str())];
            if device_info { headers.push(("X-MS-Device-Info", DEVICE_INFO)) }
            let r = self.http.request(method, &format!("{}{}", self.base, path), &headers, body.as_ref().map(|b| b.to_string())).await?;
            if r.status == 204 && method == "GET" {
                if Instant::now() > deadline { return Err(format!("{method} {}: no answer from the console", without_ids(path)).into()) }
                tokio::time::sleep(Duration::from_millis(200)).await;
                continue;
            }
            if !r.ok() {
                return Err(format!("{method} {} → {}: {}", without_ids(path), r.status, r.body.chars().take(300).collect::<String>()).into());
            }
            return Ok(serde_json::from_str(&r.body).unwrap_or(Value::Null));
        }
    }

    fn path(&self, suffix: &str) -> String {
        format!("/v5/sessions/home/{}{}", self.session.as_deref().unwrap_or(""), suffix)
    }

    /// POST /play: starts the session on `console`. A session left open by an
    /// earlier run is closed first.
    pub async fn start(&mut self, console: &str) -> Result<()> {
        self.close_abandoned().await;
        let body = json!({
            "titleId": "", "systemUpdateGroup": "", "clientSessionId": "",
            "settings": { "nanoVersion": "V3;WebrtcTransport.dll", "enableTextToSpeech": false, "highContrast": 0,
                "locale": "en-US", "useIceConnection": false, "timezoneOffsetMinutes": 120, "sdkType": "web", "osName": "windows" },
            "serverId": console, "fallbackRegionNames": [],
        });
        let r = self.request("POST", "/v5/sessions/home/play", Some(body), true).await?;
        let path = r["sessionPath"].as_str().ok_or("the console returned no sessionPath")?;
        self.session = path.split('/').nth(3).map(str::to_owned);
        // Written to disk until its DELETE: if the process dies without closing
        // it, the next start closes it. A session without DELETE leaves the
        // console in "remote" mode without a client and blocks /play.
        if let Some(id) = self.session.clone() {
            let _ = on_disk(move || {
                std::fs::create_dir_all(crate::paths::data_dir())?;
                std::fs::write(crate::paths::open_session_file(), id)
            }).await;
        }
        Ok(())
    }

    /// Polls /state until Provisioned; on ReadyToConnect sends the /connect token.
    pub async fn wait_for_console(&self, mut on_state: impl FnMut(&str)) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(90);
        let mut previous = String::new();
        while Instant::now() < deadline {
            let r = self.request("GET", &self.path("/state"), None, false).await?;
            let state = r["state"].as_str().unwrap_or("").to_owned();
            if state != previous { on_state(&state); previous = state.clone() }
            match state.as_str() {
                "Provisioned" => return Ok(()),
                "Failed" => return Err(format!("the console failed: {}", r["errorDetails"]).into()),
                "ReadyToConnect" => { self.request("POST", &self.path("/connect"), Some(json!({ "userToken": self.lpt })), false).await?; }
                _ => {}
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        Err("the console was not ready in time".into())
    }

    /// Sends the SDP offer and returns the console's answer.
    pub async fn exchange_sdp(&self, sdp: &str) -> Result<String> {
        self.request("POST", &self.path("/sdp"), Some(json!({
            "messageType": "offer", "sdp": sdp,
            "configuration": {
                "chatConfiguration": { "bytesPerSample": 2, "expectedClipDurationMs": 20, "format": { "codec": "opus", "container": "webm" }, "numChannels": 1, "sampleFrequencyHz": 24000 },
                "chat": { "minVersion": 1, "maxVersion": 1 }, "control": { "minVersion": 1, "maxVersion": 3 },
                "input": { "minVersion": 1, "maxVersion": 8 }, "message": { "minVersion": 1, "maxVersion": 1 },
            },
        })), false).await?;
        let r = self.request("GET", &self.path("/sdp"), None, false).await?;
        // With the console busy or after a dead session the answer is null and the reason is in errorDetails.
        let exchange = r["exchangeResponse"].as_str().ok_or_else(|| format!("the console did not answer the SDP offer: {}", r["errorDetails"]))?;
        let answer: Value = serde_json::from_str(exchange)?;
        Ok(answer["sdp"].as_str().ok_or("the console rejected the SDP offer")?.to_owned())
    }

    /// Sends our ICE candidates and returns the console's, without the `a=` prefix.
    pub async fn exchange_ice(&self, local: &[String]) -> Result<Vec<String>> {
        let candidates: Vec<Value> = local.iter().map(|c| json!({ "candidate": c, "sdpMLineIndex": 0, "sdpMid": "0" })).collect();
        self.request("POST", &self.path("/ice"), Some(json!({ "messageType": "iceCandidate", "candidate": candidates })), false).await?;
        let r = self.request("GET", &self.path("/ice"), None, false).await?;
        let remote: Vec<Value> = serde_json::from_str(r["exchangeResponse"].as_str().unwrap_or("[]"))?;
        Ok(remote.iter().filter_map(|c| c["candidate"].as_str())
            .map(|c| c.trim_start_matches("a=").to_owned())
            .filter(|c| !c.is_empty() && c != "end-of-candidates").collect())
    }

    pub async fn keepalive(&self) -> Result<()> {
        self.request("POST", &self.path("/keepalive"), Some(json!({})), false).await.map(|_| ())
    }

    /// DELETE: closes the session on the console.
    pub async fn stop(&mut self) -> Result<()> {
        let Some(id) = self.session.clone() else { return Ok(()) };
        let path = self.path("");
        self.session = None;
        let r = self.request("DELETE", &path, None, false).await.map(|_| ());
        if r.is_ok() {
            let _ = on_disk(move || {
                let file = crate::paths::open_session_file();
                if std::fs::read_to_string(&file)?.trim() == id { std::fs::remove_file(file) } else { Ok(()) }
            }).await;
        }
        r
    }

    /// Closes a session left open by an earlier run that died without DELETE.
    async fn close_abandoned(&self) {
        let Ok(id) = on_disk(|| std::fs::read_to_string(crate::paths::open_session_file())).await else { return };
        let id = id.trim().to_owned();
        if id.is_empty() || id.contains('/') {
            let _ = on_disk(|| std::fs::remove_file(crate::paths::open_session_file())).await;
            return;
        }
        let r = self.request("DELETE", &format!("/v5/sessions/home/{id}"), None, false).await;
        // Closed, or it no longer exists (4xx): no need to try again.
        if r.is_ok() || r.as_ref().err().is_some_and(|e| e.to_string().contains("→ 4")) {
            let _ = on_disk(|| std::fs::remove_file(crate::paths::open_session_file())).await;
        }
    }
}

/// File access off the tokio thread, which also runs the live session.
async fn on_disk<T: Send + 'static>(f: impl FnOnce() -> std::io::Result<T> + Send + 'static) -> std::io::Result<T> {
    tokio::task::spawn_blocking(f).await.unwrap_or_else(|e| Err(std::io::Error::other(e)))
}

/// Paths contain the session id: it is not logged.
fn without_ids(path: &str) -> String {
    path.split('/').map(|p| if p.len() >= 32 { "<id>" } else { p }).collect::<Vec<_>>().join("/")
}

#[cfg(test)]
mod tests {
    #[test]
    fn session_ids_are_hidden() {
        assert_eq!(super::without_ids("/v5/sessions/home/0123456789abcdef0123456789abcdef/state"), "/v5/sessions/home/<id>/state");
    }
}
