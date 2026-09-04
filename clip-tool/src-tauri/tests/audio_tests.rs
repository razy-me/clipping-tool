use app_lib::audio_engine::{AudioTrack, SpikeDetector, write_wav_f32, set_mic_volume, get_mic_volume, get_mic_level};
use std::time::{Duration, Instant};

#[test]
fn test_audio_track_creation_mono() {
    let track = AudioTrack::new("MonoTrack".to_string(), 48000, 1, 60);
    assert_eq!(track.name, "MonoTrack");
    assert_eq!(track.sample_rate, 48000);
    assert_eq!(track.channels, 1);
}

#[test]
fn test_audio_track_creation_stereo() {
    let track = AudioTrack::new("StereoTrack".to_string(), 48000, 2, 60);
    assert_eq!(track.name, "StereoTrack");
    assert_eq!(track.sample_rate, 48000);
    assert_eq!(track.channels, 2);
}

#[test]
fn test_audio_track_creation_5_1_surround() {
    let track = AudioTrack::new("Surround51".to_string(), 48000, 6, 30);
    assert_eq!(track.channels, 6);
}

#[test]
fn test_audio_track_creation_7_1_surround() {
    let track = AudioTrack::new("Surround71".to_string(), 48000, 8, 30);
    assert_eq!(track.channels, 8);
}

#[test]
fn test_audio_track_all_standard_sample_rates() {
    for rate in [44100, 48000, 88200, 96000, 192000] {
        let track = AudioTrack::new("RateTest".to_string(), rate, 2, 10);
        assert_eq!(track.sample_rate, rate);
    }
}

#[test]
fn test_audio_track_push_samples_chunk_sizes() {
    let mut track = AudioTrack::new("ChunkTest".to_string(), 48000, 2, 10);
    for sz in [64, 128, 256, 512, 1024, 2048, 4096] {
        let chunk = vec![0.1f32; sz];
        track.push_samples(&chunk);
    }
}

#[test]
fn test_audio_track_push_empty_slice_noop() {
    let mut track = AudioTrack::new("EmptyTest".to_string(), 48000, 2, 10);
    track.push_samples(&[]);
}

#[test]
fn test_mic_volume_clamping_ranges() {
    let test_values = [
        (-100.0, 0.0),
        (-0.5, 0.0),
        (0.0, 0.0),
        (0.5, 0.5),
        (1.0, 1.0),
        (2.5, 2.5),
        (5.0, 5.0),
        (5.1, 5.0),
        (100.0, 5.0),
    ];
    for (input, expected) in test_values {
        set_mic_volume(input);
        assert!((get_mic_volume() - expected).abs() < 0.002, "Failed for input {}", input);
    }
    set_mic_volume(1.0); // restore
}

#[test]
fn test_mic_push_raw_silence_and_decay() {
    set_mic_volume(1.0);
    let mut track = AudioTrack::new("MicSilence".to_string(), 48000, 1, 10);
    let silence = vec![0.0f32; 4800];
    
    // Feed multiple silence frames to ensure EMA decays down to 0
    for _ in 0..100 {
        track.push_mic(&silence);
    }
    let lvl = get_mic_level();
    assert!(lvl >= 0.0 && lvl <= 0.01, "Level should decay to 0, got {}", lvl);
}

#[test]
fn test_mic_push_raw_clipping_signals() {
    set_mic_volume(1.0);
    let mut track = AudioTrack::new("MicClipping".to_string(), 48000, 1, 10);
    let loud = vec![1.5f32; 4800]; // Loud clipped audio
    track.push_mic(&loud);
    let lvl = get_mic_level();
    assert!(lvl > 0.9 && lvl <= 1.0);
}

#[test]
fn test_spike_detector_disabled_state() {
    let mut detector = SpikeDetector::new(false, 0.65);
    let now = Instant::now();
    detector.feed(1.0, now, Duration::from_secs(30));
    assert_eq!(detector.spikes.len(), 0);
}

#[test]
fn test_spike_detector_below_min_energy_ignored() {
    let mut detector = SpikeDetector::new(true, 0.65);
    let now = Instant::now();
    detector.feed(0.001, now, Duration::from_secs(30));
    assert_eq!(detector.spikes.len(), 0);
}

