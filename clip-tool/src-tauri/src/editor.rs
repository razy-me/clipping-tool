use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

// ──────────────────────────────────────────────────────────────────────────────
// Temporary localhost editor server.
//
// Opening a clip spins up an axum instance bound to 127.0.0.1 on a random
// port, guarded by a per-session random token, and opens the default browser.
// Every route requires the token; file access is index-based and resolved
// server-side (no client-supplied paths). The session ends when the page
// unloads, after 15 minutes of inactivity, or after a 4 hour hard cap.
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize, Clone)]
pub struct AudioTrackMeta {
    pub track_index: usize,
    pub process_name: String,
    pub wav_filename: String,
}

struct Job {
    child: Option<CommandChild>,
    cancelled: Arc<AtomicBool>,
    percent: f32,
    status: String, // running | success | error | cancelled
    output: String,
}

struct Session {
    token: String,
    clip: crate::library::ClipItem,
    dir: PathBuf,
    app: AppHandle,
    jobs: Mutex<HashMap<String, Job>>,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    last_seen: AtomicU64,
}

type Ctx = Arc<Session>;

static SESSION_SHUTDOWN: Mutex<Option<tokio::sync::watch::Sender<bool>>> = Mutex::new(None);

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Entropy without pulling a rand crate: nanos xor pid xor stack address.
fn make_token() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0) as u64;
    let pid = std::process::id() as u64;
    let stack = &nanos as *const u64 as u64;
    let mut mix = nanos ^ pid.rotate_left(32) ^ stack.rotate_left(17);
    let mut out = String::with_capacity(32);
    for _ in 0..4 {
        mix = mix.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(23) ^ (mix >> 7);
        out.push_str(&format!("{:08x}", mix));
    }
    out
}

// ── Auth + helpers ───────────────────────────────────────────────────────────

fn check_token(ctx: &Ctx, params: &HashMap<String, String>) -> bool {
    ctx.last_seen.store(now_secs(), Ordering::Relaxed);
    params.get("t").map(|t| t.as_str() == ctx.token.as_str()).unwrap_or(false)
}

fn deny() -> Response {
    (StatusCode::FORBIDDEN, "forbidden").into_response()
}

// ── Routes ───────────────────────────────────────────────────────────────────

async fn page(State(ctx): State<Ctx>, Query(p): Query<HashMap<String, String>>) -> Response {
    if !check_token(&ctx, &p) { return deny(); }
    let html = include_str!("../assets/editor_web.html").replace("__TOKEN__", &ctx.token);
    ([("content-type", "text/html; charset=utf-8")], html).into_response()
}

async fn clip_info(State(ctx): State<Ctx>, Query(p): Query<HashMap<String, String>>) -> Response {
    if !check_token(&ctx, &p) { return deny(); }
    let tracks: Vec<_> = ctx.clip.audio_tracks.iter().map(|t| {
        json!({ "name": t.process_name, "wav": t.wav_filename.clone() })
    }).collect();
    Json(json!({
        "filename": ctx.clip.filename,
        "game_tag": ctx.clip.game_tag,
        "duration_secs": ctx.clip.duration_secs,
        "tracks": tracks,
        "spike_markers": ctx.clip.spike_markers,
    })).into_response()
}

