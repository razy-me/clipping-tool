use crate::process_list::visible_user_apps;
use crate::process_list::with_process_snapshot;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use wasapi::*;

// Live microphone level (RMS * 10_000) polled by the dashboard meter.
static MIC_LEVEL: AtomicU32 = AtomicU32::new(0);
// Live microphone volume/gain factor (* 10_000). 10_000 = 1.0 (unity gain)
static MIC_VOLUME: AtomicU32 = AtomicU32::new(10_000);

#[tauri::command]
pub fn get_mic_level() -> f32 {
    MIC_LEVEL.load(Ordering::Relaxed) as f32 / 10_000.0
}

#[tauri::command]
pub fn set_mic_volume(volume: f32) {
    let clamped = volume.max(0.0).min(5.0);
    MIC_VOLUME.store((clamped * 10_000.0) as u32, Ordering::Relaxed);
}

pub fn get_mic_volume() -> f32 {
    MIC_VOLUME.load(Ordering::Relaxed) as f32 / 10_000.0
}

/// Names of apps currently being captured on their own track.
#[tauri::command]
pub fn get_active_split_apps() -> Vec<String> {
    let instance_guard = AUDIO_ENGINE_INSTANCE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(engine) = instance_guard.as_ref() else { return vec![] };
    let trks = engine.tracks.lock().unwrap_or_else(|e| e.into_inner());
    let mut names: Vec<String> = trks.keys()
        .filter(|k| k.as_str() != "System" && k.as_str() != "Microphone")
        .cloned()
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names
}

/// Returns the name of the currently active default input device.
#[tauri::command]
pub fn get_active_mic_device() -> String {
    let Ok(host) = cpal::host_from_id(cpal::HostId::Wasapi) else { return "Default".into(); };
    host.default_input_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_else(|| "Default Microphone".into())
}

// ──────────────────────────────────────────────────────────────────────────────
// AudioTrack – ring buffer for one audio stream.
//
// Timeline model: every received packet is stored WITH its arrival Instant.
// At dump time chunks are placed at their exact wall-clock offsets inside the
// requested window; periods without packets become true silence instead of
// compressing the timeline. Per-process loopback does NOT deliver packets
// while the target app is silent — order-only assembly shifted those tracks
// earlier and earlier. Unlike the old gap-filler, no time is ever fabricated
// under load: placement uses measured instants only.
// ──────────────────────────────────────────────────────────────────────────────
#[derive(Clone, Debug)]
pub enum AudioChunkData {
    Samples(Vec<f32>),
    Silence(usize),
}

#[derive(Clone, Debug)]
pub struct AudioChunk {
    pub data: AudioChunkData,
    pub timestamp: std::time::Instant,
}

