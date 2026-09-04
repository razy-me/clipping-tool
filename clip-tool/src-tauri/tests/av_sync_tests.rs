use std::time::{Duration, Instant};
use app_lib::audio_engine::AudioTrack;

// ──────────────────────────────────────────────────────────────────────────────
// A/V Synchronization & Sample-Accurate Timeline Alignment Test Suite
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_av_sync_sound_starts_immediately_at_second_zero() {
    let t0 = Instant::now();
    let win_start = t0;
    let win_end = t0 + Duration::from_secs(5); // 5.0 second video clip

    let mut track = AudioTrack::new("GameAudio".to_string(), 48000, 2, 60);

    // Audio stream starts immediately at t = 0.0s and delivers continuous 100ms chunks (4800 frames = 9600 floats)
    let chunk_frames = 4800;
    for i in 1..=50 {
        let chunk_data = vec![0.75f32; chunk_frames * 2];
        let arrival = t0 + Duration::from_millis(i * 100);
        track.push_chunk_with_timestamp(&chunk_data, arrival);
    }

    let stitched = track.stitch_to_window(win_start, win_end, 0);

    // Total length must be exactly 5.0 * 48000 * 2 = 480,000 samples
    assert_eq!(stitched.len(), 5 * 48000 * 2);

    // First sample at t = 0:00 must be audio (0.75), not silence
    assert_eq!(stitched[0], 0.75);
    assert_eq!(stitched[1], 0.75);

    // Entire stream should be populated with audio
    for sample in &stitched {
        assert_eq!(*sample, 0.75);
    }
}

#[test]
fn test_av_sync_sound_starts_after_x_seconds_2_5s() {
    let t0 = Instant::now();
    let win_start = t0;
    let win_end = t0 + Duration::from_secs(5); // 5.0 second video clip

    let mut track = AudioTrack::new("DelayedVoice".to_string(), 48000, 2, 60);

    // User only speaks after X = 2.5 seconds (t = 2.5s .. 5.0s)
    let chunk_frames = 4800; // 100ms
    for i in 26..=50 { // 2.6s to 5.0s
        let chunk_data = vec![0.9f32; chunk_frames * 2];
        let arrival = t0 + Duration::from_millis(i * 100);
        track.push_chunk_with_timestamp(&chunk_data, arrival);
    }

    let stitched = track.stitch_to_window(win_start, win_end, 0);

    // Total length must be exact 5.0s
    assert_eq!(stitched.len(), 5 * 48000 * 2);

    // Exactly the first 2.5s (2.5 * 48000 * 2 = 240,000 samples) must be silence (0.0)
    let silent_samples = (2.5 * 48000.0 * 2.0) as usize;
    for (idx, sample) in stitched[..silent_samples].iter().enumerate() {
        assert_eq!(*sample, 0.0, "Sample at index {} before 2.5s should be silence", idx);
    }

    // Audio starting at 2.5s must contain the voice sound (0.9)
    assert_eq!(stitched[silent_samples], 0.9);
    assert_eq!(stitched[silent_samples + 1], 0.9);
    assert_eq!(stitched[stitched.len() - 1], 0.9);
}

#[test]
fn test_av_sync_sound_starts_after_short_delay_300ms() {
    let t0 = Instant::now();
    let win_start = t0;
    let win_end = t0 + Duration::from_secs(3);

    let mut track = AudioTrack::new("ShortDelay".to_string(), 48000, 1, 60); // Mono

    // Sound starts at t = 300ms
    let chunk_frames = 4800; // 100ms
    for i in 4..=30 {
        let chunk_data = vec![0.5f32; chunk_frames];
        let arrival = t0 + Duration::from_millis(i * 100);
        track.push_chunk_with_timestamp(&chunk_data, arrival);
    }

    let stitched = track.stitch_to_window(win_start, win_end, 0);
    assert_eq!(stitched.len(), 3 * 48000);

    // Leading 300ms (14,400 samples) must be silence
    let silence_len = (0.3 * 48000.0) as usize;
    for sample in &stitched[..silence_len] {
        assert_eq!(*sample, 0.0);
    }
    // Samples after 300ms must be sound (0.5)
    assert_eq!(stitched[silence_len], 0.5);
}

