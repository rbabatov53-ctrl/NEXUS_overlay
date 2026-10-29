use serde::Serialize;
use std::{
    collections::VecDeque,
    io::{BufRead, BufReader},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
#[cfg(windows)]
use std::{collections::HashMap, sync::OnceLock, time::Instant};
use sysinfo::System;
use tauri::{Manager, State};

mod discord;

#[derive(Serialize, Clone, Default)]
struct MediaSession {
    title: Option<String>,
    artist: Option<String>,
    source: Option<String>,
    playing: bool,
}

#[tauri::command]
fn get_media_session() -> MediaSession {
    #[cfg(windows)]
    {
        return media::get_current_session().unwrap_or_default();
    }
    #[cfg(not(windows))]
    {
        MediaSession::default()
    }
}

#[tauri::command]
fn control_media(action: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        return media::control(&action);
    }
    #[cfg(not(windows))]
    {
        let _ = action;
        Err("Windows media controls are available in the Windows app".into())
    }
}

#[tauri::command]
fn get_start_on_login() -> bool {
    #[cfg(windows)]
    {
        return std::process::Command::new("reg.exe")
            .args(["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "NEXUSOverlay"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
    }
    #[cfg(not(windows))]
    false
}

#[tauri::command]
fn set_start_on_login(enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
        let result = if enabled {
            let executable = std::env::current_exe().map_err(|error| error.to_string())?;
            let command = format!("\"{}\" --background", executable.display());
            std::process::Command::new("reg.exe")
                .args(["add", key, "/v", "NEXUSOverlay", "/t", "REG_SZ", "/d", &command, "/f"])
                .status()
        } else {
            std::process::Command::new("reg.exe")
                .args(["delete", key, "/v", "NEXUSOverlay", "/f"])
                .status()
        };
        return match result {
            Ok(status) if status.success() => Ok(()),
            Ok(_) => Err("Windows could not update the per-user startup entry".into()),
            Err(error) => Err(error.to_string()),
        };
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        Err("Background game watcher is available in the Windows app".into())
    }
}

#[tauri::command]
fn adjust_system_volume(action: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        return media::adjust_volume(&action);
    }
    #[cfg(not(windows))]
    {
        let _ = action;
        Err("System volume control is available in the Windows app".into())
    }
}

#[tauri::command]
fn get_discord_config() -> discord::ConfigSummary { discord::summary() }

#[tauri::command]
fn save_discord_credentials(app_id: String, client_secret: String, redirect_uri: String) -> Result<(), String> {
    discord::save(app_id, client_secret, redirect_uri)
}

#[tauri::command]
fn connect_discord() -> Result<discord::VoiceStatus, String> { discord::connect() }

#[tauri::command]
fn get_discord_voice_status() -> Result<discord::VoiceStatus, String> { discord::status() }

#[tauri::command]
fn control_discord(action: String) -> Result<discord::VoiceStatus, String> { discord::control(&action) }

#[tauri::command]
fn disconnect_discord() -> Result<(), String> { discord::disconnect() }

#[tauri::command]
fn forget_discord_credentials() -> Result<(), String> { discord::forget() }

#[cfg(windows)]
mod media {
    use super::MediaSession;
    use windows::{
        Media::Control::{
            GlobalSystemMediaTransportControlsSession,
            GlobalSystemMediaTransportControlsSessionManager,
            GlobalSystemMediaTransportControlsSessionPlaybackStatus,
        },
    };

    #[link(name = "user32")]
    extern "system" {
        fn keybd_event(vk: u8, scan: u8, flags: u32, extra_info: usize);
    }

    fn sessions() -> Result<Vec<GlobalSystemMediaTransportControlsSession>, String> {
        windows::core::initialize_mta().map_err(|e| e.to_string())?;
        let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .map_err(|e| e.to_string())?
            .get()
            .map_err(|e| e.to_string())?;
        let list = manager.GetSessions().map_err(|e| e.to_string())?;
        let count = list.Size().map_err(|e| e.to_string())?;
        let mut result = Vec::with_capacity(count as usize);
        for index in 0..count {
            if let Ok(session) = list.GetAt(index) {
                result.push(session);
            }
        }
        Ok(result)
    }

