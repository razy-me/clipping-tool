use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Mutex;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use tauri::{AppHandle, Manager, State};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProcessMetric {
    pub pid: u32,
    pub name: String,
    pub role: String,
    pub cpu_usage: f32,            // % of a single core (e.g. 5.2%)
    pub cpu_usage_normalized: f32, // % of total CPU capacity across all cores
    pub memory_bytes: u64,         // Physical RAM (Working Set)
    pub memory_mb: f64,
    pub virtual_memory_bytes: u64, // Virtual Commit Charge
    pub virtual_memory_mb: f64,
    pub disk_read_bytes: u64,
    pub disk_written_bytes: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SystemSpecs {
    pub os_name: String,
    pub cpu_name: String,
    pub cpu_cores: usize,
    pub total_ram_mb: f64,
    pub available_ram_mb: f64,
    pub power_source: String, // "Netzbetrieb (AC)" / "Akkubetrieb (Battery)"
    pub hardware_tier: String,
    pub gpu_name: String,
    pub display_resolution: String,
    pub display_refresh_rate: u32,
    pub capture_pixel_rate_mps: f64,
    pub raw_video_bandwidth_mbs: f64,
    pub disk_free_gb: f64,
    pub disk_total_gb: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ActiveSettingsSnapshot {
    pub video_codec: String,
    pub video_resolution: String,
    pub fps: String,
    pub bitrate_preset: String,
    pub buffer_length_secs: u32,
    pub monitor_idx: u32,
    pub show_cursor: bool,
    pub hdr_tonemapping: bool,
    pub spike_detection: bool,
    pub spike_threshold: f32,
    pub controller_clipping: bool,
    pub audio_sync_offset_ms: i32,
    pub isolated_apps_count: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PipelineDiagnostics {
    pub active_encoder: String,
    pub hw_acceleration: String,
    pub target_resolution: String,
    pub pixel_reduction_pct: f64,
    pub video_buffer_stored_secs: f64,
    pub video_buffer_chunks: usize,
    pub audio_tracks_count: usize,
    pub scaling_method: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolPerformanceSnapshot {
    pub timestamp_ms: u64,
    pub cpu_cores: usize,
    pub total_cpu_usage: f32,            // Core-equivalent %
    pub total_cpu_normalized: f32,       // 0..100% of whole PC CPU
    pub total_memory_bytes: u64,         // Total Physical RAM of ClipTool & sub-processes
    pub total_memory_mb: f64,
    pub total_virtual_memory_bytes: u64, // Total Commit of ClipTool & sub-processes
    pub total_virtual_memory_mb: f64,
    pub video_buffer_bytes: usize,       // In-Memory RAM Video Ring-Buffer
    pub video_buffer_mb: f64,
    pub video_buffer_max_bytes: usize,
    pub video_buffer_max_mb: f64,
    pub audio_buffer_bytes: usize,       // In-Memory Audio Ring-Buffer
    pub audio_buffer_mb: f64,
    pub system_specs: SystemSpecs,
    pub active_settings: ActiveSettingsSnapshot,
    pub pipeline_diagnostics: PipelineDiagnostics,
    pub bottleneck_warnings: Vec<String>,
    pub processes: Vec<ProcessMetric>,
}

pub struct PerformanceMonitorState {
    pub system: Mutex<System>,
}

impl PerformanceMonitorState {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::everything());
        Self {
            system: Mutex::new(sys),
        }
    }
}

impl Default for PerformanceMonitorState {
    fn default() -> Self {
        Self::new()
    }
}

fn detect_gpu_name() -> String {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::Graphics::Gdi::{EnumDisplayDevicesW, DISPLAY_DEVICEW};
        let mut dev = DISPLAY_DEVICEW::default();
        dev.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
        if EnumDisplayDevicesW(None, 0, &mut dev, 0).as_bool() {
            let dev_str = String::from_utf16_lossy(&dev.DeviceString);
            let cleaned = dev_str.trim_matches('\0').trim();
            if !cleaned.is_empty() {
                return cleaned.to_string();
            }
        }
    }
    "Grafikkarte (WDDM)".to_string()
}

fn determine_role(pid: u32, root_pid: u32, name: &str, cmd: &[std::ffi::OsString]) -> String {
    let name_lower = name.to_lowercase();
    if pid == root_pid {
        return "Hauptanwendung (Rust & Tauri Core)".to_string();
    }
    if name_lower.contains("ffmpeg") {
        return "Video-Aufnahme (FFmpeg Encoder)".to_string();
    }
    if name_lower.contains("msedgewebview2") {
        let cmd_str = cmd.iter().map(|c| c.to_string_lossy()).collect::<Vec<_>>().join(" ");
        if cmd_str.contains("--type=gpu-process") {
            return "WebView2 (DirectX GPU Compositor)".to_string();
        }
        if cmd_str.contains("--type=renderer") {
            return "WebView2 (UI Rendering Engine)".to_string();
        }
        if cmd_str.contains("--type=utility") {
            return "WebView2 (Utility / Audio / IPC)".to_string();
        }
        if cmd_str.contains("--type=crashpad-handler") {
            return "WebView2 (Crash Reporter)".to_string();
        }
        return "WebView2 (Browser Host)".to_string();
    }
    "Hintergrund-Subprozess".to_string()
}

#[tauri::command]
pub async fn get_tool_performance(
    app: AppHandle,
    state: State<'_, PerformanceMonitorState>,
) -> Result<ToolPerformanceSnapshot, String> {
    let (processes_metrics, total_cpu_usage, total_memory_bytes, total_virtual_memory_bytes, cpu_cores, cpu_name, total_ram_bytes, avail_ram_bytes, os_name, os_ver) = {
        let mut sys = state.system.lock().unwrap();
        let refresh_kind = ProcessRefreshKind::nothing()
            .with_cpu()
            .with_memory()
            .with_disk_usage();
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh_kind);
        sys.refresh_memory();

        let root_pid_u32 = std::process::id();
        let root_pid = Pid::from_u32(root_pid_u32);

        let mut tool_pids: HashSet<Pid> = HashSet::new();
        tool_pids.insert(root_pid);

        // Also include active FFmpeg child PID from recorder state if known
        if let Some(recorder_state) = app.try_state::<crate::recorder::RecorderState>() {
            if let Ok(guard) = recorder_state.child.lock() {
                if let Some(child_arc) = guard.as_ref() {
                    if let Ok(opt) = child_arc.lock() {
                        if let Some(child) = opt.as_ref() {
                            tool_pids.insert(Pid::from_u32(child.pid()));
                        }
                    }
                }
            }
        }

        // Fixed-point iteration to discover all descendants (children, grandchildren, etc.)
        let mut added_new = true;
        while added_new {
            added_new = false;
            for (pid, process) in sys.processes() {
                if !tool_pids.contains(pid) {
                    if let Some(parent) = process.parent() {
                        if tool_pids.contains(&parent) {
                            tool_pids.insert(*pid);
                            added_new = true;
                        }
                    }
                }
            }
        }

        let cpu_cores = sys.cpus().len().max(1);
        let mut processes_metrics = Vec::new();
        let mut total_cpu_usage = 0.0f32;
        let mut total_memory_bytes = 0u64;
        let mut total_virtual_memory_bytes = 0u64;

        for pid in &tool_pids {
            if let Some(process) = sys.process(*pid) {
                let pid_u32 = pid.as_u32();
                let name = process.name().to_string_lossy().into_owned();
                let role = determine_role(pid_u32, root_pid_u32, &name, process.cmd());
                let cpu = process.cpu_usage();
                let cpu_norm = (cpu / cpu_cores as f32).clamp(0.0, 100.0);
                let mem = process.memory();
                let virt_mem = process.virtual_memory();
                let disk = process.disk_usage();

                total_cpu_usage += cpu;
                total_memory_bytes += mem;
                total_virtual_memory_bytes += virt_mem;

                processes_metrics.push(ProcessMetric {
                    pid: pid_u32,
                    name,
                    role,
                    cpu_usage: (cpu * 10.0).round() / 10.0,
                    cpu_usage_normalized: (cpu_norm * 10.0).round() / 10.0,
                    memory_bytes: mem,
                    memory_mb: (mem as f64 / (1024.0 * 1024.0) * 10.0).round() / 10.0,
                    virtual_memory_bytes: virt_mem,
                    virtual_memory_mb: (virt_mem as f64 / (1024.0 * 1024.0) * 10.0).round() / 10.0,
                    disk_read_bytes: disk.read_bytes,
                    disk_written_bytes: disk.written_bytes,
                });
            }
        }

        // Sort processes: root process first, then descending by CPU usage
        processes_metrics.sort_by(|a, b| {
            if a.pid == root_pid_u32 {
                std::cmp::Ordering::Less
            } else if b.pid == root_pid_u32 {
                std::cmp::Ordering::Greater
            } else {
                b.cpu_usage.partial_cmp(&a.cpu_usage).unwrap_or(std::cmp::Ordering::Equal)
            }
        });

        let cpu_name = sys.cpus().first().map(|c| c.brand().to_string()).unwrap_or_else(|| "Unbekannter Prozessor".into());
        let total_ram_bytes = sys.total_memory();
        let avail_ram_bytes = sys.available_memory();
        let os_name = System::name().unwrap_or_else(|| "Windows".into());
        let os_ver = System::os_version().unwrap_or_default();

        (processes_metrics, total_cpu_usage, total_memory_bytes, total_virtual_memory_bytes, cpu_cores, cpu_name, total_ram_bytes, avail_ram_bytes, os_name, os_ver)
    };

    let (video_bytes, video_max, video_stored_secs, video_chunks) = crate::recorder::get_video_buffer_stats();
    let audio_bytes = crate::audio_engine::get_audio_buffer_bytes();

    let total_cpu_norm = (total_cpu_usage / cpu_cores as f32).clamp(0.0, 100.0);

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    // Active Settings
    let cfg = crate::config::get_config(app.clone());

    // Monitor resolution & refresh rate
    let monitors = crate::wgc_recorder::get_available_monitors();
    let (mon_w, mon_h) = if let Some(m) = monitors.get(cfg.monitor_idx as usize).or_else(|| monitors.first()) {
        (m.width, m.height)
    } else {
        (1920, 1080)
    };
    let fps_num = cfg.fps_selection.parse::<f64>().unwrap_or(60.0);

    let capture_pixel_rate_mps = ((mon_w as f64 * mon_h as f64 * fps_num) / 1_000_000.0 * 10.0).round() / 10.0;
    let raw_video_bandwidth_mbs = ((mon_w as f64 * mon_h as f64 * 4.0 * fps_num) / (1024.0 * 1024.0) * 10.0).round() / 10.0;

    let (tgt_w, tgt_h) = match cfg.video_resolution.to_lowercase().as_str() {
        "1440p" => (2560, 1440),
        "1080p" => (1920, 1080),
        "900p" => (1600, 900),
        "720p" => (1280, 720),
        "540p" => (960, 540),
        "480p" => (854, 480),
        "360p" => (640, 360),
        "240p" => (426, 240),
        _ => (mon_w, mon_h),
    };
    let tgt_pixels = tgt_w as f64 * tgt_h as f64;
    let mon_pixels = (mon_w as f64 * mon_h as f64).max(1.0);
    let pixel_reduction_pct = if tgt_pixels < mon_pixels {
        (((1.0 - (tgt_pixels / mon_pixels)) * 100.0) * 10.0).round() / 10.0
    } else {
        0.0
    };

    // Disk space info
    let disk_info = crate::library::get_disk_space_info(app.clone());

    // Hardware Specs
    let power_source = if crate::hardware_profile::is_on_battery() {
        "Akkubetrieb (Batterie)".to_string()
    } else {
        "Netzbetrieb (AC Netzstrom)".to_string()
    };
    let hw_tier = match crate::hardware_profile::get_cached_tier() {
        crate::hardware_profile::HardwareTier::Strong => "High-End Gaming Rig (Tier: Strong)".to_string(),
        crate::hardware_profile::HardwareTier::Mid => "Mittelklasse System (Tier: Mid)".to_string(),
        crate::hardware_profile::HardwareTier::Weak => "Basis / Office PC (Tier: Weak)".to_string(),
    };

    let system_specs = SystemSpecs {
        os_name: format!("{} {}", os_name, os_ver).trim().to_string(),
        cpu_name,
        cpu_cores,
        total_ram_mb: (total_ram_bytes as f64 / (1024.0 * 1024.0) * 10.0).round() / 10.0,
        available_ram_mb: (avail_ram_bytes as f64 / (1024.0 * 1024.0) * 10.0).round() / 10.0,
        power_source,
        hardware_tier: hw_tier,
        gpu_name: detect_gpu_name(),
        display_resolution: format!("{} × {}", mon_w, mon_h),
        display_refresh_rate: 60,
        capture_pixel_rate_mps,
        raw_video_bandwidth_mbs,
        disk_free_gb: disk_info.free_gb,
        disk_total_gb: disk_info.total_gb,
    };

    let codec_str = match cfg.video_codec {
        crate::config::VideoCodec::H264 => "H.264 (Universell kompatibel)",
        crate::config::VideoCodec::HEVC => "HEVC / H.265 (High Efficiency)",
        crate::config::VideoCodec::AV1 => "AV1 (Next-Gen Ultra)",
    }.to_string();

    let bitrate_str = match cfg.bitrate_preset {
        crate::config::BitratePreset::Low => "Low (6.000 kbps)",
        crate::config::BitratePreset::Balanced => "Balanced (12.000 kbps)",
        crate::config::BitratePreset::High => "High (20.000 kbps)",
        crate::config::BitratePreset::Ultra => "Ultra (35.000 kbps)",
    }.to_string();

    let active_settings = ActiveSettingsSnapshot {
        video_codec: codec_str,
        video_resolution: cfg.video_resolution.clone(),
        fps: cfg.fps_selection.clone(),
        bitrate_preset: bitrate_str,
        buffer_length_secs: cfg.buffer_length_secs,
        monitor_idx: cfg.monitor_idx,
        show_cursor: cfg.show_cursor_in_clips,
        hdr_tonemapping: cfg.hdr_tonemapping,
        spike_detection: cfg.spike_detection_enabled,
        spike_threshold: cfg.spike_threshold,
        controller_clipping: cfg.controller_clipping,
        audio_sync_offset_ms: cfg.audio_sync_offset_ms,
        isolated_apps_count: cfg.isolated_apps.len(),
    };

    let active_encoder = crate::hardware::detect_best_encoder_for_codec(&app, &cfg.video_codec).await;
    let hw_acceleration = if active_encoder.contains("qsv") {
        "Aktiv (Intel QuickSync / Xe2 ASIC)".to_string()
    } else if active_encoder.contains("nvenc") {
        "Aktiv (NVIDIA NVENC ASIC)".to_string()
    } else if active_encoder.contains("amf") {
        "Aktiv (AMD AMF / VCN ASIC)".to_string()
    } else {
        "⚠️ KEINE (CPU Software-Encoding - Hohe Last!)".to_string()
    };

    let scaling_method_str = if active_encoder.contains("nvenc") {
        "DirectX VRAM / Cuda".to_string()
    } else if active_encoder.contains("qsv") {
        "Fast-Bilinear SIMD + QSV".to_string()
    } else {
        "Fast-Bilinear SIMD (CPU Swscale)".to_string()
    };

    let pipeline_diagnostics = PipelineDiagnostics {
        active_encoder: active_encoder.clone(),
        hw_acceleration,
        target_resolution: format!("{} × {} ({})", tgt_w, tgt_h, cfg.video_resolution),
        pixel_reduction_pct,
        video_buffer_stored_secs: (video_stored_secs * 10.0).round() / 10.0,
        video_buffer_chunks: video_chunks,
        audio_tracks_count: 2 + cfg.isolated_apps.len(),
        scaling_method: scaling_method_str,
    };

    // Bottleneck analysis
    let mut bottleneck_warnings = Vec::new();
    if raw_video_bandwidth_mbs > 800.0 && cfg.video_resolution.eq_ignore_ascii_case("original") {
        bottleneck_warnings.push(format!(
            "⚠️ Extremer Bilddatenstrom ({:.0} MB/s unkomprimiert): Ihr Monitor ({}×{}) erzeugt bei {} FPS gewaltige Datenmengen. Ein Umschalten auf 1080p spart über 60% CPU- und Speicherbus-Last.",
            raw_video_bandwidth_mbs, mon_w, mon_h, cfg.fps_selection
        ));
    }
    if crate::hardware_profile::is_on_battery() {
        bottleneck_warnings.push(
            "⚠️ Akkubetrieb aktiv: Windows drosselt CPU/GPU-Taktraten zur Akkuschonung. Für maximale Spiele-FPS und geringste Tool-Last Netzteil anschließen.".to_string()
        );
    }
    if active_encoder.starts_with("libx") || active_encoder.starts_with("libsvt") {
        bottleneck_warnings.push(format!(
            "🚨 Software-Encoder aktiv ({}): Es wird keine GPU-Hardwarebeschleunigung genutzt. Wechseln Sie auf HEVC oder AV1 für bis zu 85% weniger CPU-Last.",
            active_encoder
        ));
    }
    if disk_info.is_low_space || disk_info.free_gb < 15.0 {
        bottleneck_warnings.push(format!(
            "⚠️ Geringer freier Speicherplatz: Nur noch {:.1} GB frei auf der Zielfestplatte.",
            disk_info.free_gb
        ));
    }
    if cfg.buffer_length_secs > 300 && video_max > 500 * 1024 * 1024 {
        bottleneck_warnings.push(format!(
            "ℹ️ Großer Ringpuffer ({}s): ClipTool hält bis zu {:.0} MB Video im RAM vor.",
            cfg.buffer_length_secs, video_max as f64 / (1024.0 * 1024.0)
        ));
    }
    if bottleneck_warnings.is_empty() {
        bottleneck_warnings.push("✅ Optimale Konfiguration: Hardware-Encoder aktiv, Speicherbandbreite und Puffer-Ressourcen im grünen Bereich.".to_string());
    }

    Ok(ToolPerformanceSnapshot {
        timestamp_ms: now_ms,
        cpu_cores,
        total_cpu_usage: (total_cpu_usage * 10.0).round() / 10.0,
        total_cpu_normalized: (total_cpu_norm * 10.0).round() / 10.0,
        total_memory_bytes,
        total_memory_mb: (total_memory_bytes as f64 / (1024.0 * 1024.0) * 10.0).round() / 10.0,
        total_virtual_memory_bytes,
        total_virtual_memory_mb: (total_virtual_memory_bytes as f64 / (1024.0 * 1024.0) * 10.0).round() / 10.0,
        video_buffer_bytes: video_bytes,
        video_buffer_mb: (video_bytes as f64 / (1024.0 * 1024.0) * 10.0).round() / 10.0,
        video_buffer_max_bytes: video_max,
        video_buffer_max_mb: (video_max as f64 / (1024.0 * 1024.0) * 10.0).round() / 10.0,
        audio_buffer_bytes: audio_bytes,
        audio_buffer_mb: (audio_bytes as f64 / (1024.0 * 1024.0) * 10.0).round() / 10.0,
        system_specs,
        active_settings,
        pipeline_diagnostics,
        bottleneck_warnings,
        processes: processes_metrics,
    })
}

#[tauri::command]
pub fn save_perf_snapshot(content: String) -> Result<String, String> {
    // 1. Primary path in project root for immediate AI inspection
    let root_path = std::path::PathBuf::from(r"C:\Users\flori\Downloads\cliping_tool\perf_snapshot.txt");
    let _ = std::fs::write(&root_path, &content);

    // 2. Also save timestamped copy in perf_snapshots directory
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let folder = std::path::PathBuf::from(r"C:\Users\flori\Downloads\cliping_tool\perf_snapshots");
    let _ = std::fs::create_dir_all(&folder);
    let ts_file = folder.join(format!("snapshot_{}.txt", timestamp));
    let _ = std::fs::write(&ts_file, &content);

    // 3. Also write to current working directory if different
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_file = cwd.join("perf_snapshot.txt");
        if cwd_file != root_path {
            let _ = std::fs::write(&cwd_file, &content);
        }
    }

    Ok(root_path.to_string_lossy().into_owned())
}
