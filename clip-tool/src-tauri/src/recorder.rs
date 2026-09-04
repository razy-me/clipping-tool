use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::CommandChild;
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use std::fs;

use chrono::Local;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::JobObjects::{
    CreateJobObjectW, SetInformationJobObject, AssignProcessToJobObject,
    JobObjectExtendedLimitInformation, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, PROCESS_SET_QUOTA};

use crate::audio_engine::{start_audio_capture, stop_audio_capture, dump_audio_clips};
use crate::hardware::detect_best_encoder;

// ──────────────────────────────────────────────────────────────────────────────
// Job object: guarantees bundled ffmpeg dies even if this app crashes.
// ──────────────────────────────────────────────────────────────────────────────
static GLOBAL_JOB: Mutex<Option<usize>> = Mutex::new(None);

pub fn assign_pid_to_job(pid: u32) {
    unsafe {
        let mut global_job = GLOBAL_JOB.lock().unwrap();
        if global_job.is_none() {
            if let Ok(job_handle) = CreateJobObjectW(None, None) {
                let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let _ = SetInformationJobObject(
                    job_handle,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const _,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                *global_job = Some(job_handle.0 as usize);
            }
        }

        if let Some(job_addr) = *global_job {
            let job_handle = HANDLE(job_addr as *mut _);
            if let Ok(proc_handle) = OpenProcess(PROCESS_TERMINATE | PROCESS_SET_QUOTA, false, pid) {
                let _ = AssignProcessToJobObject(job_handle, proc_handle);
                let _ = windows::Win32::Foundation::CloseHandle(proc_handle);
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// State machine surfaced to the UI as buffer://state events.
// ──────────────────────────────────────────────────────────────────────────────
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BufferState { Stopped, Starting, Active, Paused, Error }

impl BufferState {
    fn as_str(self) -> &'static str {
        match self { BufferState::Stopped => "stopped", BufferState::Starting => "starting",
                     BufferState::Active => "active",   BufferState::Paused => "paused",
                     BufferState::Error => "error" }
    }
}

static BUFFER_STATE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

fn set_state(s: BufferState) {
    BUFFER_STATE.store(match s {
        BufferState::Stopped => 0, BufferState::Starting => 1,
        BufferState::Active => 2,  BufferState::Paused => 3,
        BufferState::Error => 4,
    }, std::sync::atomic::Ordering::SeqCst);
}

fn current_state() -> BufferState {
    match BUFFER_STATE.load(std::sync::atomic::Ordering::SeqCst) {
        1 => BufferState::Starting, 2 => BufferState::Active, 3 => BufferState::Paused, 4 => BufferState::Error, _ => BufferState::Stopped,
    }
}

static LAST_DIAGNOSTIC_LOG: Mutex<String> = Mutex::new(String::new());

pub fn update_diagnostic_log(log: String) {
    println!("\n[DIAGNOSTIC LOG UPDATED]\n{}\n", log);
    if let Ok(mut lock) = LAST_DIAGNOSTIC_LOG.lock() {
        *lock = log.clone();
    }
    // Also write to last_error_report.txt in current working directory
    if let Ok(dir) = std::env::current_dir() {
        let _ = std::fs::write(dir.join("last_error_report.txt"), &log);
    }
}

#[tauri::command]
pub fn get_last_error_log() -> String {
    LAST_DIAGNOSTIC_LOG.lock().map(|g| g.clone()).unwrap_or_else(|_| "Kein Fehlerbericht verfügbar".into())
}

pub fn emit_buffer_state(app: &AppHandle, state: BufferState, detail: Option<&str>) {
    set_state(state);
    let _ = app.emit("buffer://state", serde_json::json!({
        "state": state.as_str(),
        "detail": detail,
    }));
}

/// Generation counter: bumped on every manual start/stop so stale watchdog
/// futures from a previous session never touch the new one.
static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static AUTO_RESTARTS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub struct RecorderState {
    pub child: Mutex<Option<Arc<Mutex<Option<CommandChild>>>>>,
}

// ──────────────────────────────────────────────────────────────────────────────
// In-Memory RAM Video Ring-Buffer (Eliminates SSD write wear)
// ──────────────────────────────────────────────────────────────────────────────
pub fn find_first_ts_keyframe_offset(bytes: &[u8]) -> Option<usize> {
    let mut i = 0;
    while i + 188 <= bytes.len() {
        if bytes[i] == 0x47 {
            let pusi = (bytes[i + 1] & 0x40) != 0;
            let adaptation_control = (bytes[i + 3] >> 4) & 0x03;
            let mut payload_offset = i + 4;
            let mut has_rai = false;

            if adaptation_control == 2 || adaptation_control == 3 {
                let adapt_len = bytes[i + 4] as usize;
                if adapt_len >= 1 && i + 5 < bytes.len() {
                    // Bit 6 in adaptation field flags is random_access_indicator (RAI)
                    has_rai = (bytes[i + 5] & 0x40) != 0;
                }
                payload_offset = i + 5 + adapt_len;
            }

            if pusi {
                // Check 1: MPEG-TS Adaptation Field Random Access Indicator (FFmpeg sets this on keyframes)
                if has_rai {
                    return Some(i);
                }

                // Check 2: Parse PES elementary stream headers for H.264, HEVC and AV1
                if payload_offset + 4 <= i + 188 && payload_offset + 4 <= bytes.len() {
                    // Check for PES start code 0x00 0x00 0x01
                    if bytes[payload_offset] == 0x00 && bytes[payload_offset + 1] == 0x00 && bytes[payload_offset + 2] == 0x01 {
                        let end = (i + 188).min(bytes.len());
                        // If this is a valid video PES packet (0xE0..=0xEF, 0xBD, 0xFD), skip the 9+ header bytes
                        let stream_id = bytes[payload_offset + 3];
                        let es_start = if ((0xE0..=0xEF).contains(&stream_id) || stream_id == 0xBD || stream_id == 0xFD) && payload_offset + 9 <= bytes.len() {
                            let pes_hdr_data_len = bytes[payload_offset + 8] as usize;
                            (payload_offset + 9 + pes_hdr_data_len).min(end)
                        } else {
                            payload_offset
                        };

                        let slice = &bytes[es_start..end];
                        let mut j = 0;
                        while j + 3 < slice.len() {
                            // H.264/H.265/AV1 start code: 0x00 0x00 0x01 (3-byte) or 0x00 0x00 0x00 0x01 (4-byte)
                            let nal_byte_opt = if slice[j] == 0 && slice[j + 1] == 0 && slice[j + 2] == 1 {
                                Some(slice[j + 3])
                            } else if j + 4 < slice.len() && slice[j] == 0 && slice[j + 1] == 0 && slice[j + 2] == 0 && slice[j + 3] == 1 {
                                Some(slice[j + 4])
                            } else {
                                None
                            };

                            if let Some(byte) = nal_byte_opt {
                                if (byte & 0x80) == 0 {
                                    // H.264: IDR(5), SPS(7), PPS(8)
                                    let h264_type = byte & 0x1F;
                                    let is_h264_key = h264_type == 5 || h264_type == 7 || h264_type == 8;

                                    // H.265 (HEVC): base layer (bit 0 == 0) IDR(19,20), CRA(21), VPS(32), SPS(33), PPS(34)
                                    let hevc_type = (byte >> 1) & 0x3F;
                                    let is_hevc_key = (byte & 1) == 0 && (hevc_type == 19 || hevc_type == 20 || hevc_type == 21 || hevc_type == 32 || hevc_type == 33 || hevc_type == 34);

                                    // AV1: OBU Sequence Header (obu_type == 1, reserved bit 0 == 0)
                                    let obu_type = (byte >> 3) & 0x0F;
                                    let is_av1_key = (byte & 1) == 0 && obu_type == 1;

                                    if is_h264_key || is_hevc_key || is_av1_key {
                                        return Some(i);
                                    }
                                }
                            }
                            j += 1;
                        }
                    }
                }
            }
            i += 188;
        } else {
            i += 1;
        }
    }
    None
}

pub fn is_ts_keyframe(bytes: &[u8]) -> bool {
    find_first_ts_keyframe_offset(bytes).is_some()
}

pub fn extract_ts_pts(bytes: &[u8]) -> Option<u64> {
    let mut i = 0;
    while i + 188 <= bytes.len() {
        if bytes[i] == 0x47 {
            let pusi = (bytes[i + 1] & 0x40) != 0;
            let adaptation_control = (bytes[i + 3] >> 4) & 0x03;
            let mut payload_offset = i + 4;

            if adaptation_control == 2 || adaptation_control == 3 {
                let adapt_len = bytes[i + 4] as usize;
                payload_offset = i + 5 + adapt_len;
            }

            if pusi && payload_offset + 14 <= i + 188 && payload_offset + 14 <= bytes.len() {
                // Check for PES start code 0x00 0x00 0x01
                if bytes[payload_offset] == 0x00 && bytes[payload_offset + 1] == 0x00 && bytes[payload_offset + 2] == 0x01 {
                    let stream_id = bytes[payload_offset + 3];
                    // Video stream IDs are 0xE0..0xEF, 0xBD, 0xFD
                    if (0xE0..=0xEF).contains(&stream_id) || stream_id == 0xBD || stream_id == 0xFD {
                        let pts_dts_flags = (bytes[payload_offset + 7] >> 6) & 0x03;
                        if pts_dts_flags == 2 || pts_dts_flags == 3 {
                            let b0 = bytes[payload_offset + 9] as u64;
                            let b1 = bytes[payload_offset + 10] as u64;
                            let b2 = bytes[payload_offset + 11] as u64;
                            let b3 = bytes[payload_offset + 12] as u64;
                            let b4 = bytes[payload_offset + 13] as u64;

                            let pts: u64 = ((b0 & 0x0E) << 29)
                                | ((b1 & 0xFF) << 22)
                                | ((b2 & 0xFE) << 14)
                                | ((b3 & 0xFF) << 7)
                                | ((b4 & 0xFE) >> 1);
                            return Some(pts);
                        }
                    }
                }
            }
            i += 188;
        } else {
            i += 1;
        }
    }
    None
}

#[derive(Clone, Debug)]
pub struct ExtractedVideo {
    pub data: Vec<u8>,
    pub start_ts: std::time::Instant,
    pub end_ts: std::time::Instant,
    pub duration_secs: f64,
}

#[derive(Clone)]
pub struct TsChunk {
    pub data: Vec<u8>,
    pub timestamp: std::time::Instant,
    pub is_keyframe: bool,
}

pub struct InMemoryVideoBuffer {
    pub chunks: std::collections::VecDeque<TsChunk>,
    pub max_bytes: usize,
    pub current_bytes: usize,
}

impl InMemoryVideoBuffer {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            chunks: std::collections::VecDeque::with_capacity(4096),
            max_bytes,
            current_bytes: 0,
        }
    }

    pub fn push_chunk(&mut self, data: Vec<u8>) {
        let len = data.len();
        if len == 0 || len > self.max_bytes { return; }

        while self.current_bytes + len > self.max_bytes {
            if let Some(front) = self.chunks.pop_front() {
                self.current_bytes = self.current_bytes.saturating_sub(front.data.len());
            } else {
                break;
            }
        }

        let is_key = is_ts_keyframe(&data);
        let now = std::time::Instant::now();

        self.current_bytes += len;
        self.chunks.push_back(TsChunk {
            data,
            timestamp: now,
            is_keyframe: is_key,
        });
    }

    pub fn push_bytes(&mut self, bytes: &[u8]) {
        self.push_chunk(bytes.to_vec());
    }

    pub fn extract(&self, target_secs: f64) -> ExtractedVideo {
        let now = std::time::Instant::now();
        if self.chunks.is_empty() {
            return ExtractedVideo {
                data: Vec::new(),
                start_ts: now,
                end_ts: now,
                duration_secs: 0.0,
            };
        }

        let earliest_ts = self.chunks.front().map(|c| c.timestamp).unwrap_or(now);
        let last_ts = self.chunks.back().map(|c| c.timestamp).unwrap_or(now);
        let safe_secs = if target_secs.is_nan() || target_secs < 0.0 { 0.0 } else { target_secs };
        let target_start_ts = last_ts.checked_sub(std::time::Duration::from_secs_f64(safe_secs)).unwrap_or(earliest_ts);

        let raw_start_idx = if target_start_ts <= earliest_ts {
            0
        } else {
            let mut best_idx = 0;
            for (idx, chunk) in self.chunks.iter().enumerate() {
                if chunk.is_keyframe {
                    if chunk.timestamp <= target_start_ts {
                        best_idx = idx;
                    } else {
                        break;
                    }
                }
            }
            best_idx
        };

        // Ensure start_idx lands on a keyframe so the remuxed video starts with a clean IDR frame
        let start_idx = if self.chunks[raw_start_idx].is_keyframe {
            raw_start_idx
        } else {
            self.chunks.iter().enumerate().skip(raw_start_idx).find(|(_, c)| c.is_keyframe).map(|(idx, _)| idx)
                .or_else(|| self.chunks.iter().enumerate().take(raw_start_idx).rfind(|(_, c)| c.is_keyframe).map(|(idx, _)| idx))
                .unwrap_or(raw_start_idx)
        };

        let first_chunk = &self.chunks[start_idx];
        let first_ts = first_chunk.timestamp;
        let actual_end_ts = last_ts.max(first_ts);
        let dur = if actual_end_ts >= first_ts {
            actual_end_ts.duration_since(first_ts).as_secs_f64().max(0.01)
        } else {
            0.01
        };

        let total_size: usize = self.chunks.iter().skip(start_idx).map(|c| c.data.len()).sum::<usize>();

        let mut out = Vec::with_capacity(total_size);
        for chunk in self.chunks.iter().skip(start_idx) {
            out.extend_from_slice(&chunk.data);
        }

        ExtractedVideo {
            data: out,
            start_ts: first_ts,
            end_ts: actual_end_ts,
            duration_secs: dur,
        }
    }
}

static VIDEO_BUFFER: Mutex<Option<InMemoryVideoBuffer>> = Mutex::new(None);

pub fn init_video_buffer(buffer_length_secs: u32, bitrate_str: &str, _tier: crate::hardware_profile::HardwareTier) {
    let num_k = bitrate_str.strip_suffix('k').or_else(|| bitrate_str.strip_suffix('K')).unwrap_or(bitrate_str);
    let kbps: usize = num_k.parse().unwrap_or(20000);
    let requested_bytes = ((kbps * 1000 / 8) * (buffer_length_secs as usize + 30)).max(64 * 1024 * 1024);
    
    let max_bytes = requested_bytes;

    let mut guard = VIDEO_BUFFER.lock().unwrap();
    *guard = Some(InMemoryVideoBuffer::new(max_bytes));
}

pub fn push_video_chunk(data: Vec<u8>) {
    if let Ok(mut guard) = VIDEO_BUFFER.lock() {
        if let Some(buf) = guard.as_mut() {
            buf.push_chunk(data);
        }
    }
}

pub fn get_video_buffer_stats() -> (usize, usize, f64, usize) {
    if let Ok(guard) = VIDEO_BUFFER.lock() {
        if let Some(buf) = guard.as_ref() {
            let dur = if let (Some(first), Some(last)) = (buf.chunks.front(), buf.chunks.back()) {
                if last.timestamp >= first.timestamp {
                    last.timestamp.duration_since(first.timestamp).as_secs_f64()
                } else {
                    0.0
                }
            } else {
                0.0
            };
            return (buf.current_bytes, buf.max_bytes, dur, buf.chunks.len());
        }
    }
    (0, 0, 0.0, 0)
}

pub fn is_saving() -> bool {
    SAVE_IN_FLIGHT.load(std::sync::atomic::Ordering::SeqCst)
}

// ──────────────────────────────────────────────────────────────────────────────
// Argument construction (pure, validated).
fn bitrate_from_preset(preset: &crate::config::BitratePreset) -> &'static str {
    match preset {
        crate::config::BitratePreset::Low => "6000k",
        crate::config::BitratePreset::Balanced => "12000k",
        crate::config::BitratePreset::High => "20000k",
        crate::config::BitratePreset::Ultra => "35000k",
    }
}

fn res_height(res: &str) -> Option<u32> {
    if res.eq_ignore_ascii_case("original") {
        return None;
    }
    if let Some(h_str) = res.strip_suffix('p').or_else(|| res.strip_suffix('P')) {
        if let Ok(h) = h_str.parse::<u32>() {
            if (144..=4320).contains(&h) {
                return Some(h);
            }
        }
    }
    match res.to_lowercase().as_str() {
        "1440p" => Some(1440),
        "1080p" => Some(1080),
        "900p" => Some(900),
        "720p" => Some(720),
        "540p" => Some(540),
        "480p" => Some(480),
        "360p" => Some(360),
        "240p" => Some(240),
        _ => None,
    }
}

pub fn extract_video_buffer(requested_duration_secs: f64) -> Result<ExtractedVideo, String> {
    let mut guard = VIDEO_BUFFER.lock().unwrap();
    if let Some(buf) = guard.as_mut() {
        Ok(buf.extract(requested_duration_secs))
    } else {
        Err("Video buffer not initialized".into())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Save-in-flight guard (prevents concurrent hotkey saves racing temp files).
// ──────────────────────────────────────────────────────────────────────────────
static SAVE_IN_FLIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

struct SaveGuard;
impl Drop for SaveGuard {
    fn drop(&mut self) {
        SAVE_IN_FLIGHT.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

pub fn capture_graph(
    encoder: &str,
    fps: &str,
    resolution: &str,
    monitor_h: u32,
    draw_mouse: bool,
    _scaling_method: &crate::hardware::ScalingMethod,
    monitor_idx: u32,
    hdr_tonemapping: bool,
) -> String {
    let mouse = if draw_mouse { "true" } else { "false" };
    let ddagrab = format!("ddagrab=output_idx={monitor_idx}:framerate={fps}:draw_mouse={mouse}:dup_frames=true");
    let h = res_height(resolution).unwrap_or(monitor_h);
    let needs_scale = h < monitor_h;

    let target_format = if encoder.contains("nvenc") || encoder.contains("qsv") || encoder.contains("amf") {
        "nv12"
    } else {
        "yuv420p"
    };

    if hdr_tonemapping {
        if needs_scale {
            format!("{ddagrab},hwdownload,format=bgra,tonemap=tonemap=hable:desat=0,scale=-2:{h}:flags=fast_bilinear,format={target_format}")
        } else {
            format!("{ddagrab},hwdownload,format=bgra,tonemap=tonemap=hable:desat=0,format={target_format}")
        }
    } else if needs_scale {
        format!("{ddagrab},hwdownload,format=bgra,scale=-2:{h}:flags=fast_bilinear,format={target_format}")
    } else {
        format!("{ddagrab},hwdownload,format=bgra,format={target_format}")
    }
}

fn build_ffmpeg_args(
    encoder: &str,
    fps: &str,
    resolution: &str,
    bitrate: &str,
    monitor_h: u32,
    _tier: crate::hardware_profile::HardwareTier,
    draw_mouse: bool,
    scaling_method: &crate::hardware::ScalingMethod,
    monitor_idx: u32,
    hdr_tonemapping: bool,
) -> Result<Vec<String>, String> {
    let fps_num = fps.parse::<u32>().map_err(|_| format!("Invalid FPS value: '{fps}'. Please enter a number."))?;
    if !(10..=360).contains(&fps_num) {
        return Err(format!("FPS out of range (10-360): {fps_num}"));
    }
    if !resolution.eq_ignore_ascii_case("original") && res_height(resolution).is_none() {
        return Err(format!("Unsupported resolution: {resolution}"));
    }
    let num = bitrate.strip_suffix('k').or_else(|| bitrate.strip_suffix('K')).unwrap_or(&bitrate);
    if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("Invalid bitrate: {bitrate}"));
    }

    let graph = capture_graph(encoder, fps, resolution, monitor_h, draw_mouse, scaling_method, monitor_idx, hdr_tonemapping);
    // Smooth keyframe interval: 2.0 seconds (e.g. 80 frames @ 40fps, 120 frames @ 60fps).
    // Eliminates repetitive IDR burst spikes every 500ms while keeping clip start keyframe alignment perfectly within standard limits.
    let gop_size = (fps_num * 2).max(30);

    let mut args: Vec<String> = vec![
        "-hide_banner".into(), "-loglevel".into(), "warning".into(),
        "-sws_flags".into(), "fast_bilinear".into(),
        "-filter_threads".into(), "1".into(),
        "-filter_complex".into(), graph,
        "-c:v".into(), encoder.into(),
        "-g".into(), gop_size.to_string(),
        "-fps_mode".into(), "passthrough".into(),
    ];

    match encoder {
        "libx264" | "libx265" => {
            args.extend([
                "-preset".into(), "ultrafast".into(),
                "-tune".into(), "zerolatency".into(),
                "-pix_fmt".into(), "yuv420p".into(),
                "-b:v".into(), bitrate.into(),
                "-maxrate".into(), bitrate.into(),
                "-bufsize".into(), bitrate.into(),
                "-keyint_min".into(), gop_size.to_string(),
                "-sc_threshold".into(), "0".into(),
                "-bf".into(), "0".into(),
            ]);
            let threads = (std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(4) / 2).max(1);
            args.extend(["-threads".into(), threads.to_string()]);
        }
        "libsvtav1" => {
            args.extend([
                "-preset".into(), "10".into(),
                "-pix_fmt".into(), "yuv420p".into(),
                "-b:v".into(), bitrate.into(),
                "-maxrate".into(), bitrate.into(),
                "-bufsize".into(), bitrate.into(),
                "-keyint_min".into(), gop_size.to_string(),
            ]);
            let threads = (std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(4) / 2).max(1);
            args.extend(["-threads".into(), threads.to_string()]);
        }
        "h264_nvenc" | "hevc_nvenc" | "av1_nvenc" => { 
            // Pure fixed-function NVENC ASIC execution.
            // -preset p2 (fast) with -tune ull (ultra low latency) runs 100% on the dedicated NVENC silicon
            // without launching CUDA compute kernels on GPU SMs (unlike -spatial-aq 1 which steals game render cores).
            args.extend([
                "-preset".into(), "p2".into(),
                "-tune".into(), "ull".into(),
                "-rc".into(), "vbr".into(),
                "-cq".into(), "20".into(),
                "-b:v".into(), bitrate.into(),
                "-maxrate".into(), bitrate.into(),
                "-bufsize".into(), bitrate.into(),
                "-delay".into(), "0".into(),
                "-forced-idr".into(), "1".into(),
                "-zerolatency".into(), "1".into(),
                "-bf".into(), "0".into(),
            ]); 
        }
        "av1_qsv" => {
            // Intel QuickSync AV1: -preset veryfast with leaky-bucket rate control.
            // Clean option set avoiding H.264/HEVC-specific private options (idr_interval, look_ahead).
            args.extend([
                "-preset".into(), "veryfast".into(),
                "-b:v".into(), bitrate.into(),
                "-maxrate".into(), bitrate.into(),
                "-bufsize".into(), bitrate.into(),
                "-async_depth".into(), "4".into(),
                "-bf".into(), "0".into(),
            ]);
        }
        "h264_qsv" | "hevc_qsv" => { 
            // Intel QuickSync Video: -preset veryfast saves unified memory bandwidth.
            // -async_depth 4 allows QSV to pipeline asynchronously without stalling 3D graphics presentation.
            // -look_ahead 0 eliminates lookahead RAM buffer allocations in Intel iGPU/Xe2 unified memory.
            // -bufsize bitrate clamps sudden bit bursts to smooth out CPU and GPU spikes.
            args.extend([
                "-preset".into(), "veryfast".into(),
                "-b:v".into(), bitrate.into(),
                "-maxrate".into(), bitrate.into(),
                "-bufsize".into(), bitrate.into(),
                "-idr_interval".into(), "1".into(),
                "-async_depth".into(), "4".into(),
                "-look_ahead".into(), "0".into(),
                "-bf".into(), "0".into(),
            ]); 
        }
        "h264_amf" | "hevc_amf" | "av1_amf" => { 
            // AMD Advanced Media Framework (VCN ASIC): -quality speed and lowlatency.
            args.extend([
                "-quality".into(), "speed".into(),
                "-usage".into(), "lowlatency".into(),
                "-rc".into(), "cqp".into(),
                "-qp_i".into(), "22".into(),
                "-qp_p".into(), "24".into(),
                "-b:v".into(), bitrate.into(),
                "-maxrate".into(), bitrate.into(),
                "-bufsize".into(), bitrate.into(),
                "-header_insertion_mode".into(), "idr".into(),
                "-bf".into(), "0".into(),
            ]); 
        }
        _ => {
            args.extend([
                "-b:v".into(), bitrate.into(),
                "-maxrate".into(), bitrate.into(),
                "-bufsize".into(), bitrate.into(),
            ]);
        }
    }

    args.extend([
        "-f".into(), "mpegts".into(),
        "-muxdelay".into(), "0".into(),
        "-flush_packets".into(), "1".into(),
        "-max_muxing_queue_size".into(), "2048".into(),
        "-mpegts_flags".into(), "+resend_headers".into(),
        "-pat_period".into(), "0.1".into(),
        "-sdt_period".into(), "0.1".into(),
        "-y".into(),
        "pipe:1".into(),
    ]);

    Ok(args)
}

// ──────────────────────────────────────────────────────────────────────────────
// Pipeline lifecycle
// ──────────────────────────────────────────────────────────────────────────────
fn restart_after_delay(
    app: AppHandle,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
    Box::pin(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
        let _ = start_pipeline(app).await;
    })
}

async fn start_pipeline(app: AppHandle) -> Result<(), String> {
    let gen = GENERATION.load(std::sync::atomic::Ordering::SeqCst);
    emit_buffer_state(&app, BufferState::Starting, None);

    let state = app.state::<RecorderState>();
    let had_child = {
        let mut guard = state.child.lock().unwrap();
        if let Some(child_arc) = guard.take() {
            if let Ok(mut opt) = child_arc.lock() {
                if let Some(child) = opt.take() {
                    let _ = child.kill();
                }
            }
            true
        } else {
            false
        }
    };
    if had_child {
        // Grace period for DirectX DXGI output handle cleanup
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    stop_audio_capture();

    let cfg = crate::config::get_config(app.clone());
    let bitrate = bitrate_from_preset(&cfg.bitrate_preset).to_string();
    let prof = crate::hardware_profile::detect_hardware_profile(&app).await;

    init_video_buffer(cfg.buffer_length_secs, &bitrate, prof.tier);

    crate::audio_engine::set_mic_volume(cfg.mic_volume);
    if let Err(e) = start_audio_capture(
        cfg.buffer_length_secs,
        cfg.spike_detection_enabled,
        cfg.spike_threshold,
    ) {
        eprintln!("[buffer] audio capture failed: {e}");
    }

    let encoder = detect_best_encoder(&app).await;
    let scaling_method = crate::hardware::detect_scaling_method(&app, &encoder).await;
    let monitors = crate::wgc_recorder::get_available_monitors();
    let target_mon = monitors.iter().find(|m| m.index == cfg.monitor_idx);
    let effective_monitor_idx = if target_mon.is_some() { cfg.monitor_idx } else { 0 };
    let (monitor_w, monitor_h) = target_mon
        .map(|m| (m.width, m.height))
        .unwrap_or_else(crate::wgc_recorder::get_primary_monitor_size);

    let diag_header = format!(
        "=== CLIP TOOL BUFFER DIAGNOSE-BERICHT ===\nZeitpunkt: {}\nGewählter Codec: {:?}\nErkannter Encoder: {}\nScaling Methode: {:?}\nAuflösung: {} (Monitor: {}x{})\nBitrate: {}\nFPS: {}\nHardware Tier: {:?} (CPUs: {})\nMikrofon: {} (Vol: {})\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        cfg.video_codec,
        encoder,
        scaling_method,
        cfg.video_resolution,
        monitor_w, monitor_h,
        bitrate,
        cfg.fps_selection,
        prof.tier,
        prof.logical_cores,
        crate::audio_engine::get_active_mic_device(),
        cfg.mic_volume,
    );

    let args = match build_ffmpeg_args(&encoder, &cfg.fps_selection, &cfg.video_resolution, &bitrate, monitor_h, prof.tier, cfg.show_cursor_in_clips, &scaling_method, effective_monitor_idx, cfg.hdr_tonemapping) {
        Ok(a) => a,
        Err(e) => {
            let full_report = format!("{}\nFEHLER BEIM ERSTELLEN DER ARGS:\n{}\n========================================", diag_header, e);
            update_diagnostic_log(full_report);
            emit_buffer_state(&app, BufferState::Error, Some(&e));
            return Err(e);
        }
    };

    let full_cmd_str = format!("ffmpeg {}", args.join(" "));

    let cmd = match app.shell().sidecar("ffmpeg").map_err(|e| e.to_string()) {
        Ok(c) => c.args(&args),
        Err(e) => {
            let full_report = format!("{}\nFEHLER BEIM SIDECAR-LADEN:\n{}\n========================================", diag_header, e);
            update_diagnostic_log(full_report);
            emit_buffer_state(&app, BufferState::Error, Some(&e));
            return Err(e);
        }
    };

    let (mut rx, child) = match cmd.spawn().map_err(|e| e.to_string()) {
        Ok(pair) => pair,
        Err(e) => {
            let full_report = format!("{}\nFFmpeg Befehl:\n{}\n\nFEHLER BEIM PROZESS-START (spawn):\n{}\n========================================", diag_header, full_cmd_str, e);
            update_diagnostic_log(full_report);
            emit_buffer_state(&app, BufferState::Error, Some(&e));
            return Err(e);
        }
    };
    assign_pid_to_job(child.pid());

    // Lower FFmpeg process priority to BELOW_NORMAL_PRIORITY_CLASS.
    // This gives active games, user input, and Windows DWM 100% CPU scheduling priority,
    // completely eliminating CPU-contention micro-stutters in demanding games.
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::System::Threading::{
            OpenProcess, SetPriorityClass, PROCESS_SET_INFORMATION, BELOW_NORMAL_PRIORITY_CLASS,
        };
        if let Ok(handle) = OpenProcess(PROCESS_SET_INFORMATION, false, child.pid()) {
            let _ = SetPriorityClass(handle, BELOW_NORMAL_PRIORITY_CLASS);
            let _ = windows::Win32::Foundation::CloseHandle(handle);
        }
    }

    unsafe {
        use windows::Win32::System::Power::{
            SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED, ES_DISPLAY_REQUIRED
        };

        // Sleep Guard: prevent Windows display/system sleep during active recording buffer
        let _ = SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED);
    }

    let child_arc = std::sync::Arc::new(Mutex::new(Some(child)));
    *state.child.lock().unwrap() = Some(child_arc.clone());

    // Drain output & collect video bytes directly into RAM buffer
    let app_c = app.clone();
    let diag_header_watchdog = diag_header.clone();
    let full_cmd_watchdog = full_cmd_str.clone();

    tauri::async_runtime::spawn(async move {
        let mut saw_termination = false;
        let mut term_code: Option<i32> = None;
        let mut term_signal: Option<i32> = None;
        let mut err_tail: std::collections::VecDeque<String> = std::collections::VecDeque::new();
        let started_at = std::time::Instant::now();
        let mut chunk_accumulator: Vec<u8> = Vec::with_capacity(64 * 1024);
        let mut last_flush = std::time::Instant::now();

        while let Some(event) = rx.recv().await {
            match event {
                tauri_plugin_shell::process::CommandEvent::Stdout(bytes) => {
                    // Reset auto-restart counter if pipeline has been recording stably for 5s
                    if started_at.elapsed() >= std::time::Duration::from_secs(5) {
                        AUTO_RESTARTS.store(0, std::sync::atomic::Ordering::Relaxed);
                    }
                    chunk_accumulator.extend_from_slice(&bytes);
                    // Batch chunks into 64 KB or flush every 50ms to cut mutex locks & heap allocations by 98%
                    if chunk_accumulator.len() >= 64 * 1024 || last_flush.elapsed() >= std::time::Duration::from_millis(50) {
                        let to_push = std::mem::take(&mut chunk_accumulator);
                        push_video_chunk(to_push);
                        last_flush = std::time::Instant::now();
                    }
                }
                tauri_plugin_shell::process::CommandEvent::Terminated(payload) => {
                    if !chunk_accumulator.is_empty() {
                        push_video_chunk(std::mem::take(&mut chunk_accumulator));
                    }
                    term_code = payload.code;
                    term_signal = payload.signal;
                    println!("[buffer] ffmpeg terminated: code={:?} signal={:?}", payload.code, payload.signal);
                    saw_termination = true;
                    break;
                }
                tauri_plugin_shell::process::CommandEvent::Stderr(line) => {
                    let s = String::from_utf8_lossy(&line).into_owned();
                    if err_tail.len() >= 40 { err_tail.pop_front(); }
                    err_tail.push_back(s);
                }
                _ => {}
            }
        }

        let mut report = diag_header_watchdog;
        report.push_str(&format!("\nFFmpeg Befehl:\n{}\n", full_cmd_watchdog));
        if saw_termination {
            report.push_str(&format!("\nStatus: Beendet (Exit Code: {:?}, Signal: {:?})\n", term_code, term_signal));
        }
        report.push_str("\n--- FFmpeg Konsolenausgabe (stderr) ---\n");
        if err_tail.is_empty() {
            report.push_str("(Keine Fehlermeldung auf stderr ausgegeben)\n");
        } else {
            for line in &err_tail {
                report.push_str(&format!("  {}\n", line));
            }
        }
        report.push_str("========================================");
        update_diagnostic_log(report);

        if saw_termination && !err_tail.is_empty() {
            println!("[buffer] last ffmpeg output before death:");
            for line in &err_tail {
                println!("  [ffmpeg] {line}");
            }
        }
        if GENERATION.load(std::sync::atomic::Ordering::SeqCst) != gen {
            return; // stale watchdog from an older session
        }
        if current_state() == BufferState::Active || current_state() == BufferState::Starting {
            let st = app_c.state::<RecorderState>();
            if let Some(arc) = st.child.lock().unwrap().take() {
                if let Ok(mut opt) = arc.lock() { if let Some(c) = opt.take() { let _ = c.kill(); } }
            }
            stop_audio_capture();

            let last_meaningful_err = err_tail.iter().rev()
                .find(|l| !l.trim().is_empty())
                .cloned()
                .unwrap_or_else(|| format!("FFmpeg beendet (Exit Code: {:?})", term_code));

            let attempts = AUTO_RESTARTS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if attempts < 3 {
                println!("[buffer] auto-restarting pipeline (attempt {} of 3)", attempts + 1);
                emit_buffer_state(&app_c, BufferState::Starting, Some("Wiederverbinde Puffer..."));
                let a2 = app_c.clone();
                tauri::async_runtime::spawn(restart_after_delay(a2));
            } else {
                emit_buffer_state(&app_c, BufferState::Error, Some(&last_meaningful_err));
            }
        }
    });

    if current_state() == BufferState::Starting && GENERATION.load(std::sync::atomic::Ordering::SeqCst) == gen {
        emit_buffer_state(&app, BufferState::Active, None);
    }
    Ok(())
}

#[tauri::command]
pub async fn start_buffer(app: AppHandle) -> Result<(), String> {
    AUTO_RESTARTS.store(0, std::sync::atomic::Ordering::SeqCst);
    GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    start_pipeline(app).await
}

#[tauri::command]
pub fn pause_buffer(app: AppHandle) -> Result<(), String> {
    GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let state = app.state::<RecorderState>();
    if let Some(child_arc) = state.child.lock().unwrap().take() {
        if let Ok(mut opt) = child_arc.lock() {
            if let Some(child) = opt.take() {
                let _ = child.kill();
            }
        }
    }
    stop_audio_capture();
    crate::idle_monitor::clear_auto_pause();
    unsafe {
        use windows::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS};
        let _ = SetThreadExecutionState(ES_CONTINUOUS);
    }
    emit_buffer_state(&app, BufferState::Paused, None);
    Ok(())
}