impl AudioChunk {
    #[inline]
    pub fn len(&self) -> usize {
        match &self.data {
            AudioChunkData::Samples(v) => v.len(),
            AudioChunkData::Silence(cnt) => *cnt,
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[inline]
fn is_silence(samples: &[f32]) -> bool {
    !samples.iter().any(|&s| s.abs() >= 0.0001)
}

pub struct AudioTrack {
    pub name: String,
    chunks: VecDeque<AudioChunk>,
    pub sample_rate: u32,
    pub channels: u16,
    max_samples: usize,
    stored_samples: usize,
}

impl AudioTrack {
    pub fn new(name: String, sample_rate: u32, channels: u16, max_seconds: u32) -> Self {
        // Buffer at least (max_seconds + 30s) of audio so historical window is never prematurely evicted
        let max_samples = (sample_rate as usize) * (channels as usize) * (max_seconds as usize + 30);
        Self {
            name,
            chunks: VecDeque::new(),
            sample_rate,
            channels,
            max_samples,
            stored_samples: 0,
        }
    }

    #[inline]
    pub fn push_chunk_with_timestamp(&mut self, data: &[f32], ts: std::time::Instant) {
        if data.is_empty() { return; }

        let is_silent = is_silence(data);

        if is_silent {
            let mut appended = false;
            if let Some(last) = self.chunks.back_mut() {
                if let AudioChunkData::Silence(ref mut count) = last.data {
                    *count += data.len();
                    last.timestamp = ts;
                    appended = true;
                }
            }
            if !appended {
                self.chunks.push_back(AudioChunk {
                    data: AudioChunkData::Silence(data.len()),
                    timestamp: ts,
                });
            }
        } else {
            let spf = (self.sample_rate as usize * self.channels as usize).max(1);
            let max_chunk_samples = spf / 10; // ~100ms coalesce limit

            let mut appended = false;
            if let Some(last) = self.chunks.back_mut() {
                if let AudioChunkData::Samples(ref mut last_vec) = last.data {
                    if last_vec.len() + data.len() <= max_chunk_samples {
                        last_vec.extend_from_slice(data);
                        last.timestamp = ts;
                        appended = true;
                    }
                }
            }

            if !appended {
                self.chunks.push_back(AudioChunk {
                    data: AudioChunkData::Samples(data.to_vec()),
                    timestamp: ts,
                });
            }
        }

        self.stored_samples += data.len();
        while self.stored_samples > self.max_samples {
            match self.chunks.front_mut() {
                Some(chunk) => {
                    let chunk_len = chunk.len();
                    if chunk_len <= self.stored_samples - self.max_samples {
                        self.stored_samples -= chunk_len;
                        self.chunks.pop_front();
                    } else {
                        let excess = self.stored_samples - self.max_samples;
                        if let AudioChunkData::Silence(ref mut count) = chunk.data {
                            if *count > excess {
                                *count -= excess;
                                self.stored_samples -= excess;
                            }
                        }
                        break;
                    }
                }
                None => break,
            }
        }
    }

    #[inline]
    fn push_raw(&mut self, data: &[f32]) {
        self.push_chunk_with_timestamp(data, std::time::Instant::now());
    }

    /// Plain push (loopback / system audio).
    pub fn push_samples(&mut self, data: &[f32]) {
        self.push_raw(data);
    }

    #[inline]
    pub fn memory_bytes(&self) -> usize {
        let mut bytes = 0;
        for chunk in &self.chunks {
            match &chunk.data {
                AudioChunkData::Samples(v) => {
                    bytes += v.len() * std::mem::size_of::<f32>() + std::mem::size_of::<AudioChunk>();
                }
                AudioChunkData::Silence(_) => {
                    bytes += std::mem::size_of::<AudioChunk>();
                }
            }
        }
        bytes
    }

pub fn calibrate_window(
    win_start: std::time::Instant,
    win_end: std::time::Instant,
    offset_ms: i64,
) -> (std::time::Instant, std::time::Instant) {
    let calibrated_win_start = if offset_ms > 0 {
        win_start + std::time::Duration::from_millis(offset_ms as u64)
    } else if offset_ms < 0 {
        win_start.checked_sub(std::time::Duration::from_millis((-offset_ms) as u64)).unwrap_or(win_start)
    } else {
        win_start
    };
    let calibrated_win_end = if offset_ms > 0 {
        win_end + std::time::Duration::from_millis(offset_ms as u64)
    } else if offset_ms < 0 {
        win_end.checked_sub(std::time::Duration::from_millis((-offset_ms) as u64)).unwrap_or(win_end)
    } else {
        win_end
    };
    (calibrated_win_start, calibrated_win_end)
}

    /// Sample-accurate timeline window stitcher for A/V synchronization.
    pub fn stitch_to_window(&self, win_start: std::time::Instant, win_end: std::time::Instant, offset_ms: i64) -> Vec<f32> {
        let spf = self.sample_rate as f64 * self.channels as f64;
        let Some(dur) = win_end.checked_duration_since(win_start) else {
            return Vec::new();
        };
        let duration_secs = dur.as_secs_f64();
        if duration_secs <= 0.0 {
            return Vec::new();
        }

        let (calibrated_win_start, calibrated_win_end) = Self::calibrate_window(win_start, win_end, offset_ms);

        let out_len = (spf * duration_secs) as usize;
        let mut samples = vec![0.0f32; out_len];
        let mut cursor: usize = usize::MAX;

        for chunk in &self.chunks {
            let chunk_len = chunk.len();
            if chunk_len == 0 { continue; }

            let chunk_dur = chunk_len as f64 / spf;
            let chunk_dur_dur = std::time::Duration::from_secs_f64(chunk_dur);
            let chunk_start = chunk.timestamp.checked_sub(chunk_dur_dur).unwrap_or(chunk.timestamp);

            // Skip chunks entirely outside the window.
            if chunk.timestamp <= calibrated_win_start {
                continue;
            }
            if chunk_start >= calibrated_win_end {
                break;
            }

            let (mut skip_in_data, nominal_dst) = if chunk_start < calibrated_win_start {
                let skip = ((calibrated_win_start.duration_since(chunk_start)).as_secs_f64() * spf).round() as usize;
                (skip.min(chunk_len), 0usize)
            } else {
                let dst = ((chunk_start.duration_since(calibrated_win_start)).as_secs_f64() * spf).round() as usize;
                (0usize, dst)
            };

            let mut avail = chunk_len.saturating_sub(skip_in_data);
            if avail == 0 || nominal_dst >= out_len {
                continue;
            }

            // 40ms timer jitter tolerance: contiguous playback stitches seamlessly without crackle/gaps
            let jitter_tolerance_samples = (spf * 0.040) as usize;
            let dst = if cursor == usize::MAX {
                nominal_dst
            } else if nominal_dst <= cursor + jitter_tolerance_samples && nominal_dst + jitter_tolerance_samples >= cursor {
                cursor
            } else if nominal_dst > cursor {
                nominal_dst
            } else {
                let overlap = cursor.saturating_sub(nominal_dst);
                let extra_skip = overlap.min(avail);
                skip_in_data += extra_skip;
                avail = avail.saturating_sub(extra_skip);
                cursor
            };

            if dst >= out_len || avail == 0 {
                continue;
            }

            let take = avail.min(out_len - dst);
            if take > 0 {
                if let AudioChunkData::Samples(v) = &chunk.data {
                    samples[dst..dst + take].copy_from_slice(&v[skip_in_data..skip_in_data + take]);
                }
                cursor = dst + take;
            }
        }

        samples
    }

    /// 100% Raw microphone capture (scaled by user mic_volume gain, without gating/compression).
    pub fn push_mic(&mut self, data: &[f32]) {
        if data.is_empty() { return; }
        let gain = get_mic_volume();
        let gained_buf: Vec<f32>;
        let samples: &[f32] = if (gain - 1.0).abs() > 0.001 {
            gained_buf = data.iter().map(|&s| s * gain).collect();
            &gained_buf
        } else {
            data
        };

        let mut peak: f32 = 0.0;
        let mut sum_sq: f32 = 0.0;
        for &s in samples {
            let a = s.abs();
            if a > peak { peak = a; }
            sum_sq += s * s;
        }
        let rms = (sum_sq / samples.len() as f32).sqrt();
        let metric = peak.max(rms * 1.6);
        let cur = MIC_LEVEL.load(Ordering::Relaxed) as f32 / 10_000.0;
        let smoothed = if metric > cur { metric } else { cur * 0.94 };
        MIC_LEVEL.store((smoothed.min(1.0) * 10_000.0) as u32, Ordering::Relaxed);

        // VAD / Noise-gate: If RMS is below ambient background floor (< 0.0015), zero out silence to eliminate hiss & noise
        if rms < 0.0015 {
            let silence = vec![0.0f32; samples.len()];
            self.push_raw(&silence);
        } else {
            self.push_raw(samples);
        }
    }
}

pub struct SpikeDetector {
    pub enabled: bool,
    pub threshold: f32,
    pub ema_rms: f32,
    pub last_spike: Option<std::time::Instant>,
    pub spikes: VecDeque<std::time::Instant>,
}

impl SpikeDetector {
    pub fn new(enabled: bool, threshold: f32) -> Self {
        Self {
            enabled,
            threshold,
            ema_rms: 0.01,
            last_spike: None,
            spikes: VecDeque::new(),
        }
    }

    pub fn feed(&mut self, rms: f32, now: std::time::Instant, max_age: std::time::Duration) {
        if !self.enabled {
            return;
        }

        // Always prune expired spikes even during silence
        while let Some(front) = self.spikes.front() {
            if now.checked_duration_since(*front).map_or(false, |d| d > max_age) {
                self.spikes.pop_front();
            } else {
                break;
            }
        }

        if rms < 0.005 {
            // Smoothly decay noise floor estimate during silence
            self.ema_rms = self.ema_rms * 0.98 + 0.005 * 0.02;
            return;
        }

        self.ema_rms = self.ema_rms * 0.96 + rms * 0.04;

        let mult = 2.0 + self.threshold * 2.5;
        let min_abs = 0.08 + self.threshold * 0.12;
        let is_spike = rms > self.ema_rms * mult && rms > min_abs;

        if is_spike {
            let can_fire = match self.last_spike {
                Some(last) => now.checked_duration_since(last).map_or(false, |d| d > std::time::Duration::from_millis(1800)),
                None => true,
            };
            if can_fire {
                self.last_spike = Some(now);
                self.spikes.push_back(now);
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// AudioEngine – owns all tracks and the shared stop signal
// ──────────────────────────────────────────────────────────────────────────────
pub struct AudioEngine {
    stop_signal: Arc<AtomicBool>,
    pub tracks: Arc<Mutex<HashMap<String, AudioTrack>>>,
    pub spike_detector: Arc<Mutex<SpikeDetector>>,
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.stop_signal.store(true, Ordering::Relaxed);
    }
}

static AUDIO_ENGINE_INSTANCE: Mutex<Option<Arc<AudioEngine>>> = Mutex::new(None);

pub fn get_audio_buffer_bytes() -> usize {
    if let Ok(guard) = AUDIO_ENGINE_INSTANCE.lock() {
        if let Some(engine) = guard.as_ref() {
            if let Ok(tracks) = engine.tracks.lock() {
                return tracks.values().map(|t| t.memory_bytes()).sum();
            }
        }
    }
    0
}

#[derive(Clone, Debug)]
pub struct DumpedAudioResult {
    pub tracks: Vec<(PathBuf, String)>,
    pub spike_markers: Vec<f64>,
}

// ──────────────────────────────────────────────────────────────────────────────
// Public API
// ──────────────────────────────────────────────────────────────────────────────
pub fn start_audio_capture(
    buffer_length_secs: u32,
    spike_detection_enabled: bool,
    spike_threshold: f32,
) -> Result<(), String> {
    let tracks = Arc::new(Mutex::new(HashMap::new()));
    let stop_signal = Arc::new(AtomicBool::new(false));
    let spike_detector = Arc::new(Mutex::new(SpikeDetector::new(spike_detection_enabled, spike_threshold)));

    // ── System Audio Loopback via CPAL ─────────────────────────────────────
    {
        let host = cpal::host_from_id(cpal::HostId::Wasapi)
            .map_err(|e| format!("WASAPI host error: {e}"))?;

        if let Some(device) = host.default_output_device() {
            if let Ok(config) = device.default_output_config() {
                let sr = config.sample_rate().0;
                let ch = config.channels();
                tracks.lock().unwrap().insert(
                    "System".to_string(),
                    AudioTrack::new("System".to_string(), sr, ch, buffer_length_secs),
                );

                let stop = stop_signal.clone();
                let tracks_c = tracks.clone();
                let stream_cfg: cpal::StreamConfig = config.clone().into();

                thread::spawn(move || {
                    while !stop.load(Ordering::Relaxed) {
                        let failed = Arc::new(AtomicBool::new(false));
                        let failed_c = failed.clone();
                        let tc = tracks_c.clone();
                        let stream = match config.sample_format() {
                            cpal::SampleFormat::F32 => device.build_input_stream(
                                &stream_cfg,
                                {
                                    let mut overflow = Vec::new();
                                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                                        if let Ok(mut trks) = tc.try_lock() {
                                            if let Some(t) = trks.get_mut("System") {
                                                if !overflow.is_empty() {
                                                    t.push_samples(&overflow);
                                                    overflow.clear();
                                                }
                                                t.push_samples(data);
                                            }
                                        } else {
                                            overflow.extend_from_slice(data);
                                        }
                                    }
                                },
                                move |_err| { failed_c.store(true, Ordering::Relaxed); },
                                None,
                            ),
                            cpal::SampleFormat::I16 => device.build_input_stream(
                                &stream_cfg,
                                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                                    let f32_data: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                                    if let Ok(mut trks) = tc.lock() {
                                        if let Some(t) = trks.get_mut("System") {
                                            t.push_samples(&f32_data);
                                        }
                                    }
                                },
                                move |_err| { failed_c.store(true, Ordering::Relaxed); },
                                None,
                            ),
                            cpal::SampleFormat::U16 => device.build_input_stream(
                                &stream_cfg,
                                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                                    let f32_data: Vec<f32> = data.iter().map(|&s| (s as f32 - 32768.0) / 32768.0).collect();
                                    if let Ok(mut trks) = tc.lock() {
                                        if let Some(t) = trks.get_mut("System") {
                                            t.push_samples(&f32_data);
                                        }
                                    }
                                },
                                move |_err| { failed_c.store(true, Ordering::Relaxed); },
                                None,
                            ),
                            _ => {
                                eprintln!("[audio] Unsupported sample format for system: {:?}", config.sample_format());
                                return;
                            }
                        };
                        match stream {
                            Ok(s) => {
                                if s.play().is_ok() {
                                    while !stop.load(Ordering::Relaxed) && !failed.load(Ordering::Relaxed) {
                                        thread::sleep(Duration::from_millis(100));
                                    }
                                } else {
                                    thread::sleep(Duration::from_secs(1));
                                }
                                drop(s);
                            }
                            Err(e) => {
                                eprintln!("Failed to build system audio stream: {e}");
                                thread::sleep(Duration::from_secs(2));
                            }
                        }
                        if !stop.load(Ordering::Relaxed) {
                            thread::sleep(Duration::from_millis(500));
                        }
                    }
                });
            }
        }
    }

    // ── Microphone via CPAL (100% Raw) ─────────────────────────────────────
    {
        let host = cpal::host_from_id(cpal::HostId::Wasapi)
            .map_err(|e| format!("WASAPI host error mic: {e}"))?;

        if let Some(device) = host.default_input_device() {
            if let Ok(config) = device.default_input_config() {
                let sr = config.sample_rate().0;
                let ch = config.channels();
                tracks.lock().unwrap().insert(
                    "Microphone".to_string(),
                    AudioTrack::new("Microphone".to_string(), sr, ch, buffer_length_secs),
                );

                let stop = stop_signal.clone();
                let tracks_c = tracks.clone();
                let spike_det_c = spike_detector.clone();
                let buf_len = buffer_length_secs;
                let stream_cfg: cpal::StreamConfig = config.clone().into();
                let spike_enabled = spike_detection_enabled;

                thread::spawn(move || {
                    while !stop.load(Ordering::Relaxed) {
                        let failed = Arc::new(AtomicBool::new(false));
                        let failed_c = failed.clone();
                        let tc = tracks_c.clone();
                        let sdc = spike_det_c.clone();
                        let stream = match config.sample_format() {
                            cpal::SampleFormat::F32 => device.build_input_stream(
                                &stream_cfg,
                                {
                                    let mut overflow = Vec::new();
                                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                                        if let Ok(mut trks) = tc.try_lock() {
                                            if let Some(t) = trks.get_mut("Microphone") {
                                                if !overflow.is_empty() {
                                                    t.push_mic(&overflow);
                                                    overflow.clear();
                                                }
                                                t.push_mic(data);
                                            }
                                        } else {
                                            overflow.extend_from_slice(data);
                                        }
                                        if spike_enabled && !data.is_empty() {
                                            let sum_sq: f32 = data.iter().map(|s| s * s).sum();
                                            let rms = (sum_sq / data.len() as f32).sqrt();
                                            if let Ok(mut sd) = sdc.lock() {
                                                sd.feed(rms, std::time::Instant::now(), std::time::Duration::from_secs((buf_len + 15) as u64));
                                            }
                                        }
                                    }
                                },
                                move |_err| { failed_c.store(true, Ordering::Relaxed); },
                                None,
                            ),
                            cpal::SampleFormat::I16 => device.build_input_stream(
                                &stream_cfg,
                                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                                    let f32_data: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                                    if let Ok(mut trks) = tc.lock() {
                                        if let Some(t) = trks.get_mut("Microphone") {
                                            t.push_mic(&f32_data);
                                        }
                                    }
                                    if spike_enabled && !f32_data.is_empty() {
                                        let sum_sq: f32 = f32_data.iter().map(|s| s * s).sum();
                                        let rms = (sum_sq / f32_data.len() as f32).sqrt();
                                        if let Ok(mut sd) = sdc.lock() {
                                            sd.feed(rms, std::time::Instant::now(), std::time::Duration::from_secs((buf_len + 15) as u64));
                                        }
                                    }
                                },
                                move |_err| { failed_c.store(true, Ordering::Relaxed); },
                                None,
                            ),
                            cpal::SampleFormat::U16 => device.build_input_stream(
                                &stream_cfg,
                                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                                    let f32_data: Vec<f32> = data.iter().map(|&s| (s as f32 - 32768.0) / 32768.0).collect();
                                    if let Ok(mut trks) = tc.lock() {
                                        if let Some(t) = trks.get_mut("Microphone") {
                                            t.push_mic(&f32_data);
                                        }
                                    }
                                    if spike_enabled && !f32_data.is_empty() {
                                        let sum_sq: f32 = f32_data.iter().map(|s| s * s).sum();
                                        let rms = (sum_sq / f32_data.len() as f32).sqrt();
                                        if let Ok(mut sd) = sdc.lock() {
                                            sd.feed(rms, std::time::Instant::now(), std::time::Duration::from_secs((buf_len + 15) as u64));
                                        }
                                    }
                                },
                                move |_err| { failed_c.store(true, Ordering::Relaxed); },
                                None,
                            ),
                            _ => {
                                eprintln!("[audio] Unsupported sample format for mic: {:?}", config.sample_format());
                                return;
                            }
                        };
                        match stream {
                            Ok(s) => {
                                if s.play().is_ok() {
                                    while !stop.load(Ordering::Relaxed) && !failed.load(Ordering::Relaxed) {
                                        thread::sleep(Duration::from_millis(100));
                                    }
                                } else {
                                    thread::sleep(Duration::from_secs(1));
                                }
                                drop(s);
                            }
                            Err(e) => {
                                eprintln!("Failed to build mic stream: {e}");
                                thread::sleep(Duration::from_secs(2));
                            }
                        }
                        if !stop.load(Ordering::Relaxed) {
                            thread::sleep(Duration::from_millis(500));
                        }
                    }
                });
            }
        }
    }

