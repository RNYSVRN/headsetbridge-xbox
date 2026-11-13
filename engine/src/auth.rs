//! Sign-in without a browser engine: the `Msal` flow of xal-node (requests are not signed).
//!
//!   1. user token: refresh_token → login.microsoftonline.com (saved: it rotates)
//!   2. user.auth.xboxlive.com (RPS) → 3. XSTS for http://gssv.xboxlive.com/
//!   4. xhome.gssv-play-prod.xboxlive.com/v2/login/user → gsToken and region
//!   5. login.live.com → lpt (the token for /connect)
//!   6. without `--console`: xccs, the first console that is on; 7. if none is on, Power/WakeUp
//!
//! The refresh token lives in `account.json` (see paths.rs); it rotates on every
//! use and is saved under a lock file and by rename. If there is none, the
//! Greenlight sign-in is imported once (`%APPDATA%\greenlight-desktop\config.json`,
//! key `user.tokenstore`, JSON inside JSON; read only). Otherwise the device-code
//! sign-in starts (microsoft.com/link). No secret value is ever logged.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::http::{Http, Response};
use crate::log::log;
use crate::paths::account_file;
use crate::xhome::Credentials;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

const CLIENT_ID: &str = "1f907974-e22b-4810-a9de-d9647380c97e";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const SCOPE: &str = "xboxlive.signin openid profile offline_access";
const XBL_HEADERS: [(&str, &str); 3] = [("x-xbl-contract-version", "1"), ("Origin", "https://www.xbox.com"), ("Referer", "https://www.xbox.com/")];
const DEVICE_LIST_URL: &str = "https://xccs.xboxlive.com/lists/devices?queryCurrentDevice=false&includeStorageDevices=true";

fn greenlight_config() -> PathBuf {
    PathBuf::from(std::env::var("APPDATA").unwrap_or_default()).join("greenlight-desktop").join("config.json")
}

/// Signs in and returns the session credentials. `pick_console`: also look up
/// the account's console (the first one that is on), like Greenlight does
/// (xbox-webapi SmartGlass: xccs.xboxlive.com/lists/devices).
pub async fn authenticate(pick_console: bool) -> Result<Credentials> {
    let http = Http::new(Duration::from_secs(15))?;
    // 1. A fresh user token. The refresh token rotates on every use, so read,
    // renew and save happen under the same lock.
    let lock = AccountLock::take()?;
    let user: Value = match saved_refresh_token() {
        Some(refresh) => {
            let body = form_body(&[("client_id", CLIENT_ID), ("grant_type", "refresh_token"), ("refresh_token", &refresh), ("scope", SCOPE)]);
            let r = http.request("POST", TOKEN_URL, &[("Content-Type", "application/x-www-form-urlencoded")], Some(body)).await?;
            if r.ok() {
                serde_json::from_str(&r.body)?
            } else if r.body.contains("invalid_grant") {
                // Expired or revoked: forget it and ask for a new sign-in.
                log(Instant::now(), "sign-in: the saved sign-in is no longer valid (invalid_grant); a new sign-in is required");
                let _ = std::fs::remove_file(account_file());
                drop(lock);
                return sign_in_and_finish(&http, pick_console).await;
            } else {
                return Err(format!("sign-in: {TOKEN_URL} → {}", r.status).into());
            }
        }
        None => {
            drop(lock);
            return sign_in_and_finish(&http, pick_console).await;
        }
    };
    lock.save(user["refresh_token"].as_str().ok_or("no refresh_token")?)?;
    drop(lock);
    finish(&http, user, pick_console).await
}

async fn sign_in_and_finish(http: &Http, pick_console: bool) -> Result<Credentials> {
    let user = device_code_sign_in(http).await?;
    AccountLock::take()?.save(user["refresh_token"].as_str().ok_or("no refresh_token")?)?;
    finish(http, user, pick_console).await
}