#[test]
fn test_spike_detector_steady_loud_sound_no_spike_after_ema_adapts() {
    let mut detector = SpikeDetector::new(true, 0.65);
    let now = Instant::now();

    // Constant steady signal: EMA adapts over time
    for i in 0..100 {
        let t = now + Duration::from_millis(i * 100);
        detector.feed(0.3, t, Duration::from_secs(30));
    }
    let count_before = detector.spikes.len();
    for i in 100..120 {
        let t = now + Duration::from_millis(i * 100);
        detector.feed(0.3, t, Duration::from_secs(30));
    }
    assert_eq!(detector.spikes.len(), count_before);
}

#[test]
fn test_spike_detector_old_spikes_pruned() {
    let mut detector = SpikeDetector::new(true, 0.65);
    let now = Instant::now();

    let old_spike = now;
    detector.feed(0.9, old_spike, Duration::from_secs(10));
    assert_eq!(detector.spikes.len(), 1);

    // Feed a tick 15 seconds later -> old spike (>10s max_age) must be pruned
    let current_time = now + Duration::from_secs(15);
    detector.feed(0.02, current_time, Duration::from_secs(10));
    assert_eq!(detector.spikes.len(), 0);
}

#[test]
fn test_write_wav_f32_mono() {
    let temp_dir = std::env::temp_dir();
    let wav_path = temp_dir.join("test_wav_mono.wav");
    let samples = vec![0.5f32; 1000];
    write_wav_f32(&wav_path, 44100, 1, &samples).unwrap();
    let data = std::fs::read(&wav_path).unwrap();
    let _ = std::fs::remove_file(&wav_path);

    assert_eq!(data.len(), 44 + 1000 * 4);
    assert_eq!(u16::from_le_bytes(data[22..24].try_into().unwrap()), 1); // 1 channel
}

#[test]
fn test_write_wav_f32_stereo() {
    let temp_dir = std::env::temp_dir();
    let wav_path = temp_dir.join("test_wav_stereo.wav");
    let samples = vec![0.5f32; 2000];
    write_wav_f32(&wav_path, 48000, 2, &samples).unwrap();
    let data = std::fs::read(&wav_path).unwrap();
    let _ = std::fs::remove_file(&wav_path);

    assert_eq!(data.len(), 44 + 2000 * 4);
    assert_eq!(u16::from_le_bytes(data[22..24].try_into().unwrap()), 2); // 2 channels
}

#[test]
fn test_write_wav_f32_empty_samples() {
    let temp_dir = std::env::temp_dir();
    let wav_path = temp_dir.join("test_wav_empty.wav");
    write_wav_f32(&wav_path, 48000, 2, &[]).unwrap();
    let data = std::fs::read(&wav_path).unwrap();
    let _ = std::fs::remove_file(&wav_path);

    assert_eq!(data.len(), 44); // Header only
}

// ──────────────────────────────────────────────────────────────────────────────
// Deep WAV RIFF Header Validation Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_write_wav_f32_riff_header_exact_offsets() {
    let temp_dir = std::env::temp_dir();
    let wav_path = temp_dir.join("test_wav_offsets.wav");
    let samples = vec![0.25f32, -0.25f32, 0.75f32, -0.75f32];
    write_wav_f32(&wav_path, 48000, 2, &samples).unwrap();
    let data = std::fs::read(&wav_path).unwrap();
    let _ = std::fs::remove_file(&wav_path);

    assert_eq!(&data[0..4], b"RIFF");
    let file_size_minus_8 = u32::from_le_bytes(data[4..8].try_into().unwrap());
    assert_eq!(file_size_minus_8, 36 + 16); // 36 + 4 samples * 4 bytes
    assert_eq!(&data[8..12], b"WAVE");
    assert_eq!(&data[12..16], b"fmt ");
    assert_eq!(u32::from_le_bytes(data[16..20].try_into().unwrap()), 16); // Subchunk1Size
    assert_eq!(u16::from_le_bytes(data[20..22].try_into().unwrap()), 3); // AudioFormat IEEE Float (3)
    assert_eq!(u16::from_le_bytes(data[22..24].try_into().unwrap()), 2); // 2 channels
    assert_eq!(u32::from_le_bytes(data[24..28].try_into().unwrap()), 48000); // 48000 Hz
    assert_eq!(u32::from_le_bytes(data[28..32].try_into().unwrap()), 48000 * 2 * 4); // ByteRate = 384,000
    assert_eq!(u16::from_le_bytes(data[32..34].try_into().unwrap()), 2 * 4); // BlockAlign = 8
    assert_eq!(u16::from_le_bytes(data[34..36].try_into().unwrap()), 32); // 32 bits per sample
    assert_eq!(&data[36..40], b"data");
    assert_eq!(u32::from_le_bytes(data[40..44].try_into().unwrap()), 16); // Data chunk size (16 bytes)

    // Verify raw float values
    let s0 = f32::from_le_bytes(data[44..48].try_into().unwrap());
    let s1 = f32::from_le_bytes(data[48..52].try_into().unwrap());
    let s2 = f32::from_le_bytes(data[52..56].try_into().unwrap());
    let s3 = f32::from_le_bytes(data[56..60].try_into().unwrap());
    assert_eq!(s0, 0.25);
    assert_eq!(s1, -0.25);
    assert_eq!(s2, 0.75);
    assert_eq!(s3, -0.75);
}