    // ── Per-App WASAPI ApplicationLoopback: AUTOMATIC discovery ────────────
    // Every visible user application gets its own track with zero
    // configuration. A supervisor thread re-scans visible windows; workers
    // attach process-tree loopback per app and exit when the app disappears.
    {
        type KillFlag = Arc<AtomicBool>;
        let registry: Arc<Mutex<HashMap<String, KillFlag>>> = Arc::new(Mutex::new(HashMap::new()));

        let stop_sup = stop_signal.clone();
        let tracks_sup = tracks.clone();
        let reg_sup = registry.clone();
        let buf_len_sup = buffer_length_secs;

        thread::spawn(move || {
            loop {
                if stop_sup.load(Ordering::Relaxed) { break; }

                for app in visible_user_apps() {
                    if stop_sup.load(Ordering::Relaxed) { break; }
                    let mut reg = reg_sup.lock().unwrap();
                    if reg.contains_key(&app.name) { continue; }

                    let kill = Arc::new(AtomicBool::new(false));
                    reg.insert(app.name.clone(), kill.clone());
                    drop(reg);

                    let exe = app.name.clone();
                    let kill = kill;
                    let stop = stop_sup.clone();
                    let tracks_c = tracks_sup.clone();
                    let reg_worker = reg_sup.clone();

                    thread::spawn(move || {
                        let _ = initialize_mta();
                        unsafe {
                            let _ = windows::Win32::System::Threading::SetThreadPriority(
                                windows::Win32::System::Threading::GetCurrentThread(),
                                windows::Win32::System::Threading::THREAD_PRIORITY_ABOVE_NORMAL,
                            );
                        }
                        loop {
                            if stop.load(Ordering::Relaxed) || kill.load(Ordering::Relaxed) { break; }

                            // find_app_pid resolves the CURRENT root pid of this
                            // exe each cycle (apps can restart under new pids).
                            match find_app_pid(&exe) {
                                Some(pid) => {
                                    match AudioClient::new_application_loopback_client(pid, true) {
                                        Ok(mut audio_client) => {
                                            let mode = StreamMode::EventsShared {
                                                autoconvert: true,
                                                buffer_duration_hns: 0,
                                            };

                                            // Try standard 48000 Hz, with fallbacks to 44100 Hz, 96000 Hz, and 192000 Hz if hardware/DAC requires
                                            let mut matched_sr: Option<u32> = None;
                                            for sample_rate in [48000usize, 44100, 96000, 192000] {
                                                let desired_format =
                                                    WaveFormat::new(32, 32, &SampleType::Float, sample_rate, 2, None);
                                                if audio_client.initialize_client(&desired_format, &Direction::Capture, &mode).is_ok() {
                                                    matched_sr = Some(sample_rate as u32);
                                                    break;
                                                }
                                            }

                                            if let Some(sr) = matched_sr {
                                                if let (Ok(h_event), Ok(capture_client)) = (
                                                    audio_client.set_get_eventhandle(),
                                                    audio_client.get_audiocaptureclient(),
                                                ) {
                                                    // Preserve accumulated audio across
                                                    // session hiccups: insert only when absent.
                                                    {
                                                        let mut trks = tracks_c.lock().unwrap();
                                                        trks.entry(exe.clone()).or_insert_with(|| {
                                                            AudioTrack::new(exe.clone(), sr, 2, buf_len_sup)
                                                        });
                                                    }

                                                    if audio_client.start_stream().is_ok() {
                                                        let mut sample_buf: VecDeque<u8> = VecDeque::new();
                                                        let mut overflow_f32: Vec<f32> = Vec::new();

                                                        loop {
                                                            if stop.load(Ordering::Relaxed) || kill.load(Ordering::Relaxed) { break; }

                                                            if h_event.wait_for_event(2000).is_err() {
                                                                break; // session hiccup → reconnect below
                                                            }

                                                            loop {
                                                                match capture_client.get_next_packet_size() {
                                                                    Ok(Some(0)) | Ok(None) | Err(_) => break,
                                                                    Ok(Some(_)) => {}
                                                                }
                                                                if capture_client.read_from_device_to_deque(&mut sample_buf).is_err() {
                                                                    break;
                                                                }
                                                                if !sample_buf.is_empty() {
                                                                    let slice = sample_buf.make_contiguous();
                                                                    let f32s: Vec<f32> = slice
                                                                        .chunks_exact(4)
                                                                        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                                                                        .collect();
                                                                    sample_buf.clear();
                                                                    
                                                                    if let Ok(mut trks) = tracks_c.try_lock() {
                                                                        if let Some(t) = trks.get_mut(&exe) {
                                                                            if !overflow_f32.is_empty() {
                                                                                t.push_samples(&overflow_f32);
                                                                                overflow_f32.clear();
                                                                            }
                                                                            t.push_samples(&f32s);
                                                                        }
                                                                    } else {
                                                                        overflow_f32.extend_from_slice(&f32s);
                                                                    }
                                                                }
                                                            }
                                                        }

                                                        let _ = audio_client.stop_stream();
                                                    } else {
                                                        eprintln!("[audio] start_stream failed for {}", exe);
                                                        thread::sleep(Duration::from_secs(1));
                                                    }
                                                }
                                            } else {
                                                eprintln!("[audio] initialize_client failed for {}", exe);
                                                thread::sleep(Duration::from_secs(1));
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("[audio] loopback client failed for {} (PID {}): {}", exe, pid, e);
                                            thread::sleep(Duration::from_secs(1));
                                        }
                                    }
                                }
                                None => {
                                    // Process gone. Small grace so brief relaunches
                                    // don't tear the track down.
                                    thread::sleep(Duration::from_millis(2500));
                                    if kill.load(Ordering::Relaxed)
                                        || find_app_pid(&exe).is_none()
                                    {
                                        break;
                                    }
                                }
                            }
                        }

                        // App gone or worker finished → unregister worker.
                        // NOTE: We do NOT delete the track from tracks_c immediately!
                        // This guarantees that if the user clips right after closing/crashing a game,
                        // all audio recorded during the buffer window is preserved and saved.
                        reg_worker.lock().unwrap().remove(&exe);
                    });
                }

                // Periodic cleanup of truly expired tracks (no samples in the last buffer_length + 30s window)
                if let Ok(mut trks) = tracks_sup.try_lock() {
                    let reg = reg_sup.lock().unwrap();
                    let max_track_age = Duration::from_secs((buf_len_sup + 30) as u64);
                    trks.retain(|name, track| {
                        // Always keep core tracks and actively captured tracks
                        if name == "System" || name == "Microphone" || reg.contains_key(name) {
                            return true;
                        }
                        // For exited apps, keep them as long as they have recent audio in the rolling buffer
                        if let Some(last) = track.chunks.back() {
                            last.timestamp.elapsed() <= max_track_age
                        } else {
                            false
                        }
                    });
                }

                let sleep_secs = if crate::hardware_profile::get_cached_tier() == crate::hardware_profile::HardwareTier::Weak || crate::hardware_profile::is_on_battery() { 10 } else { 5 };
                thread::sleep(Duration::from_secs(sleep_secs));
            }
        });
    }


    let engine = Arc::new(AudioEngine {
        stop_signal,
        tracks,
        spike_detector,
    });

    let mut instance = AUDIO_ENGINE_INSTANCE.lock().unwrap();
    *instance = Some(engine);

    Ok(())
}

/// Find the root PID of a named app using one shared, throttled snapshot.
fn find_app_pid(app_name: &str) -> Option<u32> {
    let target = app_name.to_lowercase().trim_end_matches(".exe").to_string();

    with_process_snapshot(|sys| {
        let mut candidates: Vec<(u32, Option<u32>)> = Vec::new();
        for (pid, process) in sys.processes() {
            let name = process.name().to_string_lossy().to_lowercase();
            let exe_name = process.exe()
                .and_then(|p| p.file_name())
                .map(|f| f.to_string_lossy().to_lowercase())
                .unwrap_or_default();

            if name.trim_end_matches(".exe") == target || exe_name.trim_end_matches(".exe") == target {
                candidates.push((pid.as_u32(), process.parent().map(|p| p.as_u32())));
            }
        }

        if candidates.is_empty() {
            return None;
        }

        // Pick the root process (parent is NOT the same app).
        let all_pids: std::collections::HashSet<u32> =
            candidates.iter().map(|(p, _)| *p).collect();
        for (pid, parent_pid) in &candidates {
            let parent_is_same = parent_pid.map_or(false, |pp| all_pids.contains(&pp));
            if !parent_is_same {
                return Some(*pid);
            }
        }
        candidates.first().map(|(p, _)| *p)
    })
}

pub fn stop_audio_capture() {
    let mut instance = AUDIO_ENGINE_INSTANCE.lock().unwrap();
    let _ = instance.take(); // Drop fires stop_signal via AudioEngine::Drop
}

// ──────────────────────────────────────────────────────────────────────────────
// WAV output – single bulk write instead of ~30M per-sample calls.
// Format: PCM float32 little-endian.
// ──────────────────────────────────────────────────────────────────────────────
pub fn write_wav_f32(path: &PathBuf, sample_rate: u32, channels: u16, samples: &[f32]) -> std::io::Result<()> {
    use std::io::Write;

    let data_len = (samples.len() * 4) as u32;
    let mut out = Vec::with_capacity(data_len as usize + 44);

    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36u32 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());          // fmt chunk size
    out.extend_from_slice(&3u16.to_le_bytes());           // IEEE float
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * channels as u32 * 4).to_le_bytes());
    out.extend_from_slice(&(channels * 4).to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());          // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }

    let mut file = std::fs::File::create(path)?;
    file.write_all(&out)
}