/// From the user token on: Xbox Live, XSTS, xHome, lpt and the console.
async fn finish(http: &Http, user: Value, pick_console: bool) -> Result<Credentials> {
    let access = user["access_token"].as_str().ok_or("no access_token")?;
    let refresh = user["refresh_token"].as_str().unwrap_or_default().to_owned();

    // 2-3. Xbox Live and XSTS for the streaming service.
    let xasu = post_json(http, "https://user.auth.xboxlive.com/user/authenticate", &XBL_HEADERS, json!({
        "Properties": { "AuthMethod": "RPS", "RpsTicket": format!("d={access}"), "SiteName": "user.auth.xboxlive.com" },
        "RelyingParty": "http://auth.xboxlive.com", "TokenType": "JWT",
    })).await?;
    let xbl_token = xasu["Token"].as_str().ok_or("no Xbox Live token")?;
    let xsts = post_json(http, "https://xsts.auth.xboxlive.com/xsts/authorize", &XBL_HEADERS, json!({
        "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
        "RelyingParty": "http://gssv.xboxlive.com/", "TokenType": "JWT",
    })).await?;

    // 4. Home streaming token and region.
    let xhome = post_json(http, "https://xhome.gssv-play-prod.xboxlive.com/v2/login/user", &[("x-gssv-client", "XboxComBrowser")],
        json!({ "token": xsts["Token"].as_str().ok_or("no XSTS token")?, "offeringId": "xhome" })).await?;
    let region = xhome["offeringSettings"]["regions"].as_array().and_then(|r| r.iter().find(|r| r["isDefault"] == true))
        .ok_or("no default region")?;
    let host = region["baseUri"].as_str().ok_or("no baseUri")?.trim_start_matches("https://").trim_end_matches('/').to_owned();

    // 5. lpt for /connect.
    let lpt = post_form(http, "https://login.live.com/oauth20_token.srf", &[
        ("client_id", CLIENT_ID), ("scope", "service::http://Passport.NET/purpose::PURPOSE_XBOX_CLOUD_CONSOLE_TRANSFER_TOKEN"),
        ("grant_type", "refresh_token"), ("refresh_token", &refresh),
    ]).await?;

    // 6. Web XSTS (for xccs: console list and wake-up) and the console, unless given.
    let web = post_json(http, "https://xsts.auth.xboxlive.com/xsts/authorize", &XBL_HEADERS, json!({
        "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
        "RelyingParty": "http://xboxlive.com", "TokenType": "JWT",
    })).await?;
    let uhs = web["DisplayClaims"]["xui"][0]["uhs"].as_str().ok_or("no uhs")?;
    let token = web["Token"].as_str().ok_or("no web token")?;
    let xccs = format!("XBL3.0 x={uhs};{token}");
    let mut woken_up = false;
    let console = if pick_console {
        let list = checked(DEVICE_LIST_URL, http.request("GET", DEVICE_LIST_URL, &xccs_headers(&xccs), None).await?)?;
        let consoles = list["result"].as_array().cloned().unwrap_or_default();
        let chosen = consoles.iter().find(|c| c["powerState"] == "On").or(consoles.first());
        let id = chosen.and_then(|c| c["id"].as_str()).map(str::to_owned);
        // 7. If none is on, wake the first one up (from "ConnectedStandby"; in
        // energy-saving mode it does not answer) and wait until it reports "On",
        // like Greenlight's power button (xbox-webapi, smartglass.js).
        if let (Some(id), Some(c)) = (&id, chosen) {
            if c["powerState"] != "On" {
                wake_up(http, &xccs, id).await?;
                woken_up = true;
            }
        }
        id
    } else { None };

    Ok(Credentials {
        host,
        region: region["name"].as_str().unwrap_or("").to_owned(),
        gs_token: xhome["gsToken"].as_str().ok_or("no gsToken")?.to_owned(),
        lpt: lpt["access_token"].as_str().ok_or("no lpt")?.to_owned(),
        console,
        woken_up,
        xccs: Some(xccs),
    })
}

/// Is the console on (`powerState` "On")? Without `console`, any console of the
/// account. Only looks: it does not wake the console up (if the user switched it
/// off, it stays off until they switch it on).
pub async fn console_is_on(authorization: &str, console: Option<&str>) -> Result<bool> {
    let http = Http::new(Duration::from_secs(10))?;
    let list = checked(DEVICE_LIST_URL, http.request("GET", DEVICE_LIST_URL, &xccs_headers(authorization), None).await?)?;
    let consoles = list["result"].as_array().cloned().unwrap_or_default();
    Ok(consoles.iter().any(|c| console.is_none_or(|id| c["id"] == id) && c["powerState"] == "On"))
}

