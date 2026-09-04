use app_lib::config::{AppConfig, VideoCodec, BitratePreset};

#[test]
fn test_app_config_defaults() {
    let cfg = AppConfig::default();
    assert_eq!(cfg.video_codec, VideoCodec::H264);
    assert_eq!(cfg.buffer_length_secs, 300);
    assert_eq!(cfg.hotkey, "Alt+Shift+C");
    assert!(cfg.spike_detection_enabled);
    assert_eq!(cfg.spike_threshold, 0.65);
    assert_eq!(cfg.mic_volume, 1.0);
    assert_eq!(cfg.autostart, false);
    assert_eq!(cfg.auto_clipboard, false);
}

#[test]
fn test_codec_serialization_h264() {
    let mut cfg = AppConfig::default();
    cfg.video_codec = VideoCodec::H264;
    let json = serde_json::to_string(&cfg).unwrap();
    let decoded: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.video_codec, VideoCodec::H264);
}

#[test]
fn test_codec_serialization_hevc() {
    let mut cfg = AppConfig::default();
    cfg.video_codec = VideoCodec::HEVC;
    let json = serde_json::to_string(&cfg).unwrap();
    let decoded: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.video_codec, VideoCodec::HEVC);
}

#[test]
fn test_codec_serialization_av1() {
    let mut cfg = AppConfig::default();
    cfg.video_codec = VideoCodec::AV1;
    let json = serde_json::to_string(&cfg).unwrap();
    let decoded: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.video_codec, VideoCodec::AV1);
}

#[test]
fn test_bitrate_preset_low() {
    let mut cfg = AppConfig::default();
    cfg.bitrate_preset = BitratePreset::Low;
    let json = serde_json::to_string(&cfg).unwrap();
    let decoded: AppConfig = serde_json::from_str(&json).unwrap();
    assert!(matches!(decoded.bitrate_preset, BitratePreset::Low));
}

#[test]
fn test_bitrate_preset_balanced() {
    let mut cfg = AppConfig::default();
    cfg.bitrate_preset = BitratePreset::Balanced;
    let json = serde_json::to_string(&cfg).unwrap();
    let decoded: AppConfig = serde_json::from_str(&json).unwrap();
    assert!(matches!(decoded.bitrate_preset, BitratePreset::Balanced));
}

#[test]
fn test_bitrate_preset_high() {
    let mut cfg = AppConfig::default();
    cfg.bitrate_preset = BitratePreset::High;
    let json = serde_json::to_string(&cfg).unwrap();
    let decoded: AppConfig = serde_json::from_str(&json).unwrap();
    assert!(matches!(decoded.bitrate_preset, BitratePreset::High));
}

#[test]
fn test_bitrate_preset_ultra() {
    let mut cfg = AppConfig::default();
    cfg.bitrate_preset = BitratePreset::Ultra;
    let json = serde_json::to_string(&cfg).unwrap();
    let decoded: AppConfig = serde_json::from_str(&json).unwrap();
    assert!(matches!(decoded.bitrate_preset, BitratePreset::Ultra));
}

#[test]
fn test_fps_selections() {
    for fps in ["30", "60", "120", "144", "240"] {
        let mut cfg = AppConfig::default();
        cfg.fps_selection = fps.to_string();
        let json = serde_json::to_string(&cfg).unwrap();
        let decoded: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.fps_selection, fps);
    }
}

#[test]
fn test_buffer_lengths() {
    for secs in [15, 30, 60, 120, 300, 600, 1200] {
        let mut cfg = AppConfig::default();
        cfg.buffer_length_secs = secs;
        let json = serde_json::to_string(&cfg).unwrap();
        let decoded: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.buffer_length_secs, secs);
    }
}

#[test]
fn test_isolated_apps_path_normalization_various_formats() {
    let raw_entries = vec![
        "C:\\Program Files\\Discord\\Discord.exe".to_string(),
        "D:/Games/Steam/steam.exe".to_string(),
        "E:\\Tools\\SubFolder.Test\\Obs.exe".to_string(),
        "Spotify.exe".to_string(),
        "game_client.exe".to_string(),
    ];

    let normalized: Vec<String> = raw_entries.into_iter().map(|entry| {
        std::path::Path::new(&entry)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or(entry)
    }).collect();

    assert_eq!(normalized, vec![
        "Discord.exe",
        "steam.exe",
        "Obs.exe",
        "Spotify.exe",
        "game_client.exe"
    ]);
}

