use app_lib::editor::compute_wav_peaks;
use std::io::Write;

fn write_test_wav_f32(path: &std::path::Path, sample_rate: u32, channels: u16, samples: &[f32]) {
    let mut file = std::fs::File::create(path).unwrap();
    let bytes_per_sample = 4u16;
    let block_align = channels * bytes_per_sample;
    let byte_rate = sample_rate * block_align as u32;
    let data_len = (samples.len() * 4) as u32;
    let riff_len = 36 + data_len;

    // RIFF header
    file.write_all(b"RIFF").unwrap();
    file.write_all(&riff_len.to_le_bytes()).unwrap();
    file.write_all(b"WAVE").unwrap();

    // fmt chunk (IEEE float = 3)
    file.write_all(b"fmt ").unwrap();
    file.write_all(&16u32.to_le_bytes()).unwrap();
    file.write_all(&3u16.to_le_bytes()).unwrap();
    file.write_all(&channels.to_le_bytes()).unwrap();
    file.write_all(&sample_rate.to_le_bytes()).unwrap();
    file.write_all(&byte_rate.to_le_bytes()).unwrap();
    file.write_all(&block_align.to_le_bytes()).unwrap();
    file.write_all(&32u16.to_le_bytes()).unwrap();

    // data chunk
    file.write_all(b"data").unwrap();
    file.write_all(&data_len.to_le_bytes()).unwrap();
    for s in samples {
        file.write_all(&s.to_le_bytes()).unwrap();
    }
}

#[test]
fn test_compute_wav_peaks_valid_mono_file() {
    let temp_dir = std::env::temp_dir();
    let wav_path = temp_dir.join("test_peaks_mono.wav");

    let mut samples = Vec::with_capacity(48000);
    for i in 0..48000 {
        let val = (i as f32 * 0.05).sin() * 0.8;
        samples.push(val);
    }
    write_test_wav_f32(&wav_path, 48000, 1, &samples);

    let peaks = compute_wav_peaks(&wav_path, 500).expect("compute_wav_peaks failed");
    let _ = std::fs::remove_file(&wav_path);

    assert_eq!(peaks.len(), 500);
    for p in &peaks {
        assert!(*p >= 0.0 && *p <= 1.0);
    }
}

#[test]
fn test_compute_wav_peaks_valid_stereo_file() {
    let temp_dir = std::env::temp_dir();
    let wav_path = temp_dir.join("test_peaks_stereo.wav");

    let mut samples = Vec::with_capacity(96000);
    for _i in 0..48000 {
        samples.push(0.5); // Left
        samples.push(-0.7); // Right
    }
    write_test_wav_f32(&wav_path, 48000, 2, &samples);

    let peaks = compute_wav_peaks(&wav_path, 200).expect("compute_wav_peaks failed");
    let _ = std::fs::remove_file(&wav_path);

    assert_eq!(peaks.len(), 200);
    for p in &peaks {
        assert!(*p > 0.0 && *p <= 1.0, "Got peak value {}", p);
    }
}

#[test]
fn test_compute_wav_peaks_bucket_sizes() {
    let temp_dir = std::env::temp_dir();
    let wav_path = temp_dir.join("test_peaks_buckets.wav");
    let samples = vec![0.2f32; 48000];
    write_test_wav_f32(&wav_path, 48000, 1, &samples);

    for b in [10, 50, 100, 250, 500, 1000] {
        let peaks = compute_wav_peaks(&wav_path, b).unwrap();
        assert_eq!(peaks.len(), b);
    }
    let _ = std::fs::remove_file(&wav_path);
}

#[test]
fn test_compute_wav_peaks_empty_file() {
    let temp_dir = std::env::temp_dir();
    let empty_path = temp_dir.join("test_empty.wav");
    std::fs::write(&empty_path, b"").unwrap();
    assert!(compute_wav_peaks(&empty_path, 100).is_err());
    let _ = std::fs::remove_file(&empty_path);
}

#[test]
fn test_compute_wav_peaks_corrupted_header() {
    let temp_dir = std::env::temp_dir();
    let corrupt_path = temp_dir.join("test_corrupt.wav");
    std::fs::write(&corrupt_path, b"RIFF1234NOTWAVE").unwrap();
    assert!(compute_wav_peaks(&corrupt_path, 100).is_err());
    let _ = std::fs::remove_file(&corrupt_path);
}