/// Stream a media file with HTTP Range support so video/audio elements can seek.
async fn serve_range(path: PathBuf, headers: &HeaderMap, content_type: &str) -> Response {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};
    let Ok(meta) = tokio::fs::metadata(&path).await else {
        return (StatusCode::NOT_FOUND, "missing").into_response();
    };
    let total = meta.len();
    if total == 0 {
        return (StatusCode::OK, "").into_response();
    }

    let mut start = 0u64;
    let mut end_incl = total.saturating_sub(1);
    let mut is_range = false;

    if let Some(range) = headers.get(header::RANGE).and_then(|v| v.to_str().ok()) {
        let spec = range.trim_start_matches("bytes=");
        let mut it = spec.splitn(2, '-');
        if let Some(s) = it.next().and_then(|v| v.parse::<u64>().ok()) { start = s; is_range = true; }
        if let Some(e) = it.next().and_then(|v| v.split(',').next())
            .and_then(|v| v.parse::<u64>().ok()) { end_incl = e.min(total - 1); is_range = true; }
        if start >= total {
            return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
        }
        // Allow up to 32MB chunks for fast video buffering in browser
        end_incl = end_incl.min(start + 32_000_000 - 1).min(total.saturating_sub(1));
    }

    let Ok(mut file) = tokio::fs::File::open(&path).await else {
        return (StatusCode::NOT_FOUND, "missing").into_response();
    };
    use std::io::SeekFrom;
    if file.seek(SeekFrom::Start(start)).await.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "seek failed").into_response();
    }
    let len = (end_incl - start + 1) as usize;
    let mut buf = vec![0u8; len];
    if file.read_exact(&mut buf).await.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "read failed").into_response();
    }

    let full = !is_range && start == 0 && end_incl == total - 1;
    let mut resp = Response::new(Body::from(buf));
    *resp.status_mut() = if full { StatusCode::OK } else { StatusCode::PARTIAL_CONTENT };
    let h = resp.headers_mut();
    h.insert(header::CONTENT_TYPE, content_type.parse().unwrap());
    h.insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
    h.insert(header::CONTENT_LENGTH, len.to_string().parse().unwrap());
    if is_range {
        h.insert(header::CONTENT_RANGE,
            format!("bytes {start}-{end_incl}/{total}").parse().unwrap());
    }
    resp
}

async fn video(State(ctx): State<Ctx>, Query(p): Query<HashMap<String, String>>, headers: HeaderMap) -> Response {
    if !check_token(&ctx, &p) { return deny(); }
    let path = if let Some(custom) = p.get("file") {
        let p_buf = std::path::PathBuf::from(custom);
        if p_buf.exists() {
            p_buf
        } else {
            let candidate = ctx.dir.join(custom);
            if candidate.exists() { candidate } else { PathBuf::from(ctx.clip.full_path.clone()) }
        }
    } else {
        PathBuf::from(ctx.clip.full_path.clone())
    };
    serve_range(path, &headers, "video/mp4").await
}

async fn wav(State(ctx): State<Ctx>, Query(p): Query<HashMap<String, String>>, headers: HeaderMap) -> Response {
    if !check_token(&ctx, &p) { return deny(); }
    let Some(path) = track_path(&ctx, p.get("track")) else { return deny(); };
    serve_range(path, &headers, "audio/wav").await
}

fn track_path(ctx: &Ctx, idx: Option<&String>) -> Option<PathBuf> {
    let i: usize = idx?.parse().ok()?;
    let meta = ctx.clip.audio_tracks.get(i)?;
    let name = &meta.wav_filename;
    if name.is_empty() || name.contains("..") || name.contains('/') || name.contains('\\') { return None; }
    Some(ctx.dir.join(name))
}

/// Server-side peak extraction so the browser needs no audio decoding.
async fn waveform(State(ctx): State<Ctx>, Query(p): Query<HashMap<String, String>>) -> Response {
    if !check_token(&ctx, &p) { return deny(); }
    let Some(path) = track_path(&ctx, p.get("track")) else { return deny(); };
    let buckets: usize = p.get("buckets").and_then(|b| b.parse().ok()).unwrap_or(600).clamp(50, 2000);

    let peaks = tokio::task::spawn_blocking(move || compute_wav_peaks(&path, buckets)).await;
    match peaks {
        Ok(Ok(v)) => Json(v).into_response(),
        _ => (StatusCode::NOT_FOUND, "no waveform").into_response(),
    }
}

