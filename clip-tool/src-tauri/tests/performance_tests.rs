use app_lib::performance::{PerformanceMonitorState, ToolPerformanceSnapshot, ProcessMetric, SystemSpecs, ActiveSettingsSnapshot, PipelineDiagnostics};

#[test]
fn test_performance_monitor_state_creation() {
    let state = PerformanceMonitorState::new();
    let sys = state.system.lock().unwrap();
    assert!(!sys.processes().is_empty());
    assert!(sys.cpus().len() >= 1);
}

#[test]
fn test_performance_snapshot_serialization_roundtrip() {
    let metric = ProcessMetric {
        pid: 1234,
        name: "clip-tool.exe".to_string(),
        role: "Hauptanwendung (Rust & Tauri Core)".to_string(),
        cpu_usage: 1.5,
        cpu_usage_normalized: 0.1,
        memory_bytes: 52_428_800,
        memory_mb: 50.0,
        virtual_memory_bytes: 83_886_080,
        virtual_memory_mb: 80.0,
        disk_read_bytes: 1024,
        disk_written_bytes: 4096,
    };

    let specs = SystemSpecs {
        os_name: "Windows 11 Pro".to_string(),
        cpu_name: "Intel(R) Core(TM) i7".to_string(),
        cpu_cores: 16,
        total_ram_mb: 16384.0,
        available_ram_mb: 8192.0,
        power_source: "Netzbetrieb (AC Netzstrom)".to_string(),
        hardware_tier: "High-End Gaming Rig (Tier: Strong)".to_string(),
        gpu_name: "Intel(R) Arc(TM) GPU".to_string(),
        display_resolution: "2880 × 1800".to_string(),
        display_refresh_rate: 60,
        capture_pixel_rate_mps: 311.0,
        raw_video_bandwidth_mbs: 1244.0,
        disk_free_gb: 50.0,
        disk_total_gb: 512.0,
    };

    let settings = ActiveSettingsSnapshot {
        video_codec: "HEVC / H.265 (High Efficiency)".to_string(),
        video_resolution: "1080p".to_string(),
        fps: "60".to_string(),
        bitrate_preset: "High (20.000 kbps)".to_string(),
        buffer_length_secs: 120,
        monitor_idx: 0,
        show_cursor: true,
        hdr_tonemapping: false,
        spike_detection: true,
        spike_threshold: 0.25,
        controller_clipping: true,
        audio_sync_offset_ms: 0,
        isolated_apps_count: 2,
    };

    let diag = PipelineDiagnostics {
        active_encoder: "hevc_qsv".to_string(),
        hw_acceleration: "Aktiv (Intel QuickSync / Xe2 ASIC)".to_string(),
        target_resolution: "1920 × 1080 (1080p)".to_string(),
        pixel_reduction_pct: 60.0,
        video_buffer_stored_secs: 120.0,
        video_buffer_chunks: 500,
        audio_tracks_count: 2,
        scaling_method: "Fast-Bilinear SIMD".to_string(),
    };

    let snapshot = ToolPerformanceSnapshot {
        timestamp_ms: 1700000000000,
        cpu_cores: 16,
        total_cpu_usage: 1.5,
        total_cpu_normalized: 0.1,
        total_memory_bytes: 52_428_800,
        total_memory_mb: 50.0,
        total_virtual_memory_bytes: 83_886_080,
        total_virtual_memory_mb: 80.0,
        video_buffer_bytes: 10_000_000,
        video_buffer_mb: 9.5,
        video_buffer_max_bytes: 100_000_000,
        video_buffer_max_mb: 95.4,
        audio_buffer_bytes: 1_000_000,
        audio_buffer_mb: 1.0,
        system_specs: specs,
        active_settings: settings,
        pipeline_diagnostics: diag,
        bottleneck_warnings: vec!["✅ Optimale Konfiguration".to_string()],
        processes: vec![metric],
    };

    let json = serde_json::to_string(&snapshot).unwrap();
    let deserialized: ToolPerformanceSnapshot = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.cpu_cores, 16);
    assert_eq!(deserialized.processes.len(), 1);
    assert_eq!(deserialized.processes[0].name, "clip-tool.exe");
    assert_eq!(deserialized.system_specs.gpu_name, "Intel(R) Arc(TM) GPU");
    assert_eq!(deserialized.pipeline_diagnostics.active_encoder, "hevc_qsv");
    assert_eq!(deserialized.bottleneck_warnings.len(), 1);
}

#[test]
fn test_process_tree_exclusion_of_system_services() {
    let state = PerformanceMonitorState::new();
    let sys = state.system.lock().unwrap();

    let my_pid = sysinfo::Pid::from_u32(std::process::id());
    let mut tool_pids = std::collections::HashSet::new();
    tool_pids.insert(my_pid);

    let mut added = true;
    while added {
        added = false;
        for (pid, process) in sys.processes() {
            if !tool_pids.contains(pid) {
                if let Some(parent) = process.parent() {
                    if tool_pids.contains(&parent) {
                        tool_pids.insert(*pid);
                        added = true;
                    }
                }
            }
        }
    }

    for (pid, process) in sys.processes() {
        let name = process.name().to_string_lossy().to_lowercase();
        if name == "explorer.exe" || name == "dwm.exe" || name == "svchost.exe" {
            assert!(
                !tool_pids.contains(pid),
                "Process '{}' (PID {:?}) must NOT be in the tool process tree!",
                name,
                pid
            );
        }
    }
}