    fn select_session() -> Result<Option<GlobalSystemMediaTransportControlsSession>, String> {
        let sessions = sessions()?;
        for session in &sessions {
            if let Ok(info) = session.GetPlaybackInfo() {
                if info.PlaybackStatus().ok()
                    == Some(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing)
                {
                    return Ok(Some(session.clone()));
                }
            }
        }
        Ok(sessions.into_iter().next())
    }

    pub fn get_current_session() -> Result<MediaSession, String> {
        let Some(session) = select_session()? else {
            return Ok(MediaSession::default());
        };
        let props = session.TryGetMediaPropertiesAsync()
            .map_err(|e| e.to_string())?
            .get()
            .map_err(|e| e.to_string())?;
        let title = props.Title().ok().map(|value| value.to_string_lossy()).filter(|s| !s.is_empty());
        let artist = props.Artist().ok().map(|value| value.to_string_lossy()).filter(|s| !s.is_empty());
        let source = session.SourceAppUserModelId().ok().map(|value| value.to_string_lossy());
        let playing = session.GetPlaybackInfo().ok()
            .and_then(|info| info.PlaybackStatus().ok())
            == Some(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing);
        Ok(MediaSession { title, artist, source, playing })
    }

    pub fn control(action: &str) -> Result<(), String> {
        let session = select_session()?.ok_or_else(|| "No active Windows media session".to_string())?;
        let operation = match action {
            "play_pause" => session.TryTogglePlayPause(),
            "next" => session.TrySkipNext(),
            "previous" => session.TrySkipPrevious(),
            _ => return Err("Unsupported media action".into()),
        };
        let accepted = operation.map_err(|e| e.to_string())?.get().map_err(|e| e.to_string())?;
        if accepted { Ok(()) } else { Err("The active player declined this action".into()) }
    }

    pub fn adjust_volume(action: &str) -> Result<(), String> {
        let key = match action {
            "up" => 0xAF,
            "down" => 0xAE,
            "mute" => 0xAD,
            _ => return Err("Unsupported volume action".into()),
        };
        unsafe {
            keybd_event(key, 0, 0, 0);
            keybd_event(key, 0, 0x0002, 0);
        }
        Ok(())
    }
}

#[cfg(windows)]
#[repr(C)]
struct HotkeyPoint { x: i32, y: i32 }

#[cfg(windows)]
#[repr(C)]
struct HotkeyMessage {
    hwnd: *mut std::ffi::c_void,
    message: u32,
    w_param: usize,
    l_param: isize,
    time: u32,
    point: HotkeyPoint,
    l_private: u32,
}

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn RegisterHotKey(hwnd: *mut std::ffi::c_void, id: i32, modifiers: u32, key: u32) -> i32;
    fn UnregisterHotKey(hwnd: *mut std::ffi::c_void, id: i32) -> i32;
    fn GetMessageW(message: *mut HotkeyMessage, hwnd: *mut std::ffi::c_void, min: u32, max: u32) -> i32;
}

#[cfg(windows)]
fn register_insert_hotkey(app: tauri::AppHandle) {
    use std::{mem::zeroed, ptr};

    thread::spawn(move || unsafe {
        const HOTKEY_ID: i32 = 0x4E58;
        const WM_HOTKEY: u32 = 0x0312;
        const MOD_NOREPEAT: u32 = 0x4000;
        const VK_INSERT: u32 = 0x2D;
        if RegisterHotKey(ptr::null_mut(), HOTKEY_ID, MOD_NOREPEAT, VK_INSERT) == 0 {
            eprintln!("Не удалось зарегистрировать глобальную клавишу Insert");
            return;
        }
        loop {
            let mut message: HotkeyMessage = zeroed();
            if GetMessageW(&mut message, ptr::null_mut(), 0, 0) <= 0 { break; }
            if message.message == WM_HOTKEY && message.w_param == HOTKEY_ID as usize {
                if let Some(window) = app.get_webview_window("main") {
                    match window.is_visible() {
                        Ok(true) => { let _ = window.hide(); }
                        _ => { let _ = window.show(); let _ = window.set_focus(); }
                    }
                }
            }
        }
        let _ = UnregisterHotKey(ptr::null_mut(), HOTKEY_ID);
    });
}