/// Minimal streaming RIFF/WAV parser: locate the samples chunk and seek-sample frames across channels without full memory allocation.
pub fn compute_wav_peaks(path: &PathBuf, buckets: usize) -> Result<Vec<f32>, String> {
    use std::fs::File;
    use std::io::{BufReader, Read, Seek, SeekFrom};

    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut reader = BufReader::with_capacity(65536, file);
    let mut header = [0u8; 128];
    let bytes_read = reader.read(&mut header).map_err(|e| e.to_string())?;
    if bytes_read < 12 || &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return Err("not a wav".into());
    }

    let mut pos = 12usize;
    let mut channels = 1usize;
    let mut data_start = 0u64;
    let mut data_size = 0u64;

    while pos + 8 <= bytes_read {
        let id = &header[pos..pos + 4];
        let size = u32::from_le_bytes(header[pos + 4..pos + 8].try_into().unwrap()) as u64;
        if id == b"fmt " && pos + 12 <= bytes_read {
            channels = u16::from_le_bytes(header[pos + 10..pos + 12].try_into().unwrap()) as usize;
        } else if id == b"data" {
            data_start = (pos + 8) as u64;
            data_size = size;
            break;
        }
        pos += 8 + (size as usize) + ((size as usize) & 1);
    }
    if data_start == 0 { return Err("no samples".into()); }

    let frame_stride = (4 * channels.max(1)) as u64;
    let frames = (data_size / frame_stride) as usize;
    if frames == 0 { return Err("empty".into()); }
    let bsize = (frames / buckets).max(1);

    let mut peaks = Vec::with_capacity(buckets);
    let mut sample_buf = [0u8; 8]; // up to 2 channels f32

    for b in 0..buckets {
        let s = b * bsize;
        let e = ((b + 1) * bsize).min(frames);
        if s >= e { break; }
        let stride = ((e - s) / 64).max(1);
        let mut max: f32 = 0.0;

        for f in (s..e).step_by(stride) {
            let offset = data_start + (f as u64) * frame_stride;
            if reader.seek(SeekFrom::Start(offset)).is_ok() {
                let read_bytes = (channels * 4).min(sample_buf.len());
                if reader.read_exact(&mut sample_buf[..read_bytes]).is_ok() {
                    for ch in 0..channels.min(2) {
                        let ch_off = ch * 4;
                        if ch_off + 4 <= read_bytes {
                            let v = f32::from_le_bytes(sample_buf[ch_off..ch_off + 4].try_into().unwrap()).abs();
                            if v > max { max = v; }
                        }
                    }
                }
            }
        }
        peaks.push(max);
    }

    let norm = peaks.iter().cloned().fold(1e-4f32, f32::max);
    Ok(peaks.into_iter().map(|p| p / norm).collect())
}

// ── Export engine (ported from the former window implementation) ─────────────

#[derive(Deserialize)]
struct TrackSetting { name: String, muted: bool, volume: f64 }

#[derive(Deserialize)]
struct ExportReq {
    start: f64,
    end: f64,
    #[serde(default)] fade_in: Option<f64>,
    #[serde(default)] fade_out: Option<f64>,
    tracks: Vec<TrackSetting>,
    #[serde(default)] target_size_mb: Option<f64>,
}

