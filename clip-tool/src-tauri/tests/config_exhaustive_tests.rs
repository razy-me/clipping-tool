use app_lib::config::{AppConfig, BitratePreset, VideoCodec, CleanupConfig};

#[test]
fn test_config_roundtrip_all_codecs() {
    for codec in [VideoCodec::H264, VideoCodec::HEVC, VideoCodec::AV1] {
        let mut cfg = AppConfig::default();
        cfg.video_codec = codec;
        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.video_codec, codec);
    }
}

#[test]
fn test_config_roundtrip_all_bitrate_presets() {
    for preset in [BitratePreset::Low, BitratePreset::Balanced, BitratePreset::High, BitratePreset::Ultra] {
        let mut cfg = AppConfig::default();
        cfg.bitrate_preset = preset.clone();
        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.bitrate_preset, preset);
    }
}

#[test]
fn test_config_all_buffer_lengths() {
    let lengths = [15, 30, 45, 60, 90, 120, 180, 240, 300, 600];
    for len in lengths {
        let mut cfg = AppConfig::default();
        cfg.buffer_length_secs = len;
        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.buffer_length_secs, len);
    }
}

#[test]
fn test_config_all_fps_options() {
    let fps_list = ["30", "60", "120", "144", "240"];
    for fps in fps_list {
        let mut cfg = AppConfig::default();
        cfg.fps_selection = fps.to_string();
        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.fps_selection, fps);
    }
}

#[test]
fn test_config_all_resolutions() {
    let resolutions = ["720p", "1080p", "1440p", "Original"];
    for res in resolutions {
        let mut cfg = AppConfig::default();
        cfg.video_resolution = res.to_string();
        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.video_resolution, res);
    }
}

#[test]
fn test_config_cleanup_boundary_values() {
    let mut cfg = AppConfig::default();
    cfg.cleanup = CleanupConfig {
        enabled: true,
        max_age_days: 90,
        max_storage_gb: 500.0,
        min_free_disk_gb: 50.0,
    };

    let json = serde_json::to_string(&cfg).unwrap();
    let deserialized: AppConfig = serde_json::from_str(&json).unwrap();

    assert!(deserialized.cleanup.enabled);
    assert_eq!(deserialized.cleanup.max_age_days, 90);
    assert_eq!(deserialized.cleanup.max_storage_gb, 500.0);
    assert_eq!(deserialized.cleanup.min_free_disk_gb, 50.0);
}

#[test]
fn test_config_isolated_apps_various_formats() {
    let apps = vec![
        "discord.exe".to_string(),
        "C:\\Program Files\\Spotify\\Spotify.exe".to_string(),
        "d:/steam/steamapps/common/cs2/cs2.exe".to_string(),
        "C:\\Games\\Riot Games\\VALORANT.exe".to_string(),
    ];
    let mut cfg = AppConfig::default();
    cfg.isolated_apps = apps.clone();

    let json = serde_json::to_string(&cfg).unwrap();
    let deserialized: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.isolated_apps, apps);
}