const NETWORK_WINDOW_SIZE: usize = 20;
const MIN_SAMPLES_FOR_FAILURE_LOSS: usize = 2;
const ICMP_TIMEOUT_MS: u32 = 1200;
const TCP_FALLBACK_TIMEOUT_MS: u64 = 800;

#[derive(Serialize, Clone, Default)]
struct GameMetrics {
    fps: Option<u32>,
    frame_time_ms: Option<f64>,
    game_name: Option<String>,
}

#[derive(Serialize)]
struct PcMetrics {
    cpu_usage: f32,
    ram_used_mb: u64,
    ram_total_mb: u64,
    gpu_name: String,
    gpu_usage: Option<u32>,
    gpu_memory_used_mb: Option<u64>,
    gpu_memory_total_mb: Option<u64>,
    gpu_temperature: Option<u32>,
}

#[derive(Serialize, Clone)]
struct NetworkMetrics {
    ping_ms: Option<u32>,
    packet_loss: Option<f64>,
    endpoint: Option<String>,
    status: String,
    samples: usize,
}

impl Default for NetworkMetrics {
    fn default() -> Self {
        Self {
            ping_ms: None,
            packet_loss: None,
            endpoint: None,
            status: "collecting".to_string(),
            samples: 0,
        }
    }
}

struct GameState {
    metrics: Arc<Mutex<GameMetrics>>,
}

struct NetworkState {
    metrics: Arc<Mutex<NetworkMetrics>>,
}

struct PcState {
    system: Mutex<System>,
}

#[tauri::command]
fn get_game_metrics(state: State<'_, GameState>) -> GameMetrics {
    state
        .metrics
        .lock()
        .map(|metrics| metrics.clone())
        .unwrap_or_default()
}

#[tauri::command]
fn get_network_metrics(state: State<'_, NetworkState>) -> NetworkMetrics {
    state
        .metrics
        .lock()
        .map(|metrics| metrics.clone())
        .unwrap_or_default()
}

#[tauri::command]
fn get_pc_metrics(state: State<'_, PcState>) -> PcMetrics {
    let mut system = state.system.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    system.refresh_cpu_usage();
    system.refresh_memory();

    let ram_total = system.total_memory() / 1024 / 1024;
    let ram_available = system.available_memory() / 1024 / 1024;

    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,utilization.gpu,memory.used,memory.total,temperature.gpu",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok();

    let mut metrics = PcMetrics {
        cpu_usage: system.global_cpu_usage(),
        ram_used_mb: ram_total.saturating_sub(ram_available),
        ram_total_mb: ram_total,
        gpu_name: "NVIDIA GPU".to_string(),
        gpu_usage: None,
        gpu_memory_used_mb: None,
        gpu_memory_total_mb: None,
        gpu_temperature: None,
    };

    if let Some(output) = output.filter(|output| output.status.success()) {
        let line = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = line.trim().split(',').map(str::trim).collect();

        if parts.len() >= 5 {
            metrics.gpu_name = parts[0].to_string();
            metrics.gpu_usage = parts[1].parse().ok();
            metrics.gpu_memory_used_mb = parts[2].parse().ok();
            metrics.gpu_memory_total_mb = parts[3].parse().ok();
            metrics.gpu_temperature = parts[4].parse().ok();
        }
    }

    metrics
}

fn find_presentmon() -> Option<PathBuf> {
    let filename = "PresentMon-2.5.1-x64.exe";

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let candidate = parent.join(filename);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    let candidate = std::env::current_dir()
        .ok()?
        .join("src-tauri")
        .join(filename);

    if candidate.exists() {
        return Some(candidate);
    }

    let candidate = std::env::current_dir().ok()?.join(filename);

    if candidate.exists() {
        return Some(candidate);
    }

    None
}