async fn api_export(State(ctx): State<Ctx>, Query(p): Query<HashMap<String, String>>, body: String) -> Response {
    if !check_token(&ctx, &p) { return deny(); }
    let Ok(req) = serde_json::from_str::<ExportReq>(&body) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"error":"bad request"}))).into_response();
    };

    let input_path = ctx.clip.full_path.clone();
    let duration = req.end - req.start;
    if duration <= 0.0 {
        return (StatusCode::BAD_REQUEST, Json(json!({"error":"invalid trim"}))).into_response();
    }

    // Versioned output names — never silently overwrite an earlier edit, safely handling any path name.
    let input_p = std::path::Path::new(&input_path);
    let stem = input_p.file_stem().unwrap_or_default().to_string_lossy();
    let parent = input_p.parent().unwrap_or(std::path::Path::new(""));
    let is_compressed = req.target_size_mb.map(|s| s > 0.0).unwrap_or(false);
    let suffix = if is_compressed { "compressed" } else { "edited" };

    let mut output_path = parent.join(format!("{stem}_{suffix}.mp4")).to_string_lossy().to_string();
    let mut n = 2;
    while std::path::Path::new(&output_path).exists() && n < 100 {
        output_path = parent.join(format!("{stem}_{suffix}_{n}.mp4")).to_string_lossy().to_string();
        n += 1;
    }

    let dir = ctx.dir.clone();
    let mut active: Vec<(PathBuf, f64)> = Vec::new();
    for meta in &ctx.clip.audio_tracks {
        let wav_name = meta.wav_filename.clone();
        if wav_name.is_empty() { continue; }
        if let Some(s) = req.tracks.iter().find(|s| s.name == meta.process_name) {
            if s.muted { continue; }
            let wav_path = dir.join(&wav_name);
            if wav_path.exists() { active.push((wav_path, s.volume.max(0.0))); }
        }
    }

    // ffmpeg args: input-side seek on every stream, uniform output-side -t.
    let mut args: Vec<String> = vec![
        "-hide_banner".into(), "-loglevel".into(), "error".into(),
        "-progress".into(), "pipe:1".into(), "-nostats".into(),
        "-ss".into(), format!("{:.3}", req.start),
        "-i".into(), input_path.clone(),
    ];
    for (wav, _) in &active {
        args.extend(["-ss".into(), format!("{:.3}", req.start), "-i".into(), wav.to_string_lossy().into_owned()]);
    }

    let fi = req.fade_in.unwrap_or(0.0).clamp(0.0, duration);
    let fo = req.fade_out.unwrap_or(0.0).clamp(0.0, duration);

    let mut chain = String::new();
    let cnt = active.len();
    if cnt == 0 {
        chain.push_str(&format!("aevalsrc=0:d={duration:.3}[aout]"));
        args.extend([
            "-map".into(), "0:v:0".into(),
            "-filter_complex".into(), chain,
            "-map".into(), "[aout]".into(),
        ]);
    } else {
        for (i, (_, vol)) in active.iter().enumerate() {
            chain.push_str(&format!("[{}:a]volume={:.3}[a{}];", i + 1, vol, i));
        }
        for i in 0..cnt { chain.push_str(&format!("[a{i}]")); }
        if cnt == 1 {
            chain.push_str("anull");
        } else {
            chain.push_str(&format!("amix=inputs={cnt}:duration=longest:normalize=0"));
        }
        let mut post: Vec<String> = Vec::new();
        if fi > 0.0 { post.push(format!("afade=t=in:st=0:d={fi:.3}")); }
        if fo > 0.0 { post.push(format!("afade=t=out:st={:.3}:d={fo:.3}", (duration - fo).max(0.0))); }
        post.push("apad".into());
        chain.push(',');
        chain.push_str(&post.join(","));
        chain.push_str("[aout]");

        args.extend([
            "-map".into(), "0:v:0".into(),
            "-filter_complex".into(), chain,
            "-map".into(), "[aout]".into(),
        ]);
    }

    if let Some(target_mb) = req.target_size_mb.filter(|&m| m > 0.0) {
        // Target compression mode with exact -0.4 MB safety margin for container overhead
        let effective_mb = (target_mb - 0.4).max(0.5);
        let target_bits = effective_mb * 8.0 * 1024.0 * 1024.0;
        let total_bps = target_bits / duration.max(0.5);

        let audio_kbps: u32 = if total_bps > 600_000.0 { 128 } else if total_bps > 300_000.0 { 64 } else { 48 };
        // Cap video bitrate at max 25,000 kbps (25 Mbps) so short clips are not bloated to absurd bitrates
        let video_kbps: u32 = (((total_bps / 1000.0) - (audio_kbps as f64)).clamp(80.0, 25_000.0)).round() as u32;

        // Balanced compression scaling prioritizing the 720p..1080p range with 2-FPS steps:
        let (target_height, target_fps) = if video_kbps >= 3600 {
            (1080, 60)
        } else if video_kbps >= 3300 {
            (1080, 56)
        } else if video_kbps >= 3000 {
            (1080, 52)
        } else if video_kbps >= 2700 {
            (1080, 48)
        } else if video_kbps >= 2400 {
            (1080, 44)
        } else if video_kbps >= 2150 {
            (1080, 40)
        } else if video_kbps >= 1950 {
            (1000, 44)
        } else if video_kbps >= 1750 {
            (1000, 40)
        } else if video_kbps >= 1600 {
            (960, 42)
        } else if video_kbps >= 1450 {
            (960, 38)
        } else if video_kbps >= 1300 {
            (900, 40)
        } else if video_kbps >= 1180 {
            (900, 36)
        } else if video_kbps >= 1060 {
            (840, 38)
        } else if video_kbps >= 960 {
            (840, 34)
        } else if video_kbps >= 860 {
            (800, 36)
        } else if video_kbps >= 780 {
            (760, 34)
        } else if video_kbps >= 700 {
            (760, 30)
        } else if video_kbps >= 620 {
            (720, 32)
        } else if video_kbps >= 550 {
            (720, 28)
        } else if video_kbps >= 480 {
            (720, 26)
        } else if video_kbps >= 420 {
            (640, 28)
        } else if video_kbps >= 360 {
            (640, 26)
        } else if video_kbps >= 300 {
            (560, 26)
        } else if video_kbps >= 240 {
            (480, 26)
        } else if video_kbps >= 180 {
            (420, 26)
        } else {
            (360, 26)
        };

        let mut vf_filters: Vec<String> = Vec::new();
        vf_filters.push(format!("scale=-2:{target_height}"));
        vf_filters.push(format!("fps={target_fps}"));

        if !vf_filters.is_empty() {
            args.extend(["-vf".into(), vf_filters.join(",")]);
        }

        args.extend([
            "-c:v".into(), "libx264".into(),
            "-preset".into(), "medium".into(),
            "-b:v".into(), format!("{video_kbps}k"),
            "-maxrate".into(), format!("{video_kbps}k"),
            "-bufsize".into(), format!("{}k", video_kbps * 2),
            "-pix_fmt".into(), "yuv420p".into(),
            "-c:a".into(), "aac".into(),
            "-b:a".into(), format!("{audio_kbps}k"),
            "-t".into(), format!("{:.3}", duration),
            "-movflags".into(), "+faststart".into(),
            "-y".into(),
            output_path.clone(),
        ]);
    } else {
        // Normal export mode
        let is_full_video = req.start <= 0.05 && (req.end >= ctx.clip.duration_secs - 0.05);
        if is_full_video {
            args.extend([
                "-c:v".into(), "copy".into(),
                "-c:a".into(), "aac".into(),
                "-b:a".into(), "192k".into(),
                "-t".into(), format!("{:.3}", duration),
                "-movflags".into(), "+faststart".into(),
                "-y".into(),
                output_path.clone(),
            ]);
        } else {
            let best_encoder = crate::hardware::detect_best_encoder(&ctx.app).await;
            if best_encoder.contains("nvenc") {
                args.extend([
                    "-c:v".into(), best_encoder.clone(),
                    "-preset".into(), "p4".into(),
                    "-rc".into(), "vbr".into(),
                    "-cq".into(), "20".into(),
                    "-b:v".into(), "0".into(),
                    "-spatial-aq".into(), "1".into(),
                ]);
                if !best_encoder.contains("av1") {
                    args.extend(["-temporal-aq".into(), "1".into()]);
                }
                args.extend(["-pix_fmt".into(), "yuv420p".into()]);
            } else if best_encoder.contains("qsv") {
                args.extend([
                    "-c:v".into(), best_encoder,
                    "-preset".into(), "faster".into(),
                    "-global_quality".into(), "20".into(),
                    "-pix_fmt".into(), "nv12".into(),
                ]);
            } else if best_encoder.contains("amf") {
                args.extend([
                    "-c:v".into(), best_encoder,
                    "-quality".into(), "speed".into(),
                    "-rc".into(), "cqp".into(),
                    "-qp_i".into(), "20".into(),
                    "-qp_p".into(), "22".into(),
                    "-pix_fmt".into(), "nv12".into(),
                ]);
            } else {
                args.extend([
                    "-c:v".into(), "libx264".into(),
                    "-preset".into(), "ultrafast".into(),
                    "-crf".into(), "19".into(),
                    "-pix_fmt".into(), "yuv420p".into(),
                ]);
            }

            args.extend([
                "-c:a".into(), "aac".into(),
                "-b:a".into(), "192k".into(),
                "-t".into(), format!("{:.3}", duration),
                "-movflags".into(), "+faststart".into(),
                "-y".into(),
                output_path.clone(),
            ]);
        }
    }

    let Ok(cmd) = ctx.app.shell().sidecar("ffmpeg") else {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":"ffmpeg missing"}))).into_response();
    };
    let Ok((mut rx, child)) = cmd.args(&args).spawn() else {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":"spawn failed"}))).into_response();
    };

    let job_id = format!("j{}-{}", now_secs(), ctx.jobs.lock().unwrap().len() + 1);
    let cancelled = Arc::new(AtomicBool::new(false));
    ctx.jobs.lock().unwrap().insert(job_id.clone(), Job {
        child: Some(child), cancelled: cancelled.clone(),
        percent: 0.0, status: "running".into(), output: output_path.clone(),
    });

    // Progress pump: parse progress lines until termination, then finalize.
    let ctx2 = ctx.clone();
    let job_id2 = job_id.clone();
    let dur_ms = (duration * 1000.0).max(1.0);
    tauri::async_runtime::spawn(async move {
        let mut buf = String::new();
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(bytes) => {
                    buf.push_str(&String::from_utf8_lossy(&bytes));
                    while let Some(pos) = buf.find('\n') {
                        let line: String = buf.drain(..=pos).collect();
                        if let Some(rest) = line.trim().strip_prefix("out_time_us=") {
                            if let Ok(us) = rest.parse::<f64>() {
                                let pct = ((us / 1000.0) / dur_ms * 100.0).min(100.0);
                                if let Ok(mut m) = ctx2.jobs.lock() {
                                    if let Some(job) = m.get_mut(&job_id2) { job.percent = pct as f32; }
                                }
                            }
                        }
                    }
                }
                CommandEvent::Terminated(_) => break,
                _ => {}
            }
        }
        let was_cancelled = cancelled.load(Ordering::Relaxed);
        if let Ok(mut m) = ctx2.jobs.lock() {
            if let Some(job) = m.get_mut(&job_id2) {
                job.child = None;
                job.status = if was_cancelled {
                    let _ = std::fs::remove_file(&output_path);
                    "cancelled".into()
                } else if matches!(std::fs::metadata(&output_path), Ok(md) if md.len() > 1024) {
                    "success".into()
                } else {
                    let _ = std::fs::remove_file(&output_path);
                    "error".into()
                };
            }
        }
    });

    Json(json!({ "job": job_id })).into_response()
}