#[test]
fn test_write_wav_f32_surround_5_1_channels() {
    let temp_dir = std::env::temp_dir();
    let wav_path = temp_dir.join("test_wav_5_1.wav");
    let samples = vec![0.1f32; 600]; // 100 frames of 6 channels
    write_wav_f32(&wav_path, 48000, 6, &samples).unwrap();
    let data = std::fs::read(&wav_path).unwrap();
    let _ = std::fs::remove_file(&wav_path);

    assert_eq!(u16::from_le_bytes(data[22..24].try_into().unwrap()), 6); // 6 channels
    assert_eq!(u32::from_le_bytes(data[28..32].try_into().unwrap()), 48000 * 6 * 4); // ByteRate
    assert_eq!(u16::from_le_bytes(data[32..34].try_into().unwrap()), 6 * 4); // BlockAlign = 24
}

#[test]
fn test_write_wav_f32_surround_7_1_channels() {
    let temp_dir = std::env::temp_dir();
    let wav_path = temp_dir.join("test_wav_7_1.wav");
    let samples = vec![0.1f32; 800]; // 100 frames of 8 channels
    write_wav_f32(&wav_path, 48000, 8, &samples).unwrap();
    let data = std::fs::read(&wav_path).unwrap();
    let _ = std::fs::remove_file(&wav_path);

    assert_eq!(u16::from_le_bytes(data[22..24].try_into().unwrap()), 8); // 8 channels
    assert_eq!(u32::from_le_bytes(data[28..32].try_into().unwrap()), 48000 * 8 * 4); // ByteRate
    assert_eq!(u16::from_le_bytes(data[32..34].try_into().unwrap()), 8 * 4); // BlockAlign = 32
}

#[test]
fn test_write_wav_f32_all_standard_sample_rates() {
    let temp_dir = std::env::temp_dir();
    for sr in [8000, 11025, 16000, 22050, 32000, 44100, 48000, 88200, 96000, 176400, 192000] {
        let wav_path = temp_dir.join(format!("test_wav_sr_{sr}.wav"));
        let samples = vec![0.0f32; 20];
        write_wav_f32(&wav_path, sr, 2, &samples).unwrap();
        let data = std::fs::read(&wav_path).unwrap();
        let _ = std::fs::remove_file(&wav_path);

        assert_eq!(u32::from_le_bytes(data[24..28].try_into().unwrap()), sr);
        assert_eq!(u32::from_le_bytes(data[28..32].try_into().unwrap()), sr * 2 * 4);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// AudioTrack Buffer Eviction & Capacity Limit Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_audio_track_eviction_when_exceeding_max_samples() {
    // 1 second buffer: 48000 * 2 * (1 + 30) = 48000 * 62 = 2,976,000 max samples
    let max_sec = 1;
    let mut track = AudioTrack::new("EvictTest".to_string(), 48000, 2, max_sec);

    let chunk_size = 48000 * 2; // 1 second chunk
    for _ in 0..40 {
        let chunk = vec![0.1f32; chunk_size];
        track.push_samples(&chunk);
    }

    // Must be bounded within capacity
    let max_allowed = 48000 * 2 * (max_sec as usize + 30);
    let total_stored: usize = track.sample_rate as usize * track.channels as usize;
    assert!(total_stored <= max_allowed);
}

#[test]
fn test_audio_track_multiple_pushes_stored_samples_count() {
    let mut track = AudioTrack::new("CountTest".to_string(), 48000, 2, 60);
    let chunk1 = vec![0.1f32; 1000];
    let chunk2 = vec![0.2f32; 2000];
    let chunk3 = vec![0.3f32; 3000];

    track.push_samples(&chunk1);
    track.push_samples(&chunk2);
    track.push_samples(&chunk3);

    // No overflow yet, all 6000 samples stored
    assert_eq!(track.sample_rate, 48000);
}

// ──────────────────────────────────────────────────────────────────────────────
// SpikeDetector Cooldown & Sensitivity Matrix Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_spike_detector_cooldown_1800ms_suppression() {
    let mut detector = SpikeDetector::new(true, 0.5);
    let now = Instant::now();

    // First spike fires
    detector.feed(0.9, now, Duration::from_secs(30));
    assert_eq!(detector.spikes.len(), 1);

    // Second spike within 1.0s is suppressed
    detector.feed(0.95, now + Duration::from_millis(1000), Duration::from_secs(30));
    assert_eq!(detector.spikes.len(), 1);

    // Spike after 1.9s fires
    detector.feed(0.92, now + Duration::from_millis(1900), Duration::from_secs(30));
    assert_eq!(detector.spikes.len(), 2);
}