#[cfg(windows)]
fn has_game_sized_window_uncached(process_id: u32) -> bool {
    use std::ffi::c_void;

    #[repr(C)]
    struct Rect { left: i32, top: i32, right: i32, bottom: i32 }
    struct Search { pid: u32, width: i32, height: i32, found: bool }

    #[link(name = "user32")]
    extern "system" {
        fn EnumWindows(callback: Option<unsafe extern "system" fn(*mut c_void, isize) -> i32>, data: isize) -> i32;
        fn IsWindowVisible(window: *mut c_void) -> i32;
        fn GetWindowThreadProcessId(window: *mut c_void, process_id: *mut u32) -> u32;
        fn GetWindowRect(window: *mut c_void, rect: *mut Rect) -> i32;
        fn GetSystemMetrics(index: i32) -> i32;
    }

    unsafe extern "system" fn visit(window: *mut c_void, data: isize) -> i32 {
        let search = &mut *(data as *mut Search);
        if IsWindowVisible(window) == 0 { return 1; }
        let mut pid = 0;
        GetWindowThreadProcessId(window, &mut pid);
        if pid != search.pid { return 1; }
        let mut rect = Rect { left: 0, top: 0, right: 0, bottom: 0 };
        if GetWindowRect(window, &mut rect) == 0 { return 1; }
        let width = rect.right.saturating_sub(rect.left);
        let height = rect.bottom.saturating_sub(rect.top);
        if width >= search.width * 4 / 5 && height >= search.height * 3 / 4 {
            search.found = true;
            return 0;
        }
        1
    }

    let width = unsafe { GetSystemMetrics(0) };
    let height = unsafe { GetSystemMetrics(1) };
    if process_id == 0 || width <= 0 || height <= 0 { return false; }
    let mut search = Search { pid: process_id, width, height, found: false };
    unsafe { EnumWindows(Some(visit), (&mut search as *mut Search) as isize); }
    search.found
}

#[cfg(windows)]
fn has_game_sized_window(process_id: u32) -> bool {
    static CACHE: OnceLock<Mutex<HashMap<u32, (Instant, bool)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(entries) = cache.lock() {
        if let Some((checked, found)) = entries.get(&process_id) {
            if checked.elapsed() < Duration::from_secs(2) { return *found; }
        }
    }
    let found = has_game_sized_window_uncached(process_id);
    if let Ok(mut entries) = cache.lock() {
        if entries.len() > 256 {
            entries.retain(|_, (checked, _)| checked.elapsed() < Duration::from_secs(60));
        }
        entries.insert(process_id, (Instant::now(), found));
    }
    found
}

#[cfg(not(windows))]
fn has_game_sized_window(_process_id: u32) -> bool { false }

fn is_game_candidate(application: &str, process_id: u32) -> bool {
    let name = application
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(application)
        .to_ascii_lowercase();

    let ignored = [
        "dwm.exe",
        "explorer.exe",
        "system",
        "idle",
        "registry",
        "searchhost.exe",
        "searchindexer.exe",
        "startmenuexperiencehost.exe",
        "shellexperiencehost.exe",
        "textinputhost.exe",
        "applicationframehost.exe",
        "runtimebroker.exe",
        "taskmgr.exe",
        "conhost.exe",
        "powershell.exe",
        "pwsh.exe",
        "cmd.exe",
        "windowsterminal.exe",
        "nexus-overlay.exe",
        "nexus.exe",
        "incy.exe",
        "obs64.exe",
        "discord.exe",
        "steam.exe",
        "steamwebhelper.exe",
        "epicgameslauncher.exe",
        "epicwebhelper.exe",
        "battle.net.exe",
        "riotclientservices.exe",
        "riotclientux.exe",
        "chrome.exe",
        "msedge.exe",
        "firefox.exe",
        "opera.exe",
        "operagx.exe",
        "brave.exe",
        "spotify.exe",
        "telegram.exe",
        "vlc.exe",
    ];

    if ignored.contains(&name.as_str()) {
        return false;
    }

    let known_games = [
        "cs2.exe",
        "csgo.exe",
        "dota2.exe",
        "valorant-win64-shipping.exe",
        "valorant.exe",
        "r5apex.exe",
        "fortniteclient-win64-shipping.exe",
        "overwatch.exe",
        "overwatch2.exe",
        "minecraft.exe",
        "javaw.exe",
        "java.exe",
        "robloxplayerbeta.exe",
        "gta5.exe",
        "gta5_enhanced.exe",
        "rdr2.exe",
        "eldenring.exe",
        "warface.exe",
        "thedivision2.exe",
        "witcher3.exe",
        "cyberpunk2077.exe",
    ];

    known_games.contains(&name.as_str())
        || name.contains("shipping.exe")
        || name.contains("client-win")
        || has_game_sized_window(process_id)
}

