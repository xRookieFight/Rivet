//! Account management: OAuth device flow, personal access tokens, and token
//! storage in the OS keychain (Keychain / Credential Manager / Secret Service).
//!
//! Only the login and avatar live in the preferences file; tokens never touch disk.

use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

const KEYRING_SERVICE: &str = "dev.rivet.github";

#[cfg(not(target_os = "android"))]
use keyring::Entry;
// keyring's v1 wrapper refuses to run on android, so there we talk to
// keyring-core directly and use the keystore backed store `platform` installs
#[cfg(target_os = "android")]
use keyring_core::Entry;

/// Scopes the app needs to cover repos, orgs, notifications, actions and deletes.
pub const SCOPES: &str = "repo read:org notifications workflow user delete_repo gist";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Account {
    pub login: String,
    pub avatar_url: String,
}

/// Rivet's own OAuth app. Device flow only needs the client id, which is public
/// by design (there's no secret involved), so it ships in the binary.
const DEFAULT_CLIENT_ID: &str = "Ov23li2GcHkfOtEOpYqj";

/// OAuth app client id for the device flow. RIVET_GITHUB_CLIENT_ID at runtime
/// or build time wins over the built in one, handy for forks with their own app.
pub fn client_id() -> String {
    std::env::var("RIVET_GITHUB_CLIENT_ID")
        .ok()
        .or_else(|| option_env!("RIVET_GITHUB_CLIENT_ID").map(str::to_string))
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_CLIENT_ID.to_string())
}

pub fn store_token(login: &str, token: &str) -> Result<()> {
    Entry::new(KEYRING_SERVICE, login)?.set_password(token)?;
    Ok(())
}

pub fn load_token(login: &str) -> Result<String> {
    Ok(Entry::new(KEYRING_SERVICE, login)?.get_password()?)
}

pub fn forget_token(login: &str) -> Result<()> {
    match Entry::new(KEYRING_SERVICE, login)?.delete_credential() {
        Ok(()) => Ok(()),
        // hmm, already gone is fine for a sign out
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[derive(Debug, Clone)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
    pub expires_in: u64,
}

fn http() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .user_agent(concat!("Rivet/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(30))
        .build()
        .expect("http client")
}

/// Step one of the device flow: get a user code to show on screen.
pub fn request_device_code(client_id: &str) -> Result<DeviceCode> {
    let v: Value = http()
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&[("client_id", client_id), ("scope", SCOPES)])
        .send()?
        .json()?;
    if let Some(err) = v["error_description"].as_str().or(v["error"].as_str()) {
        bail!("{err}");
    }
    Ok(DeviceCode {
        device_code: v["device_code"].as_str().ok_or_else(|| anyhow!("missing device_code"))?.into(),
        user_code: v["user_code"].as_str().unwrap_or_default().into(),
        verification_uri: v["verification_uri"].as_str().unwrap_or("https://github.com/login/device").into(),
        interval: v["interval"].as_u64().unwrap_or(5),
        expires_in: v["expires_in"].as_u64().unwrap_or(900),
    })
}

/// Step two: poll until the user approves in the browser. Blocks, so run it on
/// a background thread. `cancel` lets the UI abort it.
pub fn poll_device_token(client_id: &str, code: &DeviceCode, cancel: &std::sync::atomic::AtomicBool) -> Result<String> {
    let mut interval = code.interval.max(1);
    let deadline = std::time::Instant::now() + Duration::from_secs(code.expires_in);
    let client = http();
    while std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_secs(interval));
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            bail!("cancelled");
        }
        let v: Value = client
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .form(&[
                ("client_id", client_id),
                ("device_code", code.device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()?
            .json()?;
        if let Some(tok) = v["access_token"].as_str() {
            return Ok(tok.to_string());
        }
        match v["error"].as_str() {
            Some("authorization_pending") => {}
            // github wants us to back off by 5s each time it says this
            Some("slow_down") => interval = v["interval"].as_u64().unwrap_or(interval + 5),
            Some("expired_token") => bail!("The code expired. Start the sign in again."),
            Some("access_denied") => bail!("Access was denied in the browser."),
            Some(other) => bail!("{}", v["error_description"].as_str().unwrap_or(other)),
            None => bail!("unexpected response from GitHub"),
        }
    }
    bail!("The code expired. Start the sign in again.")
}

/// Reads the token the GitHub CLI already has, if it's installed and signed in.
pub fn token_from_gh_cli() -> Result<String> {
    let out = std::process::Command::new("gh").args(["auth", "token"]).output()?;
    if !out.status.success() {
        bail!("gh is not signed in ({})", String::from_utf8_lossy(&out.stderr).trim());
    }
    let tok = String::from_utf8(out.stdout)?.trim().to_string();
    if tok.is_empty() {
        bail!("gh returned an empty token");
    }
    Ok(tok)
}
