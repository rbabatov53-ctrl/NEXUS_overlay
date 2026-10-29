use serde::{Deserialize, Serialize};

const CREDENTIAL_TARGET: &str = "NEXUSOverlay.DiscordRPC";

#[derive(Clone, Deserialize, Serialize)]
struct StoredCredentials {
    app_id: String,
    client_secret: String,
    redirect_uri: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_at: u64,
}

#[derive(Clone, Deserialize, Serialize, Default)]
pub struct ConfigSummary {
    pub app_id: Option<String>,
    pub redirect_uri: Option<String>,
    pub configured: bool,
}

#[derive(Clone, Deserialize, Serialize, Default)]
pub struct VoiceStatus {
    pub connected: bool,
    pub channel_name: Option<String>,
    pub self_mute: bool,
    pub self_deaf: bool,
    pub participant_count: usize,
}

#[cfg(windows)]
mod windows_rpc {
    use super::{ConfigSummary, StoredCredentials, VoiceStatus, CREDENTIAL_TARGET};
    use reqwest::blocking::Client;
    use serde::Deserialize;
    use serde_json::{json, Value};
    use std::{
        ffi::c_void,
        fs::{File, OpenOptions},
        io::{Read, Write},
        ptr,
        slice,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    const CRED_TYPE_GENERIC: u32 = 1;
    const CRED_PERSIST_LOCAL_MACHINE: u32 = 2;

    #[repr(C)]
    struct FileTime { low: u32, high: u32 }

    #[repr(C)]
    struct WinCredential {
        flags: u32,
        credential_type: u32,
        target_name: *mut u16,
        comment: *mut u16,
        last_written: FileTime,
        credential_blob_size: u32,
        credential_blob: *mut u8,
        persist: u32,
        attribute_count: u32,
        attributes: *mut c_void,
        target_alias: *mut u16,
        user_name: *mut u16,
    }

    #[link(name = "Advapi32")]
    extern "system" {
        fn CredWriteW(credential: *const WinCredential, flags: u32) -> i32;
        fn CredReadW(target: *const u16, credential_type: u32, flags: u32, credential: *mut *mut WinCredential) -> i32;
        fn CredFree(buffer: *mut c_void);
        fn CredDeleteW(target: *const u16, credential_type: u32, flags: u32) -> i32;
    }

    #[link(name = "user32")]
    extern "system" {
        fn keybd_event(vk: u8, scan: u8, flags: u32, extra_info: usize);
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn save_credentials(credentials: &StoredCredentials) -> Result<(), String> {
        let mut target = wide(CREDENTIAL_TARGET);
        let mut user = wide("NEXUS Overlay");
        let mut blob = serde_json::to_vec(credentials).map_err(|e| e.to_string())?;
        if blob.len() > 5120 { return Err("Credential payload exceeds Windows Credential Manager limit".into()); }
        let record = WinCredential {
            flags: 0,
            credential_type: CRED_TYPE_GENERIC,
            target_name: target.as_mut_ptr(),
            comment: ptr::null_mut(),
            last_written: FileTime { low: 0, high: 0 },
            credential_blob_size: blob.len() as u32,
            credential_blob: blob.as_mut_ptr(),
            persist: CRED_PERSIST_LOCAL_MACHINE,
            attribute_count: 0,
            attributes: ptr::null_mut(),
            target_alias: ptr::null_mut(),
            user_name: user.as_mut_ptr(),
        };
        let result = unsafe { CredWriteW(&record, 0) };
        if result == 0 { return Err(std::io::Error::last_os_error().to_string()); }
        Ok(())
    }

    fn load_credentials() -> Result<StoredCredentials, String> {
        let target = wide(CREDENTIAL_TARGET);
        let mut pointer: *mut WinCredential = ptr::null_mut();
        if unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut pointer) } == 0 {
            return Err("Discord credentials are not saved in Windows Credential Manager".into());
        }
        if pointer.is_null() { return Err("Windows returned an empty credential record".into()); }
        let parsed = unsafe {
            let record = &*pointer;
            if record.credential_blob.is_null() || record.credential_blob_size == 0 {
                Err("Stored Discord credentials are empty".to_string())
            } else {
                let bytes = slice::from_raw_parts(record.credential_blob, record.credential_blob_size as usize);
                serde_json::from_slice::<StoredCredentials>(bytes).map_err(|e| e.to_string())
            }
        };
        unsafe { CredFree(pointer.cast()); }
        parsed
    }