#[tauri::command]
pub async fn resume_buffer(app: AppHandle) -> Result<(), String> {
    start_buffer(app).await
}

#[tauri::command]
pub fn stop_buffer(app: AppHandle) -> Result<(), String> {
    GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    crate::idle_monitor::clear_auto_pause();
    let state = app.state::<RecorderState>();
    if let Some(child_arc) = state.child.lock().unwrap().take() {
        if let Ok(mut opt) = child_arc.lock() {
            if let Some(child) = opt.take() {
                let _ = child.kill();
            }
        }
    }
    stop_audio_capture();
    unsafe {
        use windows::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS};
        let _ = SetThreadExecutionState(ES_CONTINUOUS);
    }
    emit_buffer_state(&app, BufferState::Stopped, None);
    Ok(())
}

#[tauri::command]
pub fn get_buffer_state() -> String {
    current_state().as_str().to_string()
}

// ──────────────────────────────────────────────────────────────────────────────
// Save clip (invoked from global hotkey or dashboard)
// ──────────────────────────────────────────────────────────────────────────────
#[tauri::command]
pub async fn save_clip_now(app: AppHandle) -> Result<(), String> {
    let game = crate::audio::get_active_game_name();
    let cfg = crate::config::get_config(app.clone());
    save_clip(app, game, cfg.buffer_length_secs, cfg.custom_clip_path).await
}

