use app_lib::audio_engine::{AudioTrack, SpikeDetector, write_wav_f32};
use std::time::{Instant, Duration};

// ──────────────────────────────────────────────────────────────────────────────
// Sample Rate Exhaustive Matrix
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_audio_track_all_supported_sample_rates() {
    let rates = [
        8000, 11025, 12000, 16000, 22050, 24000, 32000,
        44100, 48000, 64000, 88200, 96000, 128000, 176400, 192000,
    ];

    for sr in rates {
        let t = AudioTrack::new("TestTrack".to_string(), sr, 2, 60);
        assert_eq!(t.sample_rate, sr);
        assert_eq!(t.channels, 2);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Channel Layout Matrix (Mono, Stereo, 2.1, 4.0, 5.1, 7.1)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_audio_track_channel_layouts_1_to_8() {
    for ch in 1..=8u16 {
        let mut t = AudioTrack::new("Multichannel".to_string(), 48000, ch, 30);
        let sample_block = vec![0.1f32; ch as usize * 100]; // 100 frames
        t.push_samples(&sample_block);

        let now = Instant::now();
        let stitched = t.stitch_to_window(now - Duration::from_millis(500), now, 0);
        assert!(!stitched.is_empty());
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// WAV Header Byte-Level Verification
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_wav_f32_header_byte_structure() {
    let temp_path = std::env::temp_dir().join("test_wav_f32_header_byte_structure.wav");
    let samples = vec![0.0f32, 0.5f32, -0.5f32, 1.0f32];
    write_wav_f32(&temp_path, 48000, 2, &samples).unwrap();
    let wav = std::fs::read(&temp_path).unwrap();
    let _ = std::fs::remove_file(&temp_path);

    // Header length must be 44 bytes + 16 bytes data = 60
    assert!(wav.len() >= 44);

    // RIFF chunk descriptor
    assert_eq!(&wav[0..4], b"RIFF");
    // WAVE format
    assert_eq!(&wav[8..12], b"WAVE");
    // fmt subchunk
    assert_eq!(&wav[12..16], b"fmt ");
    // Subchunk1Size = 16
    assert_eq!(u32::from_le_bytes([wav[16], wav[17], wav[18], wav[19]]), 16);
    // AudioFormat = 3 (IEEE Float)
    assert_eq!(u16::from_le_bytes([wav[20], wav[21]]), 3);
    // NumChannels = 2
    assert_eq!(u16::from_le_bytes([wav[22], wav[23]]), 2);
    // SampleRate = 48000
    assert_eq!(u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]), 48000);
    // ByteRate = 48000 * 2 * 4 = 384000
    assert_eq!(u32::from_le_bytes([wav[28], wav[29], wav[30], wav[31]]), 384000);
    // BlockAlign = 2 * 4 = 8
    assert_eq!(u16::from_le_bytes([wav[32], wav[33]]), 8);
    // BitsPerSample = 32
    assert_eq!(u16::from_le_bytes([wav[34], wav[35]]), 32);
    // data subchunk
    assert_eq!(&wav[36..40], b"data");
    // Subchunk2Size = 4 samples * 4 bytes = 16
    assert_eq!(u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]), 16);
}

// ──────────────────────────────────────────────────────────────────────────────
// Spike Detector EMA and Threshold Sensitivity Matrix
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_spike_detector_threshold_sensitivity_gradient() {
    let thresholds = [1.5f32, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0];
    for &th in &thresholds {
        let mut sd = SpikeDetector::new(true, th);
        let now = Instant::now();
        let max_age = Duration::from_secs(60);

        // Baseline silence/noise floor
        for i in 0..50 {
            sd.feed(0.02, now + Duration::from_millis(i * 50), max_age);
        }
        assert_eq!(sd.spikes.len(), 0, "No spike on baseline for threshold {}", th);

        // Loud transient of 0.8
        let hit_time = now + Duration::from_millis(3000);
        sd.feed(0.85, hit_time, max_age);

        // For all reasonable thresholds, 0.85 should easily trigger over 0.02
        assert_eq!(sd.spikes.len(), 1, "Expected spike trigger for threshold {}", th);
    }
}

#[test]
fn test_spike_detector_disabled_never_records_spikes() {
    let mut sd = SpikeDetector::new(false, 1.5);
    let now = Instant::now();
    let max_age = Duration::from_secs(60);

    for i in 0..100 {
        sd.feed(1.0, now + Duration::from_millis(i * 100), max_age);
    }
    assert_eq!(sd.spikes.len(), 0, "Disabled SpikeDetector must never store spikes");
}

#[test]
fn test_spike_detector_decay_prunes_older_spikes() {
    let mut sd = SpikeDetector::new(true, 2.0);
    let start = Instant::now();
    let max_age = Duration::from_secs(5); // 5 second window

    // Spike at t=0
    sd.feed(0.9, start, max_age);
    assert_eq!(sd.spikes.len(), 1);

    // Feed at t=10s (exceeds max_age of 5s)
    sd.feed(0.01, start + Duration::from_secs(10), max_age);
    assert_eq!(sd.spikes.len(), 0, "Old spike at t=0 should be pruned after 10s");
}