    pub fn summary() -> ConfigSummary {
        load_credentials().map(|credentials| ConfigSummary {
            app_id: Some(credentials.app_id),
            redirect_uri: Some(credentials.redirect_uri),
            configured: true,
        }).unwrap_or_default()
    }

    pub fn save(app_id: String, client_secret: String, redirect_uri: String) -> Result<(), String> {
        let app_id = app_id.trim().to_string();
        let client_secret = client_secret.trim().to_string();
        let redirect_uri = redirect_uri.trim().to_string();
        if app_id.is_empty() || client_secret.is_empty() || redirect_uri.is_empty() {
            return Err("Application ID, client secret and redirect URI are required".into());
        }
        if !app_id.chars().all(|c| c.is_ascii_digit()) {
            return Err("Discord Application ID must contain digits only".into());
        }
        if !redirect_uri.starts_with("https://") && !redirect_uri.starts_with("http://localhost") {
            return Err("Use the exact HTTPS or localhost redirect URI registered in Discord Developer Portal".into());
        }
        save_credentials(&StoredCredentials {
            app_id,
            client_secret,
            redirect_uri,
            access_token: None,
            refresh_token: None,
            expires_at: 0,
        })
    }

    fn pipe_for_app(app_id: &str) -> Result<File, String> {
        let mut last_error = None;
        for index in 0..10 {
            let path = format!(r"\\.\pipe\discord-ipc-{index}");
            match OpenOptions::new().read(true).write(true).open(path) {
                Ok(mut pipe) => {
                    send_packet(&mut pipe, 0, &json!({ "v": 1, "client_id": app_id }))?;
                    let (opcode, ready) = read_packet(&mut pipe)?;
                    if opcode != 1 || ready.get("evt").and_then(Value::as_str) != Some("READY") {
                        last_error = Some(format!("Discord RPC handshake rejected: {ready}"));
                        continue;
                    }
                    return Ok(pipe);
                }
                Err(error) => last_error = Some(error.to_string()),
            }
        }
        Err(last_error.unwrap_or_else(|| "Discord desktop IPC pipe was not found. Is Discord running?".into()))
    }

    fn send_packet(pipe: &mut File, opcode: u32, value: &Value) -> Result<(), String> {
        let payload = serde_json::to_vec(value).map_err(|e| e.to_string())?;
        let length = u32::try_from(payload.len()).map_err(|e| e.to_string())?;
        pipe.write_all(&opcode.to_le_bytes()).map_err(|e| e.to_string())?;
        pipe.write_all(&length.to_le_bytes()).map_err(|e| e.to_string())?;
        pipe.write_all(&payload).map_err(|e| e.to_string())?;
        pipe.flush().map_err(|e| e.to_string())
    }

    fn read_packet(pipe: &mut File) -> Result<(u32, Value), String> {
        let mut header = [0u8; 8];
        pipe.read_exact(&mut header).map_err(|e| e.to_string())?;
        let opcode = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        if length > 2 * 1024 * 1024 { return Err("Discord RPC response exceeded the size limit".into()); }
        let mut body = vec![0; length];
        pipe.read_exact(&mut body).map_err(|e| e.to_string())?;
        let value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
        Ok((opcode, value))
    }

    fn rpc_command(pipe: &mut File, command: &str, args: Value) -> Result<Value, String> {
        static NEXT_NONCE: AtomicU64 = AtomicU64::new(1);
        let nonce = format!("nexus-{}", NEXT_NONCE.fetch_add(1, Ordering::Relaxed));
        send_packet(pipe, 1, &json!({ "cmd": command, "args": args, "nonce": nonce }))?;
        loop {
            let (opcode, response) = read_packet(pipe)?;
            if opcode == 2 { return Err("Discord RPC connection closed".into()); }
            if response.get("nonce").and_then(Value::as_str) != Some(nonce.as_str()) { continue; }
            if response.get("evt").and_then(Value::as_str) == Some("ERROR") {
                let message = response.pointer("/data/message").and_then(Value::as_str).unwrap_or("Discord RPC returned an error");
                return Err(message.to_string());
            }
            return Ok(response.get("data").cloned().unwrap_or(Value::Null));
        }
    }