#[tauri::command]
pub async fn save_clip(
    app: AppHandle,
    game_name: String,
    buffer_length_secs: u32,
    save_path: String,
) -> Result<(), String> {
    if SAVE_IN_FLIGHT.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err("A clip save is already in progress".into());
    }
    let _save_guard = SaveGuard;

    // Instant audible confirmation sound only when save is actually accepted
    crate::audio::play_notification_sound();

    let clean_game_name: String = game_name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let clean_trimmed = clean_game_name.trim();
    let final_game_name = if clean_trimmed.is_empty() {
        "Desktop".to_string()
    } else {
        clean_trimmed.to_string()
    };

    let temp_dir = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("buffer");
    let _ = fs::create_dir_all(&temp_dir);

    let game_dir = PathBuf::from(&save_path).join(&final_game_name);
    let _ = tokio::fs::create_dir_all(&game_dir).await;

    let timestamp = Local::now().format("%Y-%m-%d-%H-%M-%S").to_string();
    let final_clip_path = game_dir.join(format!("{}-{}.mp4", final_game_name, timestamp));

    // Extract video data from memory buffer
    let extracted = extract_video_buffer(buffer_length_secs as f64)?;
    if extracted.data.is_empty() {
        return Err("Video buffer has not received frames yet".into());
    }
    let video_duration = extracted.duration_secs;
    let start_ts = extracted.start_ts;
    let end_ts = extracted.end_ts;

    let temp_video_path = temp_dir.join(format!("temp_{}.ts", timestamp));
    tokio::fs::write(&temp_video_path, extracted.data).await.map_err(|e| e.to_string())?;
    
    // RAII guard ensuring temporary video file is deleted even if FFmpeg remux fails
    struct TempFileGuard(PathBuf);
    impl Drop for TempFileGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _temp_video_guard = TempFileGuard(temp_video_path.clone());

    // Dump audio tracks DIRECTLY into game_dir using the EXACT video start & end timestamps
    let base_audio_path = game_dir.join(format!("{}-{}", final_game_name, timestamp));
    let cfg = crate::config::get_config(app.clone());
    let dumped_audio = dump_audio_clips(base_audio_path, start_ts, end_ts, cfg.audio_sync_offset_ms).unwrap_or_else(|e| {
        println!("[audio] note: no audio tracks saved ({e}), exporting video-only");
        crate::audio_engine::DumpedAudioResult {
            tracks: Vec::new(),
            spike_markers: Vec::new(),
        }
    });
    let audio_tracks = dumped_audio.tracks;
    let spike_markers = dumped_audio.spike_markers;

    let mix_tracks = &audio_tracks;

    let mut merge_args = vec![
        // Disable FFmpeg's stream probing phase: the MPEG-TS already has
        // fresh PAT/PMT every 100ms, so no probing is needed. Without this,
        // FFmpeg buffers the first ~1s of video frames while analyzing the
        // stream, causing the first second to appear in slow motion.
        "-probesize".to_string(), "32".to_string(),
        "-analyzeduration".to_string(), "0".to_string(),
        "-fflags".to_string(), "+genpts+discardcorrupt".to_string(),
        "-i".to_string(), temp_video_path.to_string_lossy().to_string(),
    ];

    let is_hevc = matches!(cfg.video_codec, crate::config::VideoCodec::HEVC);

    if !mix_tracks.is_empty() {
        for (track_path, _) in mix_tracks {
            merge_args.push("-i".to_string());
            merge_args.push(track_path.to_string_lossy().to_string());
        }
        merge_args.extend(["-map".to_string(), "0:v".to_string()]);

        let n = mix_tracks.len();
        if n == 1 {
            merge_args.extend([
                "-filter_complex".to_string(),
                "[1:a]aresample=async=1[aout]".to_string(),
                "-map".to_string(),
                "[aout]".to_string(),
            ]);
        } else {
            let inputs: String = (1..=n).map(|i| format!("[{i}:a]")).collect();
            merge_args.extend([
                "-filter_complex".to_string(),
                format!("{inputs}amix=inputs={n}:duration=shortest:normalize=0,aresample=async=1[aout]"),
                "-map".to_string(), "[aout]".to_string(),
            ]);
        }

        merge_args.extend([
            "-c:v".to_string(), "copy".to_string(),
        ]);
        if is_hevc {
            merge_args.extend(["-tag:v".to_string(), "hvc1".to_string()]);
        }
        merge_args.extend([
            "-c:a".to_string(), "aac".to_string(),
            "-b:a".to_string(), "192k".to_string(),
            "-threads".to_string(), "4".to_string(),
            "-max_muxing_queue_size".to_string(), "2048".to_string(),
            "-avoid_negative_ts".to_string(), "make_zero".to_string(),
            "-shortest".to_string(),
            "-movflags".to_string(), "+faststart".to_string(),
            "-y".to_string(),
            final_clip_path.to_string_lossy().to_string(),
        ]);
    } else {
        merge_args.extend([
            "-c:v".to_string(), "copy".to_string(),
        ]);
        if is_hevc {
            merge_args.extend(["-tag:v".to_string(), "hvc1".to_string()]);
        }
        merge_args.extend([
            "-max_muxing_queue_size".to_string(), "2048".to_string(),
            "-avoid_negative_ts".to_string(), "make_zero".to_string(),
            "-movflags".to_string(), "+faststart".to_string(),
            "-y".to_string(),
            final_clip_path.to_string_lossy().to_string(),
        ]);
    }

    let merge_cmd = app.shell().sidecar("ffmpeg").map_err(|e| e.to_string())?.args(merge_args);
    let merge_output = merge_cmd.output().await.map_err(|e| e.to_string())?;

    if !merge_output.status.success() {
        return Err(String::from_utf8_lossy(&merge_output.stderr).into_owned());
    }

    drop(_temp_video_guard);

    let duration_to_save = video_duration;

    // Generate thumbnail in background so save_clip returns immediately!
    let app_preview = app.clone();
    let final_clip_str = final_clip_path.to_string_lossy().to_string();
    tauri::async_runtime::spawn(async move {
        let _ = crate::library::generate_preview(&app_preview, &final_clip_str, duration_to_save).await;
    });

    let tracks_meta: Vec<crate::library::AudioTrackInfo> = audio_tracks.iter().enumerate()
        .map(|(i, (wav_path, name))| crate::library::AudioTrackInfo {
            track_index: i,
            process_name: name.clone(),
            wav_filename: wav_path.file_name().unwrap_or_default().to_string_lossy().to_string(),
        }).collect();

    let meta_file = crate::library::ClipMetadataFile {
        duration_secs: duration_to_save,
        audio_tracks: tracks_meta,
        spike_markers,
    };
    let tracks_json_path = game_dir.join(format!("{}-{}_tracks.json", final_game_name, timestamp));
    if let Ok(json) = serde_json::to_string(&meta_file) {
        let _ = tokio::fs::write(tracks_json_path, json).await;
    }

    let cfg = crate::config::get_config(app.clone());
    if cfg.auto_clipboard {
        let _ = crate::library::copy_clip_to_clipboard(final_clip_path.to_string_lossy().to_string());
    }

    crate::overlay::show_clip_overlay(&app, &final_game_name, duration_to_save as u32);

    let app_cleanup = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = crate::library::run_auto_cleanup(&app_cleanup);
    });

    let _ = app.emit("clips://saved", serde_json::json!({
        "path": final_clip_path.to_string_lossy(),
        "game": final_game_name,
    }));
    Ok(())
}