fn start_presentmon(metrics: Arc<Mutex<GameMetrics>>, executable: PathBuf) {
    thread::spawn(move || loop {
        let child_result = Command::new(&executable)
            .args([
                "--output_stdout",
                "--v2_metrics",
                "--no_console_stats",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();

        let mut child = match child_result {
            Ok(child) => child,
            Err(error) => {
                eprintln!("Не удалось запустить PresentMon: {error}");
                thread::sleep(Duration::from_secs(3));
                continue;
            }
        };

        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);

            for line_result in reader.lines() {
                let line = match line_result {
                    Ok(line) => line,
                    Err(error) => {
                        eprintln!("Ошибка чтения PresentMon: {error}");
                        break;
                    }
                };

                let fields: Vec<&str> = line.split(',').collect();

                if fields.len() <= 9
                    || fields[0].eq_ignore_ascii_case("Application")
                {
                    continue;
                }

                let application = fields[0].trim();

                let process_id = fields.get(1).and_then(|value| value.trim().parse::<u32>().ok()).unwrap_or(0);
                if !is_game_candidate(application, process_id) {
                    continue;
                }

                let frame_time = match fields[9].trim().parse::<f64>() {
                    Ok(value) if value.is_finite() && value > 0.0 => value,
                    _ => continue,
                };

                let fps = (1000.0 / frame_time).round() as u32;

                let game_name = application
                    .rsplit(['\\', '/'])
                    .next()
                    .unwrap_or(application)
                    .trim()
                    .to_string();

                if let Ok(mut current) = metrics.lock() {
                    *current = GameMetrics {
                        fps: Some(fps),
                        frame_time_ms: Some(frame_time),
                        game_name: Some(game_name),
                    };
                }
            }
        }

        let _ = child.kill();
        let _ = child.wait();

        if let Ok(mut current) = metrics.lock() {
            *current = GameMetrics::default();
        }

        thread::sleep(Duration::from_secs(2));
    });
}

#[cfg(windows)]
mod win_icmp {
    use std::{ffi::c_void, mem::size_of, net::Ipv4Addr, ptr};

    type Handle = *mut c_void;

    const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
    const IP_SUCCESS: u32 = 0;

    #[repr(C)]
    #[derive(Copy, Clone)]
    #[allow(dead_code)]
    struct IpOptionInformation {
        ttl: u8,
        tos: u8,
        flags: u8,
        options_size: u8,
        options_data: *mut u8,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    #[allow(dead_code)]
    struct IcmpEchoReply {
        address: u32,
        status: u32,
        round_trip_time: u32,
        data_size: u16,
        reserved: u16,
        data: *mut c_void,
        options: IpOptionInformation,
    }

    #[repr(C, align(8))]
    struct AlignedReplyBuffer([u8; 256]);

    #[link(name = "iphlpapi")]
    extern "system" {
        fn IcmpCreateFile() -> Handle;
        fn IcmpCloseHandle(icmp_handle: Handle) -> i32;
        fn IcmpSendEcho(
            icmp_handle: Handle,
            destination_address: u32,
            request_data: *const c_void,
            request_size: u16,
            request_options: *const IpOptionInformation,
            reply_buffer: *mut c_void,
            reply_size: u32,
            timeout: u32,
        ) -> u32;
    }