/// The xccs (SmartGlass) headers for the console list and commands.
fn xccs_headers(authorization: &str) -> [(&str, &str); 5] {
    [("Authorization", authorization), ("x-xbl-contract-version", "4"), ("skillplatform", "RemoteManagement"), ("Accept", "application/json"), ("Accept-Language", "en-US")]
}

/// `Power/WakeUp` through xccs, then waits (up to 60 s) until the list reports "On".
async fn wake_up(http: &Http, authorization: &str, id: &str) -> Result<()> {
    let url = "https://xccs.xboxlive.com/commands";
    let body = json!({
        "destination": "Xbox", "type": "Power", "command": "WakeUp",
        "sessionId": uuid::Uuid::new_v4().to_string(), "sourceId": "com.microsoft.smartglass",
        "parameters": [], "linkedXboxId": id,
    });
    let mut headers = xccs_headers(authorization).to_vec();
    headers.push(("Content-Type", "application/json"));
    checked(url, http.request("POST", url, &headers, Some(body.to_string())).await?)?;
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        tokio::time::sleep(Duration::from_secs(3)).await;
        let list = checked(DEVICE_LIST_URL, http.request("GET", DEVICE_LIST_URL, &xccs_headers(authorization), None).await?)?;
        let on = list["result"].as_array().into_iter().flatten()
            .any(|c| c["id"].as_str() == Some(id) && c["powerState"] == "On");
        if on { return Ok(()) }
    }
    Err("the console did not turn on within 60 s (energy-saving power mode?)".into())
}

async fn post_form(http: &Http, url: &str, fields: &[(&str, &str)]) -> Result<Value> {
    let headers = [("Content-Type", "application/x-www-form-urlencoded"), ("Cache-Control", "no-store, must-revalidate, no-cache")];
    checked(url, http.request("POST", url, &headers, Some(form_body(fields))).await?)
}

async fn post_json(http: &Http, url: &str, headers: &[(&str, &str)], body: Value) -> Result<Value> {
    let mut all = vec![("Content-Type", "application/json"), ("Cache-Control", "no-cache")];
    all.extend_from_slice(headers);
    checked(url, http.request("POST", url, &all, Some(body.to_string())).await?)
}

/// The status and the URL on failure, never the body (it contains tokens).
fn checked(url: &str, r: Response) -> Result<Value> {
    if !r.ok() { return Err(format!("sign-in: {url} → {}", r.status).into()) }
    Ok(serde_json::from_str(&r.body)?)
}

fn url_encode(text: &str) -> String {
    text.bytes().map(|b| match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
        _ => format!("%{b:02X}"),
    }).collect()
}

fn form_body(fields: &[(&str, &str)]) -> String {
    fields.iter().map(|(k, v)| format!("{}={}", url_encode(k), url_encode(v))).collect::<Vec<_>>().join("&")
}

fn read_greenlight_token_store(config: &Path) -> Result<Value> {
    let c: Value = serde_json::from_str(&std::fs::read_to_string(config)?).map_err(|_| "Greenlight's config.json is not valid JSON")?;
    let text = c["user"]["tokenstore"].as_str().ok_or("no Greenlight sign-in")?;
    serde_json::from_str(text).map_err(|_| "Greenlight's token store is not valid JSON".into())
}

/// The refresh token: our own or, the first time, Greenlight's (read only).
fn saved_refresh_token() -> Option<String> {
    crate::paths::migrate_legacy();
    let own = std::fs::read_to_string(account_file()).ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v["refresh_token"].as_str().map(str::to_owned));
    own.or_else(|| read_greenlight_token_store(&greenlight_config()).ok().and_then(|a| a["userToken"]["refresh_token"].as_str().map(str::to_owned)))
}

/// The lock of `account.json`: taken by whoever reads, renews and saves the
/// (rotating) token, so two sign-ins at the same time do not overwrite each other.
struct AccountLock(PathBuf);

