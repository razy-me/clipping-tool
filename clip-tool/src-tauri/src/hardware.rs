use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_shell::ShellExt;
use crate::config::VideoCodec;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ScalingMethod {
    Cuda,         // NVIDIA: scale_cuda (Pure VRAM)
    Qsv,          // Intel: scale_qsv (Pure VRAM / iGPU)
    D3d11Direct,  // AMD AMF / Direct D3D11 (no hwdownload for native, fallback scale)
    CpuFallback,  // Software / Older GPUs: hwdownload + swscale
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct HardwareDiskCache {
    encoders: HashMap<String, String>,
    scaling_methods: HashMap<String, ScalingMethod>,
}

static CACHE: Mutex<Option<HardwareDiskCache>> = Mutex::new(None);

fn load_disk_cache(app: &AppHandle) -> HardwareDiskCache {
    if let Ok(config_dir) = app.path().app_config_dir() {
        let cache_file = config_dir.join("encoder_cache.json");
        if cache_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&cache_file) {
                if let Ok(cached) = serde_json::from_str::<HardwareDiskCache>(&content) {
                    return cached;
                }
            }
        }
    }
    HardwareDiskCache::default()
}

fn save_disk_cache(app: &AppHandle, cache: &HardwareDiskCache) {
    if let Ok(config_dir) = app.path().app_config_dir() {
        let _ = std::fs::create_dir_all(&config_dir);
        let cache_file = config_dir.join("encoder_cache.json");
        if let Ok(json) = serde_json::to_string_pretty(cache) {
            let _ = std::fs::write(cache_file, json);
        }
    }
}

pub async fn detect_best_encoder_for_codec(app: &AppHandle, codec: &VideoCodec) -> String {
    let codec_str = match codec {
        VideoCodec::H264 => "H264",
        VideoCodec::HEVC => "HEVC",
        VideoCodec::AV1 => "AV1",
    };

    {
        let mut guard = CACHE.lock().unwrap();
        if guard.is_none() {
            *guard = Some(load_disk_cache(app));
        }
        if let Some(cache) = guard.as_ref() {
            if let Some(enc) = cache.encoders.get(codec_str) {
                return enc.clone();
            }
        }
    }

    let (candidates, fallback) = match codec {
        VideoCodec::H264 => (&["h264_nvenc", "h264_amf", "h264_qsv"][..], "libx264"),
        VideoCodec::HEVC => (&["hevc_nvenc", "hevc_amf", "hevc_qsv"][..], "libx265"),
        VideoCodec::AV1 => (&["av1_nvenc", "av1_amf", "av1_qsv"][..], "libsvtav1"),
    };

    // Sequential probing with early exit: avoids concurrent GPU driver context conflicts / TDR resets (F-15)
    for enc in candidates {
        if probe_encoder(app, enc).await {
            println!("[encoder] {} available for {:?}", enc, codec);
            let mut guard = CACHE.lock().unwrap();
            let cache = guard.get_or_insert_with(HardwareDiskCache::default);
            cache.encoders.insert(codec_str.to_string(), enc.to_string());
            save_disk_cache(app, cache);
            return enc.to_string();
        }
    }

    if *codec == VideoCodec::AV1 {
        let fallbacks = ["hevc_nvenc", "h264_nvenc", "hevc_amf", "h264_amf", "hevc_qsv", "h264_qsv"];
        for enc in fallbacks {
            if probe_encoder(app, enc).await {
                println!("[encoder] AV1 hardware encoder not supported on this GPU, using {} instead", enc);
                let mut guard = CACHE.lock().unwrap();
                let cache = guard.get_or_insert_with(HardwareDiskCache::default);
                cache.encoders.insert(codec_str.to_string(), enc.to_string());
                save_disk_cache(app, cache);
                return enc.to_string();
            }
        }
    }

    println!("[encoder] no hardware encoder found for {:?}, falling back to {}", codec, fallback);
    let mut guard = CACHE.lock().unwrap();
    let cache = guard.get_or_insert_with(HardwareDiskCache::default);
    cache.encoders.insert(codec_str.to_string(), fallback.to_string());
    save_disk_cache(app, cache);
    fallback.to_string()
}

pub async fn detect_scaling_method(_app: &AppHandle, encoder: &str) -> ScalingMethod {
    let method = if encoder.contains("qsv") {
        ScalingMethod::Qsv
    } else if encoder.contains("nvenc") {
        ScalingMethod::Cuda
    } else if encoder.contains("amf") {
        ScalingMethod::D3d11Direct
    } else {
        ScalingMethod::CpuFallback
    };
    println!("[hardware] scaling method for {}: {:?}", encoder, method);
    method
}

pub async fn detect_best_encoder(app: &AppHandle) -> String {
    let cfg = crate::config::get_config(app.clone());
    detect_best_encoder_for_codec(app, &cfg.video_codec).await
}

#[tauri::command]
pub async fn get_encoder(app: AppHandle) -> String {
    detect_best_encoder(&app).await
}

#[tauri::command]
pub async fn get_scaling_method(app: AppHandle) -> ScalingMethod {
    let enc = detect_best_encoder(&app).await;
    detect_scaling_method(&app, &enc).await
}

#[tauri::command]
pub async fn get_available_encoders(app: AppHandle) -> HashMap<String, String> {
    let mut results = HashMap::new();
    for codec in &[VideoCodec::H264, VideoCodec::HEVC, VideoCodec::AV1] {
        let name = match codec {
            VideoCodec::H264 => "H264",
            VideoCodec::HEVC => "HEVC",
            VideoCodec::AV1 => "AV1",
        };
        let enc = detect_best_encoder_for_codec(&app, codec).await;
        results.insert(name.to_string(), enc);
    }
    results
}

async fn probe_encoder(app: &AppHandle, encoder: &str) -> bool {
    let pix_fmt = if encoder.contains("qsv") || encoder.contains("amf") {
        "nv12"
    } else {
        "yuv420p"
    };

    let args: Vec<String> = vec![
        "-hide_banner".into(),
        "-loglevel".into(), "error".into(),
        "-f".into(), "lavfi".into(),
        "-i".into(), "testsrc=size=256x256:rate=1".into(),
        "-pix_fmt".into(), pix_fmt.into(),
        "-frames:v".into(), "1".into(),
        "-c:v".into(), encoder.into(),
        "-f".into(), "null".into(),
        "-".into(),
    ];

    let cmd = match app.shell().sidecar("ffmpeg").or_else(|_| app.shell().command("ffmpeg")) {
        Ok(c) => c.args(args),
        Err(_) => return false,
    };

    match tokio::time::timeout(Duration::from_secs(4), cmd.output()).await {
        Ok(Ok(out)) => out.status.success(),
        _ => false,
    }
}