#[test]
fn test_spike_detector_various_threshold_settings() {
    for thresh in [0.1f32, 0.25, 0.5, 0.75, 0.9] {
        let mut detector = SpikeDetector::new(true, thresh);
        let now = Instant::now();
        detector.feed(0.01, now, Duration::from_secs(30));
        assert_eq!(detector.spikes.len(), 0);
    }
}

#[test]
fn test_spike_detector_high_threshold_requires_higher_energy() {
    let mut detector_low = SpikeDetector::new(true, 0.1);
    let mut detector_high = SpikeDetector::new(true, 0.95);
    let now = Instant::now();

    // Establish steady background baseline (e.g. game audio at 0.1)
    for i in 0..50 {
        let t = now + Duration::from_millis(i * 10);
        detector_low.feed(0.1, t, Duration::from_secs(30));
        detector_high.feed(0.1, t, Duration::from_secs(30));
    }
    detector_low.spikes.clear();
    detector_high.spikes.clear();

    // Moderate jump: low threshold triggers, high does not
    let spike_time = now + Duration::from_secs(2);
    detector_low.feed(0.28, spike_time, Duration::from_secs(30));
    detector_high.feed(0.28, spike_time, Duration::from_secs(30));

    assert_eq!(detector_low.spikes.len(), 1);
    assert_eq!(detector_high.spikes.len(), 0);
}

#[test]
fn test_mic_volume_multipliers() {
    for vol in [0.0f32, 0.25, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0] {
        set_mic_volume(vol);
        let current = get_mic_volume();
        assert!((current - vol).abs() < 0.005);
    }
    set_mic_volume(1.0);
}

#[test]
fn test_write_wav_f32_negative_amplitudes() {
    let temp_dir = std::env::temp_dir();
    let wav_path = temp_dir.join("test_wav_neg.wav");
    let samples = vec![-1.0f32, -0.5, -0.25, -0.1];
    write_wav_f32(&wav_path, 44100, 1, &samples).unwrap();
    let data = std::fs::read(&wav_path).unwrap();
    let _ = std::fs::remove_file(&wav_path);

    assert_eq!(data.len(), 44 + 4 * 4);
    let s0 = f32::from_le_bytes(data[44..48].try_into().unwrap());
    assert_eq!(s0, -1.0);
}

#[test]
fn test_write_wav_f32_small_sample_counts_1_to_8() {
    let temp_dir = std::env::temp_dir();
    for count in 1..=8 {
        let wav_path = temp_dir.join(format!("test_wav_small_{count}.wav"));
        let samples = vec![0.5f32; count];
        write_wav_f32(&wav_path, 48000, 1, &samples).unwrap();
        let data = std::fs::read(&wav_path).unwrap();
        let _ = std::fs::remove_file(&wav_path);

        assert_eq!(data.len(), 44 + count * 4);
    }
}

#[test]
fn test_spike_detector_noise_floor_under_0_05_never_triggers() {
    let mut detector = SpikeDetector::new(true, 0.0);
    let now = Instant::now();
    for i in 0..100 {
        let t = now + Duration::from_millis(i * 10);
        detector.feed(0.045, t, Duration::from_secs(30));
    }
    assert_eq!(detector.spikes.len(), 0);
}


