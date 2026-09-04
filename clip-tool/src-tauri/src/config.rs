use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use std::sync::Mutex;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum BitratePreset {
    Low,
    Balanced,
    High,
    Ultra,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    HEVC,
    AV1,
}

impl Default for VideoCodec {
    fn default() -> Self {
        VideoCodec::H264
    }
}

fn default_spike_detection() -> bool { true }
fn default_spike_threshold() -> f32 { 0.65 }
fn default_mic_volume() -> f32 { 1.0 }
fn default_show_cursor() -> bool { true }
fn default_idle_minutes() -> u32 { 5 }
fn default_overlay_notification() -> bool { true }
fn default_cleanup_enabled() -> bool { true }
fn default_cleanup_max_age_days() -> u32 { 30 }
fn default_cleanup_max_storage_gb() -> f64 { 50.0 }
fn default_cleanup_min_free_disk_gb() -> f64 { 15.0 }

fn default_audio_sync_offset_ms() -> i32 { 200 }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HotkeyBinding {
    pub id: String,
    pub hotkey: String,
    pub duration_secs: u32,
    pub label: String,
}

pub fn default_hotkey_bindings() -> Vec<HotkeyBinding> {
    vec![
        HotkeyBinding {
            id: "slot_1".to_string(),
            hotkey: "Alt+C".to_string(),
            duration_secs: 30,
            label: "Kurzer Clip (30s)".to_string(),
        },
        HotkeyBinding {
            id: "slot_2".to_string(),
            hotkey: "Alt+Shift+C".to_string(),
            duration_secs: 120,
            label: "Standard Clip (2min)".to_string(),
        },
        HotkeyBinding {
            id: "slot_3".to_string(),
            hotkey: "Alt+Ctrl+C".to_string(),
            duration_secs: 0,
            label: "Ganzer Puffer".to_string(),
        },
    ]
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CleanupConfig {
    #[serde(default = "default_cleanup_enabled")]
    pub enabled: bool,
    #[serde(default = "default_cleanup_max_age_days")]
    pub max_age_days: u32,
    #[serde(default = "default_cleanup_max_storage_gb")]
    pub max_storage_gb: f64,
    #[serde(default = "default_cleanup_min_free_disk_gb")]
    pub min_free_disk_gb: f64,
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_age_days: 30,
            max_storage_gb: 50.0,
            min_free_disk_gb: 15.0,
        }
    }
}

fn default_cleanup() -> CleanupConfig {
    CleanupConfig::default()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppConfig {
    pub custom_clip_path: String,
    pub buffer_length_secs: u32,
    pub fps_selection: String,
    pub video_resolution: String,
    pub bitrate_preset: BitratePreset,
    #[serde(default)]
    pub video_codec: VideoCodec,
    pub mic_threshold: f32,
    #[serde(default = "default_mic_volume")]
    pub mic_volume: f32,
    pub hotkey: String,
    #[serde(default = "default_hotkey_bindings")]
    pub hotkeys: Vec<HotkeyBinding>,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default)]
    pub auto_clipboard: bool,
    #[serde(default = "default_spike_detection")]
    pub spike_detection_enabled: bool,
    #[serde(default = "default_spike_threshold")]
    pub spike_threshold: f32,
    #[serde(default)]
    pub isolated_apps: Vec<String>,
    #[serde(default = "default_show_cursor")]
    pub show_cursor_in_clips: bool,
    #[serde(default)]
    pub monitor_idx: u32,
    #[serde(default)]
    pub controller_clipping: bool,
    #[serde(default = "default_audio_sync_offset_ms")]
    pub audio_sync_offset_ms: i32,
    #[serde(default = "default_cleanup")]
    pub cleanup: CleanupConfig,
    #[serde(default)]
    pub hdr_tonemapping: bool,
    #[serde(default)]
    pub auto_pause_idle: bool,
    #[serde(default = "default_idle_minutes")]
    pub auto_pause_idle_minutes: u32,
    #[serde(default = "default_overlay_notification")]
    pub overlay_notification: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        let cores = std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(4);
        let mut sys = sysinfo::System::new_with_specifics(
            sysinfo::RefreshKind::nothing().with_memory(sysinfo::MemoryRefreshKind::nothing().with_ram())
        );
        sys.refresh_memory();
        let total_ram = sys.total_memory() / 1024 / 1024;
        
        let (fps, res, bitrate, cursor) = if cores <= 4 || total_ram <= 8192 {
            ("30".to_string(), "720p".to_string(), BitratePreset::Low, false)
        } else {
            ("60".to_string(), "Original".to_string(), BitratePreset::Balanced, true)
        };

        Self {
            custom_clip_path: "".to_string(),
            buffer_length_secs: 300,
            fps_selection: fps,
            video_resolution: res,
            bitrate_preset: bitrate,
            video_codec: VideoCodec::H264,
            mic_threshold: 0.5,
            mic_volume: 1.0,
            hotkey: "Alt+Shift+C".to_string(),
            hotkeys: default_hotkey_bindings(),
            autostart: false,
            auto_clipboard: false,
            spike_detection_enabled: true,
            spike_threshold: 0.65,
            isolated_apps: Vec::new(),
            show_cursor_in_clips: cursor,
            monitor_idx: 0,
            controller_clipping: false,
            audio_sync_offset_ms: 200,
            cleanup: CleanupConfig::default(),
            hdr_tonemapping: false,
            auto_pause_idle: false,
            auto_pause_idle_minutes: 5,
            overlay_notification: true,
        }
    }
}