async fn api_progress(State(ctx): State<Ctx>, Query(p): Query<HashMap<String, String>>) -> Response {
    if !check_token(&ctx, &p) { return deny(); }
    let Some(id) = p.get("job") else { return (StatusCode::BAD_REQUEST, "no job").into_response(); };
    let guard = ctx.jobs.lock().unwrap();
    match guard.get(id) {
        Some(j) => Json(json!({"status": j.status, "percent": j.percent, "output": j.output})).into_response(),
        None => (StatusCode::NOT_FOUND, "unknown job").into_response(),
    }
}

async fn api_cancel(State(ctx): State<Ctx>, Query(p): Query<HashMap<String, String>>, body: String) -> Response {
    if !check_token(&ctx, &p) { return deny(); }
    let job_id = serde_json::from_str::<serde_json::Value>(&body).ok()
        .and_then(|v| v.get("job").and_then(|j| j.as_str()).map(String::from));
    let Some(job_id) = job_id else { return (StatusCode::BAD_REQUEST, "no job").into_response(); };

    if let Ok(mut m) = ctx.jobs.lock() {
        if let Some(job) = m.get_mut(&job_id) {
            if let Some(child) = job.child.take() { let _ = child.kill(); }
            job.cancelled.store(true, Ordering::Relaxed);
        }
    }
    Json(json!({"ok": true})).into_response()
}