// ──────────────────────────────────────────────────────────────────────────────
// Serde Default & Partial Deserialization Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_config_deserialization_from_minimal_json() {
    let json = r#"{
        "custom_clip_path": "C:\\Clips",
        "buffer_length_secs": 180,
        "fps_selection": "60",
        "video_resolution": "1080p",
        "bitrate_preset": "High",
        "mic_threshold": 0.5,
        "hotkey": "Alt+C"
    }"#;
    let cfg: AppConfig = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.video_codec, VideoCodec::H264); // default
    assert_eq!(cfg.mic_volume, 1.0); // default
    assert_eq!(cfg.spike_detection_enabled, true); // default
    assert_eq!(cfg.spike_threshold, 0.65); // default
    assert_eq!(cfg.show_cursor_in_clips, true); // default
    assert_eq!(cfg.monitor_idx, 0); // default
    assert_eq!(cfg.controller_clipping, false); // default
    assert_eq!(cfg.audio_sync_offset_ms, 200); // default
}

#[test]
fn test_config_audio_sync_offsets() {
    for offset in [-1000, -500, -250, 0, 100, 250, 500, 1000] {
        let mut cfg = AppConfig::default();
        cfg.audio_sync_offset_ms = offset;
        let json = serde_json::to_string(&cfg).unwrap();
        let decoded: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.audio_sync_offset_ms, offset);
    }
}

#[test]
fn test_config_controller_clipping_flag() {
    let mut cfg = AppConfig::default();
    cfg.controller_clipping = true;
    let json = serde_json::to_string(&cfg).unwrap();
    let decoded: AppConfig = serde_json::from_str(&json).unwrap();
    assert!(decoded.controller_clipping);
}

#[test]
fn test_config_show_cursor_flag() {
    let mut cfg = AppConfig::default();
    cfg.show_cursor_in_clips = false;
    let json = serde_json::to_string(&cfg).unwrap();
    let decoded: AppConfig = serde_json::from_str(&json).unwrap();
    assert!(!decoded.show_cursor_in_clips);
}

#[test]
fn test_config_monitor_indexes() {
    for idx in 0..=8 {
        let mut cfg = AppConfig::default();
        cfg.monitor_idx = idx;
        let json = serde_json::to_string(&cfg).unwrap();
        let decoded: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.monitor_idx, idx);
    }
}

#[test]
fn test_config_resolutions_all_presets() {
    for res in ["Original", "1440p", "1080p", "900p", "720p", "540p", "480p", "360p", "240p"] {
        let mut cfg = AppConfig::default();
        cfg.video_resolution = res.to_string();
        let json = serde_json::to_string(&cfg).unwrap();
        let decoded: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.video_resolution, res);
    }
}

#[test]
fn test_config_custom_paths_with_unicode_and_spaces() {
    for path in [
        "C:\\Users\\Gamer\\Videos\\Clip Sammlung",
        "D:/Spiele/Höhepunkte/2026",
        "E:\\Recs (HD) [Vault] #1",
    ] {
        let mut cfg = AppConfig::default();
        cfg.custom_clip_path = path.to_string();
        let json = serde_json::to_string(&cfg).unwrap();
        let decoded: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.custom_clip_path, path);
    }
}

#[test]
fn test_config_all_buffer_length_ranges() {
    for secs in [10u32, 15, 30, 45, 60, 90, 120, 180, 240, 300, 600, 900, 1200, 1800, 3600] {
        let mut cfg = AppConfig::default();
        cfg.buffer_length_secs = secs;
        let json = serde_json::to_string(&cfg).unwrap();
        let decoded: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.buffer_length_secs, secs);
    }
}

#[test]
fn test_config_mic_threshold_ranges() {
    for thresh in [0.0f32, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0] {
        let mut cfg = AppConfig::default();
        cfg.mic_threshold = thresh;
        let json = serde_json::to_string(&cfg).unwrap();
        let decoded: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.mic_threshold, thresh);
    }
}

#[test]
fn test_config_spike_threshold_ranges() {
    for thresh in [0.1f32, 0.35, 0.5, 0.65, 0.8, 0.95] {
        let mut cfg = AppConfig::default();
        cfg.spike_threshold = thresh;
        let json = serde_json::to_string(&cfg).unwrap();
        let decoded: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.spike_threshold, thresh);
    }
}

#[test]
fn test_config_all_fps_strings() {
    for fps in ["15", "24", "30", "45", "60", "90", "120", "144", "165", "240", "360"] {
        let mut cfg = AppConfig::default();
        cfg.fps_selection = fps.to_string();
        let json = serde_json::to_string(&cfg).unwrap();
        let decoded: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.fps_selection, fps);
    }
}

#[test]
fn test_config_all_bitrate_preset_variants() {
    for preset in [BitratePreset::Low, BitratePreset::Balanced, BitratePreset::High, BitratePreset::Ultra] {
        let mut cfg = AppConfig::default();
        cfg.bitrate_preset = preset;
        let json = serde_json::to_string(&cfg).unwrap();
        let decoded: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.bitrate_preset, preset);
    }
}