#[test]
fn test_compute_wav_peaks_no_data_chunk() {
    let temp_dir = std::env::temp_dir();
    let nodata_path = temp_dir.join("test_nodata.wav");
    let mut f = std::fs::File::create(&nodata_path).unwrap();
    f.write_all(b"RIFF\x24\x00\x00\x00WAVEfmt \x10\x00\x00\x00\x03\x00\x01\x00\x80\xbb\x00\x00\x00\xee\x02\x00\x04\x00\x20\x00").unwrap();
    drop(f);
    assert!(compute_wav_peaks(&nodata_path, 100).is_err());
    let _ = std::fs::remove_file(&nodata_path);
}

#[test]
fn test_versioned_output_path_simple() {
    let base = std::path::Path::new("C:/Clips/clip.mp4");
    let stem = base.file_stem().unwrap().to_string_lossy();
    let parent = base.parent().unwrap();
    assert_eq!(parent.join(format!("{stem}_edited.mp4")).to_string_lossy().replace('\\', "/"), "C:/Clips/clip_edited.mp4");
}

#[test]
fn test_versioned_output_path_with_dots_in_folder() {
    let base = std::path::Path::new("C:/Games.Vault.2026/clip.mp4");
    let stem = base.file_stem().unwrap().to_string_lossy();
    let parent = base.parent().unwrap();
    assert_eq!(parent.join(format!("{stem}_edited.mp4")).to_string_lossy().replace('\\', "/"), "C:/Games.Vault.2026/clip_edited.mp4");
}

#[test]
fn test_versioned_output_path_with_spaces_and_hyphens() {
    let base = std::path::Path::new("D:/My Awesome Recordings/Game - 2026-08-31.mp4");
    let stem = base.file_stem().unwrap().to_string_lossy();
    let parent = base.parent().unwrap();
    assert_eq!(parent.join(format!("{stem}_edited.mp4")).to_string_lossy().replace('\\', "/"), "D:/My Awesome Recordings/Game - 2026-08-31_edited.mp4");
}