#[test]
fn test_av_sync_sound_stops_early_padded_with_silence() {
    let t0 = Instant::now();
    let win_start = t0;
    let win_end = t0 + Duration::from_secs(5);

    let mut track = AudioTrack::new("EarlyStop".to_string(), 48000, 2, 60);

    // Audio plays from t = 0.0s to t = 3.0s, then app closes or goes silent
    let chunk_frames = 4800;
    for i in 1..=30 {
        let chunk_data = vec![0.6f32; chunk_frames * 2];
        let arrival = t0 + Duration::from_millis(i * 100);
        track.push_chunk_with_timestamp(&chunk_data, arrival);
    }

    let stitched = track.stitch_to_window(win_start, win_end, 0);
    assert_eq!(stitched.len(), 5 * 48000 * 2);

    let sound_samples = 3 * 48000 * 2;
    for sample in &stitched[..sound_samples] {
        assert_eq!(*sample, 0.6);
    }
    // Trailing 2 seconds must be silence padded
    for sample in &stitched[sound_samples..] {
        assert_eq!(*sample, 0.0);
    }
}

#[test]
fn test_av_sync_audio_started_before_video_window_pruned() {
    let t0 = Instant::now();
    let win_start = t0; // Video starts at t0
    let win_end = t0 + Duration::from_secs(4);

    let mut track = AudioTrack::new("PreBufferAudio".to_string(), 48000, 1, 60);

    // Audio was already playing for 2 seconds BEFORE video clip start (t = -2.0s to +4.0s)
    let chunk_frames = 4800; // 100ms
    // Chunk from t = -1.0s to 0.0s has value 0.1
    track.push_chunk_with_timestamp(&vec![0.1f32; chunk_frames * 10], t0);
    // Chunk from t = 0.0s to +4.0s has value 0.8
    for i in 1..=40 {
        let arrival = t0 + Duration::from_millis(i * 100);
        track.push_chunk_with_timestamp(&vec![0.8f32; chunk_frames], arrival);
    }

    let stitched = track.stitch_to_window(win_start, win_end, 0);
    assert_eq!(stitched.len(), 4 * 48000);

    // The output starts exactly at win_start: must contain the post-start sound 0.8
    assert_eq!(stitched[0], 0.8);
    assert_eq!(stitched[stitched.len() - 1], 0.8);
}

#[test]
fn test_av_sync_audio_stuttering_jitter_tolerance_40ms() {
    let t0 = Instant::now();
    let win_start = t0;
    let win_end = t0 + Duration::from_secs(5);

    let mut track = AudioTrack::new("JitteryAudio".to_string(), 48000, 1, 60);

    // Audio chunks arrive with jitter (e.g. timers oscillating +/- 20ms)
    // 40ms jitter tolerance must stitch chunks without micro-gaps or losing sync
    let chunk_frames = 4800;
    for i in 1..=50 {
        // Alternating timer jitter +/- 15ms
        let jitter = if i % 2 == 0 { 15i64 } else { -10i64 };
        let ms = ((i * 100) as i64 + jitter).max(0) as u64;
        let current_ts = t0 + Duration::from_millis(ms);
        let chunk_data = vec![0.5f32; chunk_frames];
        track.push_chunk_with_timestamp(&chunk_data, current_ts);
    }

    let stitched = track.stitch_to_window(win_start, win_end, 0);
    assert_eq!(stitched.len(), 5 * 48000);

    // Jitter stitcher smoothly covers the whole span without crackle/silence dropouts
    let non_zero_count = stitched.iter().filter(|&&s| s == 0.5).count();
    let coverage_pct = non_zero_count as f64 / stitched.len() as f64;
    assert!(coverage_pct > 0.95, "Jitter tolerance should maintain >95% seamless coverage");
}