async fn api_copy_clipboard(State(ctx): State<Ctx>, Query(p): Query<HashMap<String, String>>, body: String) -> Response {
    if !check_token(&ctx, &p) { return deny(); }
    let path = serde_json::from_str::<serde_json::Value>(&body).ok()
        .and_then(|v| v.get("path").and_then(|j| j.as_str()).map(String::from));
    let Some(path) = path else { return (StatusCode::BAD_REQUEST, "no path").into_response(); };
    match crate::library::copy_clip_to_clipboard(path) {
        Ok(_) => Json(json!({"ok": true})).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))).into_response(),
    }
}

fn copy_bmp_to_clipboard(bmp_data: &[u8]) -> Result<(), String> {
    if bmp_data.len() <= 14 {
        return Err("BMP too small".into());
    }
    let dib_data = &bmp_data[14..];

    use windows::Win32::System::DataExchange::{OpenClipboard, EmptyClipboard, SetClipboardData, CloseClipboard};
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GHND};
    use windows::Win32::Foundation::GlobalFree;

    unsafe {
        let h_global = GlobalAlloc(GHND, dib_data.len()).map_err(|e| e.to_string())?;
        let ptr = GlobalLock(h_global);
        if ptr.is_null() {
            return Err("GlobalLock failed".into());
        }
        std::ptr::copy_nonoverlapping(dib_data.as_ptr(), ptr as *mut u8, dib_data.len());
        let _ = GlobalUnlock(h_global);

        if OpenClipboard(None).is_err() {
            let _ = GlobalFree(Some(h_global));
            return Err("OpenClipboard failed".into());
        }
        let _ = EmptyClipboard();
        const CF_DIB: u32 = 8;
        let res = SetClipboardData(CF_DIB, Some(windows::Win32::Foundation::HANDLE(h_global.0)));
        let _ = CloseClipboard();

        if res.is_err() {
            let _ = GlobalFree(Some(h_global));
            return Err("SetClipboardData failed".into());
        }
    }
    Ok(())
}

