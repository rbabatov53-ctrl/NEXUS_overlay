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
use sysinfo::System;
use tauri::{Manager, State};

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
fn get_pc_metrics() -> PcMetrics {
    let mut system = System::new_all();
    system.refresh_cpu_usage();

    thread::sleep(Duration::from_millis(250));
    system.refresh_cpu_usage();

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
    let filename = "PresentMon-2.6.0-x64.exe";

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

fn is_game_candidate(application: &str) -> bool {
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

                if !is_game_candidate(application) {
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
    let game_state = GameState {
        metrics: Arc::new(Mutex::new(GameMetrics::default())),
    };

    let game_metrics = Arc::clone(&game_state.metrics);

    let network_state = NetworkState {
        metrics: Arc::new(Mutex::new(NetworkMetrics::default())),
    };

    let network_metrics = Arc::clone(&network_state.metrics);

    tauri::Builder::default()
        .manage(game_state)
        .manage(network_state)
        .setup(move |app| {
            let executable = find_presentmon().or_else(|| {
                app.path()
                    .resource_dir()
                    .ok()
                    .map(|dir| dir.join("PresentMon-2.6.0-x64.exe"))
                    .filter(|path| path.exists())
            });

            if let Some(executable) = executable {
                start_presentmon(game_metrics, executable);
            } else {
                eprintln!("PresentMon-2.6.0-x64.exe не найден");
            }

            start_network_monitor(network_metrics);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_pc_metrics,
            get_game_metrics,
            get_network_metrics
        ])
        .run(tauri::generate_context!())
        .expect("error while running NEXUS");
}