#[test]
fn test_av_sync_real_audio_gap_longer_than_jitter_preserved() {
    let t0 = Instant::now();
    let win_start = t0;
    let win_end = t0 + Duration::from_secs(5);

    let mut track = AudioTrack::new("PauseGap".to_string(), 48000, 1, 60);

    // Sound from t = 0.0s to 1.0s (10 chunks)
    for i in 1..=10 {
        let arrival = t0 + Duration::from_millis(i * 100);
        track.push_chunk_with_timestamp(&vec![0.4f32; 4800], arrival);
    }

    // Real pause of 1.5 seconds (t = 1.0s .. 2.5s) - exceeds 40ms jitter window!

    // Sound resumes at t = 2.5s .. 5.0s (25 chunks)
    for i in 26..=50 {
        let arrival = t0 + Duration::from_millis(i * 100);
        track.push_chunk_with_timestamp(&vec![0.4f32; 4800], arrival);
    }

    let stitched = track.stitch_to_window(win_start, win_end, 0);
    assert_eq!(stitched.len(), 5 * 48000);

    // Sound from 0..1.0s
    assert_eq!(stitched[0], 0.4);
    assert_eq!(stitched[48000 - 1], 0.4);

    // True silence preserved during the 1.5s pause (sample 48,000 to 120,000)
    for sample in &stitched[48000..120000] {
        assert_eq!(*sample, 0.0, "Real pause must remain silence");
    }

    // Sound resumed at 2.5s (sample 120,000)
    assert_eq!(stitched[120000], 0.4);
}

#[test]
fn test_av_sync_audio_sync_offset_positive_calibration() {
    let t0 = Instant::now();
    let win_start = t0;
    let win_end = t0 + Duration::from_secs(3);

    let mut track = AudioTrack::new("OffsetPos".to_string(), 48000, 1, 60);

    // Continuous sound starting at t = 0.0s
    for i in 1..=30 {
        let arrival = t0 + Duration::from_millis(i * 100);
        track.push_chunk_with_timestamp(&vec![0.7f32; 4800], arrival);
    }

    // audio_sync_offset_ms = +100ms shifts window 100ms forward (skips first 100ms of audio, leaves 100ms trailing padding)
    let stitched = track.stitch_to_window(win_start, win_end, 100);
    assert_eq!(stitched.len(), 3 * 48000);
    assert_eq!(stitched[0], 0.7);
    assert_eq!(stitched[3 * 48000 - 4801], 0.7); // Last active sample before 2.9s
    assert_eq!(stitched[3 * 48000 - 1], 0.0); // Trailing 100ms padded silence
}

#[test]
fn test_av_sync_audio_sync_offset_negative_calibration() {
    let t0 = Instant::now();
    let win_start = t0;
    let win_end = t0 + Duration::from_secs(3);

    let mut track = AudioTrack::new("OffsetNeg".to_string(), 48000, 1, 60);

    // Sound starting at t = 200ms (chunks from 200ms to 3000ms)
    for i in 3..=30 {
        let arrival = t0 + Duration::from_millis(i * 100);
        track.push_chunk_with_timestamp(&vec![0.7f32; 4800], arrival);
    }

    // audio_sync_offset_ms = -100ms shifts window 100ms earlier -> sound starting at 200ms is placed at 300ms (14,400 samples)
    let stitched = track.stitch_to_window(win_start, win_end, -100);
    assert_eq!(stitched.len(), 3 * 48000);

    let silence_len = (0.3 * 48000.0) as usize; // 14,400 samples
    for sample in &stitched[..silence_len] {
        assert_eq!(*sample, 0.0);
    }
    assert_eq!(stitched[silence_len], 0.7);
}