#[test]
fn test_versioned_output_path_increment_loop() {
    let base = std::path::Path::new("C:/Clips/clip.mp4");
    let stem = base.file_stem().unwrap().to_string_lossy();
    let parent = base.parent().unwrap();
    
    for n in 2..=10 {
        let out = parent.join(format!("{stem}_edited_{n}.mp4")).to_string_lossy().replace('\\', "/");
        assert_eq!(out, format!("C:/Clips/clip_edited_{n}.mp4"));
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Advanced Waveform & Audio Peak Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_compute_wav_peaks_all_zero_silence() {
    let temp_dir = std::env::temp_dir();
    let wav_path = temp_dir.join("test_peaks_silence.wav");
    let samples = vec![0.0f32; 48000];
    write_test_wav_f32(&wav_path, 48000, 1, &samples);

    let peaks = compute_wav_peaks(&wav_path, 100).expect("compute_wav_peaks failed");
    let _ = std::fs::remove_file(&wav_path);

    assert_eq!(peaks.len(), 100);
    for p in &peaks {
        assert_eq!(*p, 0.0);
    }
}

#[test]
fn test_compute_wav_peaks_surround_5_1_channels() {
    let temp_dir = std::env::temp_dir();
    let wav_path = temp_dir.join("test_peaks_5_1.wav");
    let mut samples = Vec::new();
    for _ in 0..1000 {
        samples.extend_from_slice(&[0.1, 0.2, 0.3, 0.4, 0.5, 0.6]); // 6 channels
    }
    write_test_wav_f32(&wav_path, 48000, 6, &samples);

    let peaks = compute_wav_peaks(&wav_path, 50).expect("compute_wav_peaks 5.1 failed");
    let _ = std::fs::remove_file(&wav_path);

    assert_eq!(peaks.len(), 50);
    for p in &peaks {
        assert!(*p > 0.0 && *p <= 1.0);
    }
}

#[test]
fn test_compute_wav_peaks_high_sample_rate_96k() {
    let temp_dir = std::env::temp_dir();
    let wav_path = temp_dir.join("test_peaks_96k.wav");
    let samples = vec![0.4f32; 96000];
    write_test_wav_f32(&wav_path, 96000, 2, &samples);

    let peaks = compute_wav_peaks(&wav_path, 200).expect("compute_wav_peaks 96k failed");
    let _ = std::fs::remove_file(&wav_path);

    assert_eq!(peaks.len(), 200);
}

#[test]
fn test_compute_wav_peaks_high_sample_rate_192k() {
    let temp_dir = std::env::temp_dir();
    let wav_path = temp_dir.join("test_peaks_192k.wav");
    let samples = vec![0.3f32; 192000];
    write_test_wav_f32(&wav_path, 192000, 2, &samples);

    let peaks = compute_wav_peaks(&wav_path, 300).expect("compute_wav_peaks 192k failed");
    let _ = std::fs::remove_file(&wav_path);

    assert_eq!(peaks.len(), 300);
}

#[test]
fn test_compute_wav_peaks_extreme_buckets_50_to_1000() {
    let temp_dir = std::env::temp_dir();
    let wav_path = temp_dir.join("test_peaks_extremes.wav");
    let samples = vec![0.5f32; 48000];
    write_test_wav_f32(&wav_path, 48000, 1, &samples);

    for b in [50, 100, 250, 500, 750, 1000] {
        let peaks = compute_wav_peaks(&wav_path, b).unwrap();
        assert_eq!(peaks.len(), b);
    }
    let _ = std::fs::remove_file(&wav_path);
}

// ──────────────────────────────────────────────────────────────────────────────
// Path Versioning Unicode & Complex Pattern Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_versioned_output_path_german_umlauts() {
    let base = std::path::Path::new("C:/Videos/Höhepunkte_Überraschung.mp4");
    let stem = base.file_stem().unwrap().to_string_lossy();
    let parent = base.parent().unwrap();
    assert_eq!(parent.join(format!("{stem}_edited.mp4")).to_string_lossy().replace('\\', "/"), "C:/Videos/Höhepunkte_Überraschung_edited.mp4");
}

#[test]
fn test_versioned_output_path_with_brackets_and_tags() {
    let base = std::path::Path::new("D:/Clips/[2026] Game #1 (1080p60).mp4");
    let stem = base.file_stem().unwrap().to_string_lossy();
    let parent = base.parent().unwrap();
    assert_eq!(parent.join(format!("{stem}_edited.mp4")).to_string_lossy().replace('\\', "/"), "D:/Clips/[2026] Game #1 (1080p60)_edited.mp4");
}

#[test]
fn test_versioned_output_path_multiple_extensions() {
    let base = std::path::Path::new("C:/Clips/game.backup.clip.mp4");
    let stem = base.file_stem().unwrap().to_string_lossy();
    let parent = base.parent().unwrap();
    assert_eq!(parent.join(format!("{stem}_edited.mp4")).to_string_lossy().replace('\\', "/"), "C:/Clips/game.backup.clip_edited.mp4");
}

#[test]
fn test_versioned_output_path_nested_deep_directories() {
    let base = std::path::Path::new("C:/Users/User/Videos/Clips/2026/August/31/Recording.mp4");
    let stem = base.file_stem().unwrap().to_string_lossy();
    let parent = base.parent().unwrap();
    assert_eq!(parent.join(format!("{stem}_edited.mp4")).to_string_lossy().replace('\\', "/"), "C:/Users/User/Videos/Clips/2026/August/31/Recording_edited.mp4");
}

#[test]
fn test_versioned_output_path_various_video_extensions() {
    for ext in ["mp4", "mkv", "mov", "webm", "avi"] {
        let base_str = format!("C:/Clips/video.{ext}");
        let base = std::path::Path::new(&base_str);
        let stem = base.file_stem().unwrap().to_string_lossy();
        let parent = base.parent().unwrap();
        assert_eq!(
            parent.join(format!("{stem}_edited.{ext}")).to_string_lossy().replace('\\', "/"),
            format!("C:/Clips/video_edited.{ext}")
        );
    }
}

#[test]
fn test_compute_wav_peaks_various_amplitudes_normalized_to_one() {
    let temp_dir = std::env::temp_dir();
    for amp in [0.05f32, 0.2, 0.5, 0.8, 1.0] {
        let wav_path = temp_dir.join(format!("test_peaks_amp_{amp}.wav"));
        let samples = vec![amp; 4800];
        write_test_wav_f32(&wav_path, 48000, 1, &samples);

        let peaks = compute_wav_peaks(&wav_path, 10).unwrap();
        let _ = std::fs::remove_file(&wav_path);

        assert_eq!(peaks.len(), 10);
        for p in &peaks {
            // compute_wav_peaks normalizes max peak to 1.0
            assert!((*p - 1.0).abs() < 0.01, "Expected normalized peak 1.0, got {}", p);
        }
    }
}

#[test]
fn test_compute_wav_peaks_stereo_left_right_isolated_channels() {
    let temp_dir = std::env::temp_dir();
    let wav_path = temp_dir.join("test_peaks_isolated_ch.wav");
    let mut samples = Vec::new();
    for _ in 0..2400 {
        samples.push(0.8f32); // Left
        samples.push(0.0f32); // Right
    }
    write_test_wav_f32(&wav_path, 48000, 2, &samples);

    let peaks = compute_wav_peaks(&wav_path, 10).unwrap();
    let _ = std::fs::remove_file(&wav_path);

    assert_eq!(peaks.len(), 10);
    for p in &peaks {
        assert!((*p - 1.0).abs() < 0.01);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// FFmpeg Export Filter Complex Graph Builder Tests
// ──────────────────────────────────────────────────────────────────────────────

fn build_mock_filter_chain(tracks: &[(usize, f64)], fade_in: Option<f64>, fade_out: Option<f64>, duration: f64) -> String {
    let mut chain = String::new();
    let cnt = tracks.len();
    if cnt == 0 {
        chain.push_str("[0:a]anull");
    } else {
        for (i, (_, vol)) in tracks.iter().enumerate() {
            chain.push_str(&format!("[{}:a]volume={:.3}[a{}];", i + 1, vol, i));
        }
        for i in 0..cnt { chain.push_str(&format!("[a{i}]")); }
        if cnt == 1 {
            chain.push_str("anull");
        } else {
            chain.push_str(&format!("amix=inputs={cnt}:duration=longest:normalize=0"));
        }
    }
    let mut post: Vec<String> = Vec::new();
    let fi = fade_in.unwrap_or(0.0).clamp(0.0, duration);
    let fo = fade_out.unwrap_or(0.0).clamp(0.0, duration);
    if fi > 0.0 { post.push(format!("afade=t=in:st=0:d={fi:.3}")); }
    if fo > 0.0 { post.push(format!("afade=t=out:st={:.3}:d={fo:.3}", (duration - fo).max(0.0))); }
    post.push("apad".into());
    chain.push(',');
    chain.push_str(&post.join(","));
    chain.push_str("[aout]");
    chain
}

#[test]
fn test_export_filter_zero_tracks_falls_back_to_main_video_audio() {
    let chain = build_mock_filter_chain(&[], None, None, 10.0);
    assert_eq!(chain, "[0:a]anull,apad[aout]");
}

#[test]
fn test_export_filter_single_track_volume_scaling() {
    let chain = build_mock_filter_chain(&[(0, 1.5)], None, None, 10.0);
    assert!(chain.contains("[1:a]volume=1.500[a0];[a0]anull"));
    assert!(chain.ends_with(",apad[aout]"));
}

#[test]
fn test_export_filter_two_tracks_mixing() {
    let chain = build_mock_filter_chain(&[(0, 1.0), (1, 2.0)], None, None, 10.0);
    assert!(chain.contains("[1:a]volume=1.000[a0];[2:a]volume=2.000[a1];"));
    assert!(chain.contains("[a0][a1]amix=inputs=2:duration=longest:normalize=0"));
    assert!(chain.ends_with(",apad[aout]"));
}

#[test]
fn test_export_filter_three_tracks_mixing() {
    let chain = build_mock_filter_chain(&[(0, 1.0), (1, 1.5), (2, 0.8)], None, None, 15.0);
    assert!(chain.contains("[1:a]volume=1.000[a0];[2:a]volume=1.500[a1];[3:a]volume=0.800[a2];"));
    assert!(chain.contains("[a0][a1][a2]amix=inputs=3:duration=longest:normalize=0"));
}

#[test]
fn test_export_filter_fade_in_and_fade_out() {
    let chain = build_mock_filter_chain(&[(0, 1.0)], Some(1.0), Some(2.0), 10.0);
    assert!(chain.contains("afade=t=in:st=0:d=1.000"));
    assert!(chain.contains("afade=t=out:st=8.000:d=2.000"));
    assert!(chain.contains("apad"));
}

#[test]
fn test_export_filter_fade_duration_clamping() {
    // Fade duration exceeding total clip length (10s fade on 5s clip) is clamped to 5s
    let chain = build_mock_filter_chain(&[(0, 1.0)], Some(10.0), None, 5.0);
    assert!(chain.contains("afade=t=in:st=0:d=5.000"));
}