impl AccountLock {
    fn take() -> Result<Self> {
        let account = account_file();
        if let Some(dir) = account.parent() { std::fs::create_dir_all(dir)? }
        let lock = account.with_extension("json.lock");
        let start = Instant::now();
        loop {
            match std::fs::OpenOptions::new().write(true).create_new(true).open(&lock) {
                Ok(_) => return Ok(Self(lock)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    // A lock older than 30 s belongs to a process that died.
                    if std::fs::metadata(&lock).and_then(|m| m.modified()).map(|t| t.elapsed().unwrap_or_default() > Duration::from_secs(30)).unwrap_or(false) {
                        let _ = std::fs::remove_file(&lock);
                    }
                    if start.elapsed() > Duration::from_secs(20) { return Err("the saved sign-in is busy".into()) }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// Saves the new refresh token by rename, so the file is never half-written.
    fn save(&self, refresh: &str) -> Result<()> {
        let account = account_file();
        let temp = account.with_extension(format!("json.{}.tmp", std::process::id()));
        std::fs::write(&temp, json!({ "refresh_token": refresh }).to_string())?;
        std::fs::rename(&temp, &account)?;
        Ok(())
    }
}

impl Drop for AccountLock {
    fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); }
}

/// No saved sign-in: device code. Opens microsoft.com/link in the browser and a
/// small dialog with the code (the exe has no console), then polls until the
/// user has entered it, at most as long as Microsoft allows (15 min).
async fn device_code_sign_in(http: &Http) -> Result<Value> {
    let d = post_form(http, "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode", &[("client_id", CLIENT_ID), ("scope", SCOPE)]).await?;
    let user_code = d["user_code"].as_str().ok_or("no user_code")?.to_owned();
    let device_code = d["device_code"].as_str().ok_or("no device_code")?.to_owned();
    let url = d["verification_uri"].as_str().unwrap_or("https://www.microsoft.com/link").to_owned();
    let mut interval = d["interval"].as_u64().unwrap_or(5).max(1);
    let deadline = Instant::now() + Duration::from_secs(d["expires_in"].as_u64().unwrap_or(900));
    log(Instant::now(), &format!("sign-in: no saved sign-in; sign in at {url} with the code from the sign-in window"));
    show_code(&url, &user_code);
    while Instant::now() < deadline {
        tokio::time::sleep(Duration::from_secs(interval)).await;
        let body = form_body(&[("grant_type", "urn:ietf:params:oauth:grant-type:device_code"), ("client_id", CLIENT_ID), ("device_code", device_code.as_str())]);
        let r = http.request("POST", TOKEN_URL, &[("Content-Type", "application/x-www-form-urlencoded")], Some(body)).await?;
        if r.ok() { return Ok(serde_json::from_str(&r.body)?) }
        let error = serde_json::from_str::<Value>(&r.body).ok().and_then(|v| v["error"].as_str().map(str::to_owned)).unwrap_or_default();
        match error.as_str() {
            "authorization_pending" => {}
            "slow_down" => interval += 5,
            "authorization_declined" => return Err("sign-in: declined on the Microsoft page".into()),
            "expired_token" => return Err("sign-in: the code expired; please start again".into()),
            other => return Err(format!("sign-in: {other} ({})", r.status).into()),
        }
    }
    Err("sign-in: the code expired without being used".into())
}

/// Opens the page in the browser and shows the code in a dialog (on its own
/// thread, so polling continues). Outside Windows, only the log line.
#[cfg(windows)]
fn show_code(url: &str, code: &str) {
    use windows::core::HSTRING;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK, MB_SETFOREGROUND, MB_TOPMOST, SW_SHOWNORMAL};
    unsafe { ShellExecuteW(None, &HSTRING::from("open"), &HSTRING::from(url), None, None, SW_SHOWNORMAL); }
    let (title, text) = crate::lang::sign_in_dialog(url, code);
    std::thread::spawn(move || unsafe {
        MessageBoxW(None, &HSTRING::from(text), &HSTRING::from(title), MB_OK | MB_ICONINFORMATION | MB_TOPMOST | MB_SETFOREGROUND);
    });
}

#[cfg(not(windows))]
fn show_code(_: &str, _: &str) {}