#[test]
fn test_av_sync_multi_track_sample_parity() {
    let t0 = Instant::now();
    let win_start = t0;
    let win_end = t0 + Duration::from_secs(5);

    // Three tracks with completely different start times and channels:
    let mut system_track = AudioTrack::new("System".to_string(), 48000, 2, 60);     // Starts at 0.0s (stereo)
    let mut mic_track = AudioTrack::new("Microphone".to_string(), 48000, 1, 60);     // Starts at 1.5s (mono)
    let mut discord_track = AudioTrack::new("Discord.exe".to_string(), 48000, 2, 60); // Starts at 3.2s (stereo)

    // System: 0..5s
    for i in 1..=50 {
        system_track.push_chunk_with_timestamp(&vec![0.2f32; 4800 * 2], t0 + Duration::from_millis(i * 100));
    }
    // Mic: 1.5..5s
    for i in 16..=50 {
        mic_track.push_chunk_with_timestamp(&vec![0.8f32; 4800], t0 + Duration::from_millis(i * 100));
    }
    // Discord: 3.2..5s
    for i in 33..=50 {
        discord_track.push_chunk_with_timestamp(&vec![0.5f32; 4800 * 2], t0 + Duration::from_millis(i * 100));
    }

    let sys_stitched = system_track.stitch_to_window(win_start, win_end, 0);
    let mic_stitched = mic_track.stitch_to_window(win_start, win_end, 0);
    let dis_stitched = discord_track.stitch_to_window(win_start, win_end, 0);

    // Frame parity check: all tracks have exactly 5.0 * 48000 = 240,000 frames
    assert_eq!(sys_stitched.len() / 2, 240000);
    assert_eq!(mic_stitched.len() / 1, 240000);
    assert_eq!(dis_stitched.len() / 2, 240000);
}

#[test]
fn test_av_sync_high_res_96khz_sample_accuracy() {
    let t0 = Instant::now();
    let win_start = t0;
    let win_end = t0 + Duration::from_secs(4);

    let mut track = AudioTrack::new("HighResDAC".to_string(), 96000, 2, 60);

    // 96kHz stream: 9600 frames per 100ms chunk (19,200 floats)
    for i in 1..=40 {
        track.push_chunk_with_timestamp(&vec![0.3f32; 9600 * 2], t0 + Duration::from_millis(i * 100));
    }

    let stitched = track.stitch_to_window(win_start, win_end, 0);
    assert_eq!(stitched.len(), 4 * 96000 * 2); // 768,000 samples
    assert_eq!(stitched[0], 0.3);
}

#[test]
fn test_av_sync_impulse_event_time_precision_sub_millisecond() {
    let t0 = Instant::now();
    let win_start = t0;
    let win_end = t0 + Duration::from_secs(5);

    let mut track = AudioTrack::new("ImpulseTest".to_string(), 48000, 1, 60);

    // Continuous quiet background (0.01)
    let chunk_frames = 4800; // 100ms
    for i in 1..=50 {
        let mut chunk = vec![0.01f32; chunk_frames];
        // Inject single-sample Dirac delta impulse (1.0) exactly at t = 2.345s (in chunk 24 at index 2160)
        // 2.345s = 2.3s (start of chunk 24) + 45ms (0.045 * 48000 = 2160 samples)
        if i == 24 {
            chunk[2160] = 1.0;
        }
        track.push_chunk_with_timestamp(&chunk, t0 + Duration::from_millis(i * 100));
    }

    let stitched = track.stitch_to_window(win_start, win_end, 0);
    assert_eq!(stitched.len(), 5 * 48000);

    // The impulse at t = 2.345s must be located at exact sample 2.345 * 48000 = 112,560
    let target_sample = (2.345f64 * 48000.0f64).round() as usize;
    assert_eq!(stitched[target_sample], 1.0, "Impulse event must have sub-millisecond precision");
}