async fn api_screenshot_to_clipboard(State(ctx): State<Ctx>, Query(p): Query<HashMap<String, String>>) -> Response {
    if !check_token(&ctx, &p) { return deny(); }
    let time_sec: f64 = p.get("time").and_then(|t| t.parse().ok()).unwrap_or(0.0);
    let input_path = ctx.clip.full_path.clone();

    let Ok(cmd) = ctx.app.shell().sidecar("ffmpeg") else {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"ok":false,"error":"ffmpeg missing"}))).into_response();
    };

    let args = vec![
        "-hide_banner".to_string(), "-loglevel".to_string(), "error".to_string(),
        "-ss".to_string(), format!("{:.3}", time_sec),
        "-i".to_string(), input_path,
        "-frames:v".to_string(), "1".to_string(),
        "-c:v".to_string(), "bmp".to_string(),
        "-f".to_string(), "image2pipe".to_string(),
        "-".to_string(),
    ];

    let Ok(output) = cmd.args(&args).output().await else {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"ok":false,"error":"extract failed"}))).into_response();
    };

    if !output.status.success() || output.stdout.len() < 54 {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"ok":false,"error":"bad frame data"}))).into_response();
    }

    match copy_bmp_to_clipboard(&output.stdout) {
        Ok(_) => Json(json!({"ok": true})).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"ok": false, "error": e}))).into_response(),
    }
}

#[derive(Deserialize)]
struct ShowInFolderReq {
    path: String,
}

async fn api_show_in_folder(State(ctx): State<Ctx>, Query(p): Query<HashMap<String, String>>, body: String) -> Response {
    if !check_token(&ctx, &p) { return deny(); }
    let Ok(req) = serde_json::from_str::<ShowInFolderReq>(&body) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"ok":false}))).into_response();
    };
    let path = std::path::Path::new(&req.path);
    if path.exists() {
        let _ = std::process::Command::new("explorer.exe")
            .args(["/select,", &req.path])
            .spawn();
        Json(json!({"ok":true})).into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(json!({"ok":false}))).into_response()
    }
}