    pub enum IcmpOutcome {
        Success { rtt_ms: u32, ttl: u8 },
        TimeoutOrUnreachable,
        ApiError(u32),
    }

    pub struct IcmpClient {
        handle: Handle,
    }

    impl IcmpClient {
        pub fn new() -> Result<Self, u32> {
            let handle = unsafe { IcmpCreateFile() };
            if handle.is_null() || handle == INVALID_HANDLE_VALUE {
                let err = std::io::Error::last_os_error()
                    .raw_os_error()
                    .unwrap_or(0) as u32;
                Err(err)
            } else {
                Ok(Self { handle })
            }
        }

        pub fn ping(&self, target: Ipv4Addr, timeout_ms: u32) -> IcmpOutcome {
            const PAYLOAD: &[u8; 32] = b"NEXUS_OVERLAY_ICMP_PROBE_0000001";
            let mut buffer = AlignedReplyBuffer([0u8; 256]);

            debug_assert!(
                buffer.0.len() >= size_of::<IcmpEchoReply>() + PAYLOAD.len() + 8
            );

            let destination = u32::from_ne_bytes(target.octets());

            let reply_count = unsafe {
                IcmpSendEcho(
                    self.handle,
                    destination,
                    PAYLOAD.as_ptr().cast::<c_void>(),
                    PAYLOAD.len() as u16,
                    ptr::null(),
                    buffer.0.as_mut_ptr().cast::<c_void>(),
                    buffer.0.len() as u32,
                    timeout_ms,
                )
            };

            if reply_count == 0 {
                let err = std::io::Error::last_os_error()
                    .raw_os_error()
                    .unwrap_or(0) as u32;

                if matches!(err, 5 | 6 | 8 | 50 | 87 | 122) {
                    return IcmpOutcome::ApiError(err);
                }

                return IcmpOutcome::TimeoutOrUnreachable;
            }

            let reply =
                unsafe { ptr::read(buffer.0.as_ptr().cast::<IcmpEchoReply>()) };
            let reply_ip = Ipv4Addr::from(reply.address.to_ne_bytes());

            if reply.status == IP_SUCCESS && reply_ip == target {
                IcmpOutcome::Success {
                    rtt_ms: reply.round_trip_time,
                    ttl: reply.options.ttl,
                }
            } else {
                IcmpOutcome::TimeoutOrUnreachable
            }
        }
    }

    impl Drop for IcmpClient {
        fn drop(&mut self) {
            if !self.handle.is_null() && self.handle != INVALID_HANDLE_VALUE {
                unsafe {
                    let _ = IcmpCloseHandle(self.handle);
                }
            }
        }
    }
}

enum ProbeCycle {
    Success {
        endpoint: &'static str,
        rtt_ms: u32,
    },
    Unreachable,
    SensorError(String),
}

fn check_tcp_reachability() -> bool {
    let targets = [
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(8, 8, 8, 8), 53)),
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(1, 0, 0, 1), 443)),
    ];
    let timeout = Duration::from_millis(TCP_FALLBACK_TIMEOUT_MS);

    targets
        .iter()
        .any(|addr| TcpStream::connect_timeout(addr, timeout).is_ok())
}

#[cfg(windows)]
fn is_likely_local_intercept(rtt_ms: u32, ttl: u8) -> bool {
    // На ряде домашних роутеров и локальных VPN-адаптеров адрес 1.1.1.1
    // перехватывается локально (0 мс и нетронутый начальный TTL 64 / 128 / 255).
    rtt_ms == 0 && matches!(ttl, 64 | 128 | 255)
}