pub fn dump_audio_clips(
    base_output_path: PathBuf,
    win_start: std::time::Instant,
    win_end: std::time::Instant,
    audio_offset_ms: i32,
) -> Result<DumpedAudioResult, String> {
    let engine = {
        let instance_guard = AUDIO_ENGINE_INSTANCE.lock().unwrap_or_else(|e| e.into_inner());
        instance_guard.as_ref().ok_or_else(|| "Audio engine not running".to_string())?.clone()
    };

    let (calibrated_win_start, calibrated_win_end) = AudioTrack::calibrate_window(win_start, win_end, audio_offset_ms as i64);

    let duration_secs = if win_end >= win_start {
        win_end.duration_since(win_start).as_secs_f64().clamp(0.01, 600.0)
    } else {
        0.01
    };

    // Extract spike markers within the window
    let spike_markers: Vec<f64> = {
        if let Ok(sd) = engine.spike_detector.lock() {
            sd.spikes.iter().filter_map(|t| {
                if *t >= calibrated_win_start && *t <= calibrated_win_end {
                    Some(t.duration_since(calibrated_win_start).as_secs_f64())
                } else {
                    None
                }
            }).collect()
        } else {
            Vec::new()
        }
    };

    // Snapshot under a short lock using the tested stitch_to_window engine.
    let raw_tracks: Vec<(String, u32, u16, Vec<f32>)> = {
        let trks = engine.tracks.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = Vec::new();
        for (name, track) in trks.iter() {
            let samples = track.stitch_to_window(win_start, win_end, audio_offset_ms as i64);
            if !samples.is_empty() {
                println!(
                    "[audio] track '{}': window {:.1}s, stitched {} samples",
                    name, duration_secs, samples.len()
                );
                out.push((name.clone(), track.sample_rate, track.channels, samples));
            }
        }
        out
    };

    if raw_tracks.is_empty() {
        return Ok(DumpedAudioResult {
            tracks: Vec::new(),
            spike_markers,
        });
    }

    // Filter tracks to eliminate duplication and silent app noise:
    // 1. Separate Microphone, System, and per-app tracks.
    // 2. An app track is active if peak amplitude >= 0.001.
    // 3. If any active per-app tracks exist, exclude System (since System is the sum of all apps).
    // 4. If no active per-app tracks exist, keep System as fallback desktop audio.
    let mut tracks_to_save = Vec::new();
    let mut active_app_tracks = Vec::new();
    let mut system_track = None;
    let mut mic_track = None;

    for (name, sr, ch, samples) in raw_tracks {
        let peak = samples.iter().fold(0.0f32, |acc, &x| acc.max(x.abs()));
        if name == "Microphone" {
            mic_track = Some((name, sr, ch, samples));
        } else if name == "System" {
            system_track = Some((name, sr, ch, samples));
        } else if peak >= 0.001 {
            // App has audible sound
            active_app_tracks.push((name, sr, ch, samples));
        }
    }

    if !active_app_tracks.is_empty() {
        // Dedicated per-app tracks with audio (e.g. Chrome, Game, Discord).
        // Discard System so sounds are NOT doubled!
        tracks_to_save.extend(active_app_tracks);
    } else if let Some(sys) = system_track {
        // Fallback: no isolated app tracks had audio, so use System.
        tracks_to_save.push(sys);
    }

    // Always include Microphone if present
    if let Some(mic) = mic_track {
        tracks_to_save.push(mic);
    }

    if tracks_to_save.is_empty() {
        return Ok(DumpedAudioResult {
            tracks: Vec::new(),
            spike_markers,
        });
    }

    let mut saved_paths = Vec::new();

    for (name, sample_rate, channels, mut samples) in tracks_to_save {
        // 5ms fade-out to prevent clicks
        let fade_samples = ((sample_rate as f64) * (channels as f64) * 0.005) as usize;
        let total = samples.len();
        if total > fade_samples {
            let fade_start = total - fade_samples;
            for i in 0..fade_samples {
                let factor = 1.0 - (i as f32 / fade_samples as f32);
                samples[fade_start + i] *= factor;
            }
        }

        let mut out_path = base_output_path.clone();
        let file_name = format!(
            "{}_{}.wav",
            out_path.file_stem().unwrap_or_default().to_string_lossy(),
            name
        );
        out_path.set_file_name(file_name);

        match write_wav_f32(&out_path, sample_rate, channels, &samples) {
            Ok(_) => saved_paths.push((out_path, name)),
            Err(e) => eprintln!("[audio] failed to write {}: {e}", out_path.display()),
        }
    }

    Ok(DumpedAudioResult {
        tracks: saved_paths,
        spike_markers,
    })
}