pub fn get_config_path(app: &AppHandle) -> PathBuf {
    let mut path = app.path().app_config_dir().expect("Failed to get config dir");
    let _ = fs::create_dir_all(&path);
    path.push("config.json");
    path
}

static CONFIG_CACHE: Mutex<Option<AppConfig>> = Mutex::new(None);

#[tauri::command]
pub fn get_config(app: AppHandle) -> AppConfig {
    if let Some(cfg) = CONFIG_CACHE.lock().unwrap().as_ref() {
        return cfg.clone();
    }

    let path = get_config_path(&app);
    let mut loaded = None;
    if path.exists() {
        if let Ok(data) = fs::read_to_string(&path) {
            if let Ok(config) = serde_json::from_str::<AppConfig>(&data) {
                loaded = Some(config);
            }
        }
    }
    
    let mut config = loaded.unwrap_or_else(|| AppConfig::default());
    if config.hotkeys.is_empty() {
        config.hotkeys = default_hotkey_bindings();
    }
    if config.custom_clip_path.is_empty() {
        if let Ok(video_dir) = app.path().video_dir() {
            config.custom_clip_path = video_dir.join("CustomClips").to_string_lossy().to_string();
            let _ = save_config(app.clone(), config.clone());
        }
    }
    
    *CONFIG_CACHE.lock().unwrap() = Some(config.clone());
    config
}

#[tauri::command]
pub fn save_config(app: AppHandle, mut config: AppConfig) -> Result<(), String> {
    if let Some(first) = config.hotkeys.first() {
        config.hotkey = first.hotkey.clone();
    }

    config.isolated_apps = config.isolated_apps.into_iter().map(|entry| {
        std::path::Path::new(&entry)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or(entry)
    }).collect();

    let path = get_config_path(&app);
    let data = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;

    // Atomic write: write to temp file first, then atomically rename over target
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, &data).map_err(|e| e.to_string())?;
    if let Err(_) = fs::rename(&tmp_path, &path) {
        // Fallback for Windows if rename fails while target is locked
        let _ = fs::remove_file(&path);
        fs::rename(&tmp_path, &path).map_err(|e| e.to_string())?;
    }

    // Update atomic caches for background threads
    crate::hotkeys::update_controller_enabled(config.controller_clipping);
    crate::idle_monitor::update_idle_config(config.auto_pause_idle, config.auto_pause_idle_minutes);

    *CONFIG_CACHE.lock().unwrap() = Some(config);
    Ok(())
}
