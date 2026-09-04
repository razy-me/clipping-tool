use app_lib::editor::compute_wav_peaks;

fn versioned_output_path(original: &str) -> String {
    let base = std::path::Path::new(original);
    let parent = base.parent().unwrap_or_else(|| std::path::Path::new(""));
    let stem = base.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default();
    let ext = base.extension().map(|s| s.to_string_lossy()).unwrap_or_default();
    let filename = if ext.is_empty() {
        format!("{stem}_edited")
    } else {
        format!("{stem}_edited.{ext}")
    };
    parent.join(filename).to_string_lossy().into_owned()
}

fn build_audio_export_filter(
    active: &[(&str, f32, bool)],
    fi: f64,
    fo: f64,
    duration: f64,
) -> (String, usize) {
    let unmuted: Vec<(&str, f32)> = active.iter()
        .filter(|(_, _, muted)| !muted)
        .map(|&(name, vol, _)| (name, vol))
        .collect();

    let cnt = unmuted.len();
    if cnt == 0 {
        return (format!("aevalsrc=0:d={duration:.3}[aout]"), 0);
    }

    let mut chain = String::new();
    for (i, (_, vol)) in unmuted.iter().enumerate() {
        chain.push_str(&format!("[{}:a]volume={:.3}[a{}];", i + 1, vol, i));
    }
    for i in 0..cnt {
        chain.push_str(&format!("[a{i}]"));
    }
    if cnt == 1 {
        chain.push_str("anull");
    } else {
        chain.push_str(&format!("amix=inputs={cnt}:duration=longest:normalize=0"));
    }
    let mut post: Vec<String> = Vec::new();
    if fi > 0.0 {
        post.push(format!("afade=t=in:st=0:d={fi:.3}"));
    }
    if fo > 0.0 {
        post.push(format!("afade=t=out:st={:.3}:d={fo:.3}", (duration - fo).max(0.0)));
    }
    post.push("apad".into());
    chain.push(',');
    chain.push_str(&post.join(","));
    chain.push_str("[aout]");

    (chain, cnt)
}

// ──────────────────────────────────────────────────────────────────────────────
// Audio Filter Generation Matrix (1 to 6 Tracks, Mute States, Volumes)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_editor_single_track_various_volumes() {
    let volumes = [0.0, 0.1, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0];
    for v in volumes {
        let (filter, track_count) = build_audio_export_filter(&[("System", v, false)], 0.0, 0.0, 30.0);
        assert_eq!(track_count, 1);
        assert!(filter.contains(&format!("volume={:.3}", v)));
    }
}

#[test]
fn test_editor_single_track_muted_returns_silence() {
    let (filter, track_count) = build_audio_export_filter(&[("System", 1.0, true)], 0.0, 0.0, 30.0);
    assert_eq!(track_count, 0);
    assert_eq!(filter, "aevalsrc=0:d=30.000[aout]");
}

#[test]
fn test_editor_two_tracks_mixing() {
    let tracks = [
        ("System", 1.0, false),
        ("Microphone", 1.5, false),
    ];
    let (filter, track_count) = build_audio_export_filter(&tracks, 0.0, 0.0, 30.0);
    assert_eq!(track_count, 2);
    assert!(filter.contains("amix=inputs=2:duration=longest"));
    assert!(filter.contains("[1:a]volume=1.000[a0]"));
    assert!(filter.contains("[2:a]volume=1.500[a1]"));
}

#[test]
fn test_editor_three_tracks_mixing() {
    let tracks = [
        ("System", 0.8, false),
        ("Microphone", 1.0, false),
        ("Discord", 1.2, false),
    ];
    let (filter, track_count) = build_audio_export_filter(&tracks, 0.0, 0.0, 60.0);
    assert_eq!(track_count, 3);
    assert!(filter.contains("amix=inputs=3:duration=longest"));
    assert!(filter.contains("[1:a]volume=0.800[a0]"));
    assert!(filter.contains("[2:a]volume=1.000[a1]"));
    assert!(filter.contains("[3:a]volume=1.200[a2]"));
}

#[test]
fn test_editor_fade_in_and_fade_out() {
    let tracks = [("System", 1.0, false)];
    let (filter, _) = build_audio_export_filter(&tracks, 1.5, 2.0, 30.0);
    assert!(filter.contains("afade=t=in:st=0:d=1.500"));
    assert!(filter.contains("afade=t=out:st=28.000:d=2.000"));
}

// ──────────────────────────────────────────────────────────────────────────────
// Versioned Output Path Exhaustive Formatting Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_versioned_output_path_extensions() {
    let exts = ["mp4", "mkv", "mov", "webm", "avi", "ts"];
    for ext in exts {
        let base = format!("C:\\Videos\\Clip.{}", ext);
        let v1 = versioned_output_path(&base);
        assert!(v1.ends_with(&format!("_edited.{}", ext)));
    }
}

#[test]
fn test_versioned_output_path_complex_stems() {
    let inputs = [
        "C:\\Users\\Game\\Desktop 2026.09.02 - 18.23.44.01.DVR.mp4",
        "D:\\Clips\\CS2 (De_Dust2) [Ace] {4K}.mp4",
        "E:\\Media\\Clip_with_dots.v1.0.final.mp4",
        "C:\\Cap\\Clip-With-Dashes-123.mp4",
        "C:\\Cap\\Clip_With_Underscores_456.mp4",
    ];

    for path in inputs {
        let out = versioned_output_path(path);
        assert!(out.contains("_edited"));
        assert!(out.ends_with(".mp4"));
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Waveform Peak Computation Bucket Range Matrix
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_compute_wav_peaks_bucket_sizes_range() {
    let mut samples = Vec::new();
    for i in 0..48000 {
        let v = ((i as f32) / 48000.0 * 100.0 * std::f32::consts::PI).sin();
        samples.push(v);
        samples.push(v); // Stereo
    }
    let temp_dir = std::env::temp_dir();
    let temp_wav = temp_dir.join("test_peaks_matrix.wav");
    app_lib::audio_engine::write_wav_f32(&temp_wav, 48000, 2, &samples).unwrap();

    let bucket_counts = [10, 25, 50, 100, 200, 400, 800, 1200, 1600, 2000];
    for b in bucket_counts {
        let peaks = compute_wav_peaks(&temp_wav, b).unwrap();
        assert_eq!(peaks.len(), b, "Failed for bucket count {}", b);
        for p in &peaks {
            assert!(*p >= 0.0 && *p <= 1.0, "Peak {} out of range 0..1", p);
        }
    }
    let _ = std::fs::remove_file(&temp_wav);
}