#[cfg(windows)]
fn probe_public_nodes(client: &mut Option<win_icmp::IcmpClient>) -> ProbeCycle {
    const PUBLIC_NODES: [(&str, Ipv4Addr); 2] = [
        ("1.1.1.1", Ipv4Addr::new(1, 1, 1, 1)),
        ("8.8.8.8", Ipv4Addr::new(8, 8, 8, 8)),
    ];

    if client.is_none() {
        match win_icmp::IcmpClient::new() {
            Ok(created) => *client = Some(created),
            Err(code) => {
                return ProbeCycle::SensorError(format!(
                    "IcmpCreateFile вернул код Win32 {code}"
                ));
            }
        }
    }

    let Some(icmp) = client.as_ref() else {
        return ProbeCycle::SensorError(
            "ICMP-клиент не инициализирован".to_string(),
        );
    };

    let mut local_intercept_candidate: Option<(&'static str, u32)> = None;
    let mut last_api_error: Option<u32> = None;

    for (label, ip) in PUBLIC_NODES {
        match icmp.ping(ip, ICMP_TIMEOUT_MS) {
            win_icmp::IcmpOutcome::Success { rtt_ms, ttl } => {
                if is_likely_local_intercept(rtt_ms, ttl) {
                    if local_intercept_candidate.is_none() {
                        local_intercept_candidate = Some((label, rtt_ms));
                    }
                    continue;
                }

                return ProbeCycle::Success {
                    endpoint: label,
                    rtt_ms,
                };
            }
            win_icmp::IcmpOutcome::TimeoutOrUnreachable => {}
            win_icmp::IcmpOutcome::ApiError(code) => {
                last_api_error = Some(code);
            }
        }
    }

    if let Some((label, rtt_ms)) = local_intercept_candidate {
        if check_tcp_reachability() {
            return ProbeCycle::Success {
                endpoint: label,
                rtt_ms,
            };
        }
        return ProbeCycle::Unreachable;
    }

    if let Some(code) = last_api_error {
        *client = None;
        return ProbeCycle::SensorError(format!(
            "IcmpSendEcho вернул локальную ошибку Win32 {code}"
        ));
    }

    ProbeCycle::Unreachable
}

#[cfg(not(windows))]
fn probe_public_nodes(_client: &mut Option<()>) -> ProbeCycle {
    let _ = ICMP_TIMEOUT_MS;
    if check_tcp_reachability() {
        ProbeCycle::SensorError(
            "Нативный Win32 ICMP доступен только на Windows".to_string(),
        )
    } else {
        ProbeCycle::Unreachable
    }
}

fn start_network_monitor(metrics: Arc<Mutex<NetworkMetrics>>) {
    thread::spawn(move || {
        #[cfg(windows)]
        let mut icmp_client: Option<win_icmp::IcmpClient> = None;
        #[cfg(not(windows))]
        let mut icmp_client: Option<()> = None;

        let mut history: VecDeque<bool> =
            VecDeque::with_capacity(NETWORK_WINDOW_SIZE);
        let mut consecutive_failures: u32 = 0;
        let mut last_endpoint: Option<String> = None;

        loop {
            match probe_public_nodes(&mut icmp_client) {
                ProbeCycle::Success { endpoint, rtt_ms } => {
                    consecutive_failures = 0;
                    last_endpoint = Some(endpoint.to_string());

                    if history.len() == NETWORK_WINDOW_SIZE {
                        history.pop_front();
                    }
                    history.push_back(true);

                    let lost = history.iter().filter(|&&ok| !ok).count();
                    let packet_loss =
                        (lost as f64 / history.len() as f64) * 100.0;
                    let status = if lost == 0 { "online" } else { "degraded" };

                    if let Ok(mut current) = metrics.lock() {
                        *current = NetworkMetrics {
                            ping_ms: Some(rtt_ms),
                            packet_loss: Some(packet_loss),
                            endpoint: last_endpoint.clone(),
                            status: status.to_string(),
                            samples: history.len(),
                        };
                    }
                }
                ProbeCycle::Unreachable => {
                    consecutive_failures =
                        consecutive_failures.saturating_add(1);
                    let had_icmp_success = history.iter().any(|&ok| ok);

                    // Если ICMP ни разу не проходил, но TCP до публичных узлов открывается —
                    // провайдер или фаервол фильтрует ICMP, при этом интернет работает.
                    if !had_icmp_success && check_tcp_reachability() {
                        if let Ok(mut current) = metrics.lock() {
                            *current = NetworkMetrics {
                                ping_ms: None,
                                packet_loss: None,
                                endpoint: Some(
                                    "1.1.1.1 / 8.8.8.8".to_string(),
                                ),
                                status: "icmp_blocked".to_string(),
                                samples: 0,
                            };
                        }
                    } else {
                        if history.len() == NETWORK_WINDOW_SIZE {
                            history.pop_front();
                        }
                        history.push_back(false);

                        let samples = history.len();
                        let lost = history.iter().filter(|&&ok| !ok).count();
                        let loss_pct = (lost as f64 / samples as f64) * 100.0;

                        // На первой неудачной пробе при холодном старте не пугаем ложными 100%,
                        // а дожидаемся подтверждения второй завершённой пробой.
                        if !had_icmp_success
                            && samples < MIN_SAMPLES_FOR_FAILURE_LOSS
                        {
                            if let Ok(mut current) = metrics.lock() {
                                *current = NetworkMetrics {
                                    ping_ms: None,
                                    packet_loss: None,
                                    endpoint: last_endpoint.clone(),
                                    status: "collecting".to_string(),
                                    samples,
                                };
                            }
                        } else {
                            let status = if consecutive_failures >= 3
                                || lost == samples
                            {
                                "offline"
                            } else {
                                "degraded"
                            };

                            if let Ok(mut current) = metrics.lock() {
                                *current = NetworkMetrics {
                                    ping_ms: None,
                                    packet_loss: Some(loss_pct),
                                    endpoint: last_endpoint.clone().or_else(
                                        || {
                                            Some(
                                                "1.1.1.1 / 8.8.8.8".to_string(),
                                            )
                                        },
                                    ),
                                    status: status.to_string(),
                                    samples,
                                };
                            }
                        }
                    }
                }
                ProbeCycle::SensorError(reason) => {
                    // Ошибку датчика/ОС не засчитываем как потерю сетевых пакетов.
                    eprintln!("Ошибка сетевого монитора NEXUS: {reason}");

                    if history.is_empty() {
                        if let Ok(mut current) = metrics.lock() {
                            *current = NetworkMetrics {
                                ping_ms: None,
                                packet_loss: None,
                                endpoint: None,
                                status: "error".to_string(),
                                samples: 0,
                            };
                        }
                    }
                }
            }

            thread::sleep(Duration::from_secs(1));
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let pc_state = PcState {
        system: Mutex::new(System::new()),
    };

    let game_state = GameState {
        metrics: Arc::new(Mutex::new(GameMetrics::default())),
    };

    let game_metrics = Arc::clone(&game_state.metrics);

    let network_state = NetworkState {
        metrics: Arc::new(Mutex::new(NetworkMetrics::default())),
    };

    let network_metrics = Arc::clone(&network_state.metrics);

    tauri::Builder::default()
        .manage(pc_state)
        .manage(game_state)
        .manage(network_state)
        .setup(move |app| {
            let executable = find_presentmon().or_else(|| {
                app.path()
                    .resource_dir()
                    .ok()
                    .map(|dir| dir.join("PresentMon-2.5.1-x64.exe"))
                    .filter(|path| path.exists())
            });

            if let Some(executable) = executable {
                start_presentmon(game_metrics, executable);
            } else {
                eprintln!("PresentMon-2.5.1-x64.exe не найден");
            }

            start_network_monitor(network_metrics);

            #[cfg(windows)]
            register_insert_hotkey(app.handle().clone());

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_always_on_top(true);
                if std::env::args().any(|arg| arg == "--background") {
                    let _ = window.hide();
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_pc_metrics,
            get_game_metrics,
            get_network_metrics,
            get_media_session,
            control_media,
            adjust_system_volume,
            get_start_on_login,
            set_start_on_login,
            get_discord_config,
            save_discord_credentials,
            connect_discord,
            get_discord_voice_status,
            control_discord,
            disconnect_discord,
            forget_discord_credentials
        ])
        .run(tauri::generate_context!())
        .expect("error while running NEXUS");
}