    fn authenticated_pipe(credentials: &StoredCredentials) -> Result<File, String> {
        let mut pipe = pipe_for_app(&credentials.app_id)?;
        let token = credentials.access_token.as_deref().ok_or_else(|| "Connect Discord first".to_string())?;
        rpc_command(&mut pipe, "AUTHENTICATE", json!({ "access_token": token }))?;
        Ok(pipe)
    }

    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: String,
        refresh_token: Option<String>,
        expires_in: Option<u64>,
    }

    fn token_request(fields: &[(&str, &str)]) -> Result<TokenResponse, String> {
        Client::builder().timeout(std::time::Duration::from_secs(15)).build()
            .map_err(|e| e.to_string())?
            .post("https://discord.com/api/v10/oauth2/token")
            .form(fields)
            .send().map_err(|e| e.to_string())?
            .error_for_status().map_err(|e| e.to_string())?
            .json::<TokenResponse>().map_err(|e| e.to_string())
    }

    fn refresh_if_needed(credentials: &mut StoredCredentials) -> Result<(), String> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_secs();
        if credentials.access_token.is_some() && credentials.expires_at > now + 90 { return Ok(()); }
        let refresh = credentials.refresh_token.as_deref().ok_or_else(|| "Discord authorization expired; connect again".to_string())?;
        let token = token_request(&[
            ("client_id", &credentials.app_id),
            ("client_secret", &credentials.client_secret),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh),
        ])?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_secs();
        credentials.access_token = Some(token.access_token);
        if token.refresh_token.is_some() { credentials.refresh_token = token.refresh_token; }
        credentials.expires_at = now + token.expires_in.unwrap_or(604800);
        save_credentials(credentials)
    }

    pub fn connect() -> Result<VoiceStatus, String> {
        let mut credentials = load_credentials()?;
        let mut pipe = pipe_for_app(&credentials.app_id)?;
        let auth = rpc_command(&mut pipe, "AUTHORIZE", json!({
            "client_id": credentials.app_id,
            "scopes": ["rpc", "identify"]
        }))?;
        let code = auth.get("code").and_then(Value::as_str).ok_or_else(|| "Discord did not return an authorization code".to_string())?;
        let token = token_request(&[
            ("client_id", &credentials.app_id),
            ("client_secret", &credentials.client_secret),
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &credentials.redirect_uri),
        ])?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_secs();
        credentials.access_token = Some(token.access_token);
        credentials.refresh_token = token.refresh_token;
        credentials.expires_at = now + token.expires_in.unwrap_or(604800);
        save_credentials(&credentials)?;
        let mut pipe = authenticated_pipe(&credentials)?;
        voice_status(&mut pipe)
    }

    fn voice_status(pipe: &mut File) -> Result<VoiceStatus, String> {
        let channel = rpc_command(pipe, "GET_SELECTED_VOICE_CHANNEL", json!({}))?;
        let settings = rpc_command(pipe, "GET_VOICE_SETTINGS", json!({}))?;
        let channel_name = channel.get("name").and_then(Value::as_str).map(str::to_string);
        let participant_count = channel.get("voice_states").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
        Ok(VoiceStatus {
            connected: !channel.is_null(),
            channel_name,
            self_mute: settings.get("mute").and_then(Value::as_bool).unwrap_or(false),
            self_deaf: settings.get("deaf").and_then(Value::as_bool).unwrap_or(false),
            participant_count,
        })
    }

    pub fn status() -> Result<VoiceStatus, String> {
        let mut credentials = load_credentials()?;
        refresh_if_needed(&mut credentials)?;
        let mut pipe = authenticated_pipe(&credentials)?;
        voice_status(&mut pipe)
    }

    pub fn control(action: &str) -> Result<VoiceStatus, String> {
        let mut credentials = load_credentials()?;
        refresh_if_needed(&mut credentials)?;
        let mut pipe = authenticated_pipe(&credentials)?;
        match action {
            "mute" => {
                let current = rpc_command(&mut pipe, "GET_VOICE_SETTINGS", json!({}))?;
                let value = !current.get("mute").and_then(Value::as_bool).unwrap_or(false);
                rpc_command(&mut pipe, "SET_VOICE_SETTINGS", json!({ "mute": value }))?;
            }
            "deafen" => {
                let current = rpc_command(&mut pipe, "GET_VOICE_SETTINGS", json!({}))?;
                let value = !current.get("deaf").and_then(Value::as_bool).unwrap_or(false);
                rpc_command(&mut pipe, "SET_VOICE_SETTINGS", json!({ "deaf": value }))?;
            }
            "leave" => { rpc_command(&mut pipe, "SELECT_VOICE_CHANNEL", json!({ "channel_id": null }))?; }
            _ => return Err("Unsupported Discord RPC action".into()),
        }
        voice_status(&mut pipe)
    }

    pub fn disconnect() -> Result<(), String> {
        let mut credentials = load_credentials()?;
        if let Some(token) = credentials.access_token.as_deref() {
            let client = Client::builder().timeout(std::time::Duration::from_secs(15)).build().map_err(|e| e.to_string())?;
            let response = client.post("https://discord.com/api/v10/oauth2/token/revoke")
                .form(&[
                    ("client_id", credentials.app_id.as_str()),
                    ("client_secret", credentials.client_secret.as_str()),
                    ("token", token),
                    ("token_type_hint", "access_token"),
                ])
                .send().map_err(|e| e.to_string())?;
            if !response.status().is_success() { return Err(format!("Discord revoke failed: {}", response.status())); }
        }
        credentials.access_token = None;
        credentials.refresh_token = None;
        credentials.expires_at = 0;
        save_credentials(&credentials)
    }

    pub fn store(app_id: String, secret: String, redirect_uri: String) -> Result<(), String> {
        let app_id = app_id.trim().to_string();
        let secret = secret.trim().to_string();
        let redirect_uri = redirect_uri.trim().to_string();
        if app_id.is_empty() || secret.is_empty() || redirect_uri.is_empty() {
            return Err("Discord Application ID, Client Secret and Redirect URI are required".into());
        }
        if !app_id.chars().all(|c| c.is_ascii_digit()) { return Err("Discord Application ID must contain digits only".into()); }
        if !(redirect_uri.starts_with("https://") || redirect_uri.starts_with("http://localhost")) {
            return Err("Use the exact HTTPS or localhost redirect URI registered in Developer Portal".into());
        }
        save_credentials(&StoredCredentials {
            app_id,
            client_secret: secret,
            redirect_uri,
            access_token: None,
            refresh_token: None,
            expires_at: 0,
        })
    }

    pub fn has_credentials() -> bool { load_credentials().is_ok() }

    pub fn forget() -> Result<(), String> {
        let target = super::CREDENTIAL_TARGET.encode_utf16().chain(std::iter::once(0)).collect::<Vec<_>>();
        if unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) } == 0 {
            let code = std::io::Error::last_os_error().raw_os_error().unwrap_or_default();
            if code != 1168 { return Err(std::io::Error::from_raw_os_error(code).to_string()); }
        }
        Ok(())
    }
}