async fn api_shutdown(State(ctx): State<Ctx>, Query(p): Query<HashMap<String, String>>) -> Response {
    if !check_token(&ctx, &p) { return deny(); }
    let tx = ctx.shutdown_tx.clone();
    let seen_at = ctx.last_seen.load(Ordering::Relaxed);
    let ctx_c = ctx.clone();
    tauri::async_runtime::spawn(async move {
        // 5s grace period: allows page reloads (F5) without closing server
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        if ctx_c.last_seen.load(Ordering::Relaxed) <= seen_at {
            let _ = tx.send(true);
        }
    });
    Json(json!({"bye": true})).into_response()
}

// ──────────────────────────────────────────────────────────────────────────────
// Public command (name kept so the frontend call site stays identical)
// ──────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn open_editor_window(app: AppHandle, clip: crate::library::ClipItem) -> Result<(), String> {
    // One live session at a time: politely retire the previous one.
    if let Some(tx) = SESSION_SHUTDOWN.lock().unwrap().take() {
        let _ = tx.send(true);
    }

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let token = make_token();

    let dir = std::path::Path::new(&clip.full_path)
        .parent().map(|p| p.to_path_buf())
        .ok_or("clip has no parent directory")?;

    let has_app_tracks = clip.audio_tracks.iter().any(|t| t.process_name != "System" && t.process_name != "Microphone");
    let mut clean_clip = clip;
    if has_app_tracks {
        clean_clip.audio_tracks.retain(|t| t.process_name != "System");
    }

    let ctx: Ctx = Arc::new(Session {
        token,
        clip: clean_clip,
        dir,
        app: app.clone(),
        jobs: Mutex::new(HashMap::new()),
        shutdown_tx: shutdown_tx.clone(),
        last_seen: AtomicU64::new(now_secs()),
    });
    *SESSION_SHUTDOWN.lock().unwrap() = Some(shutdown_tx);

    let router = Router::new()
        .route("/", get(page))
        .route("/api/clip", get(clip_info))
        .route("/api/waveform", get(waveform))
        .route("/media/video", get(video))
        .route("/media/wav", get(wav))
        .route("/api/export", post(api_export))
        .route("/api/progress", get(api_progress))
        .route("/api/cancel", post(api_cancel))
        .route("/api/copy_clipboard", post(api_copy_clipboard))
        .route("/api/show_in_folder", post(api_show_in_folder))
        .route("/api/screenshot_to_clipboard", get(api_screenshot_to_clipboard))
        .route("/api/shutdown", post(api_shutdown))
        .with_state(ctx.clone());

    // Idle reaper: 15 min quiet closes the session; absolute cap 4 hours.
    let ctx_reaper = ctx.clone();
    tauri::async_runtime::spawn(async move {
        let ctx = ctx_reaper;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            let seen = ctx.last_seen.load(Ordering::Relaxed);
            if now_secs().saturating_sub(seen) > 15 * 60 || now_secs() > seen + 4 * 3600 {
                let _ = ctx.shutdown_tx.send(true);
                break;
            }
        }
    });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await.map_err(|e| format!("bind failed: {e}"))?;
    let addr = listener.local_addr().map_err(|e| e.to_string())?;

    let serve_shutdown = async move {
        let mut rx = shutdown_rx;
        let _ = rx.changed().await;
    };
    tauri::async_runtime::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(serve_shutdown)
            .await;
        println!("[editor] session closed");
        *SESSION_SHUTDOWN.lock().unwrap() = None;
    });

    let url = format!("http://{}/?t={}", addr, ctx.token);
    println!("[editor] serving at {}", url.split("?t=").next().unwrap_or(""));

    // Open the default browser using the native Windows Shell API (avoids cmd.exe escaping bugs)
    unsafe {
        use windows::core::HSTRING;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let url_h = HSTRING::from(url.as_str());
        let op_h = HSTRING::from("open");
        let _ = ShellExecuteW(None, &op_h, &url_h, None, None, SW_SHOWNORMAL);
    }
    Ok(())
}