#[cfg(windows)]
use windows_rpc as platform;

#[cfg(not(windows))]
mod platform {
    use super::{ConfigSummary, VoiceStatus};
    pub fn summary() -> ConfigSummary { ConfigSummary::default() }
    pub fn save(_: String, _: String, _: String) -> Result<(), String> { Err("Discord RPC is available in the Windows app".into()) }
    pub fn connect() -> Result<VoiceStatus, String> { Err("Discord RPC is available in the Windows app".into()) }
    pub fn status() -> Result<VoiceStatus, String> { Err("Discord RPC is available in the Windows app".into()) }
    pub fn control(_: &str) -> Result<VoiceStatus, String> { Err("Discord RPC is available in the Windows app".into()) }
    pub fn disconnect() -> Result<(), String> { Err("Discord RPC is available in the Windows app".into()) }
    pub fn has_credentials() -> bool { false }
    pub fn forget() -> Result<(), String> { Ok(()) }
}

pub fn summary() -> ConfigSummary { platform::summary() }
pub fn save(app_id: String, secret: String, redirect_uri: String) -> Result<(), String> { platform::save(app_id, secret, redirect_uri) }
pub fn connect() -> Result<VoiceStatus, String> { platform::connect() }
pub fn status() -> Result<VoiceStatus, String> { platform::status() }
pub fn control(action: &str) -> Result<VoiceStatus, String> { platform::control(action) }
pub fn disconnect() -> Result<(), String> { platform::disconnect() }
pub fn has_credentials() -> bool { platform::has_credentials() }
pub fn forget() -> Result<(), String> { platform::forget() }
