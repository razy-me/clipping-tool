// ──────────────────────────────────────────────────────────────────────────────
// FFmpeg Export Progress Parser & Telemetry Test Suite
// ──────────────────────────────────────────────────────────────────────────────

fn parse_progress_chunk(buf: &mut String, dur_ms: f64) -> Vec<f32> {
    let mut progress_updates = Vec::new();
    while let Some(pos) = buf.find('\n') {
        let line: String = buf.drain(..=pos).collect();
        if let Some(rest) = line.trim().strip_prefix("out_time_us=") {
            if let Ok(us) = rest.parse::<f64>() {
                let pct = ((us / 1000.0) / dur_ms * 100.0).clamp(0.0, 100.0);
                progress_updates.push(pct as f32);
            }
        }
    }
    progress_updates
}

#[test]
fn test_export_progress_standard_steps() {
    let mut buf = String::new();
    let total_duration_ms = 10_000.0; // 10s clip

    buf.push_str("frame=30\nfps=60\nout_time_us=2500000\n"); // 2.5s -> 25%
    let p1 = parse_progress_chunk(&mut buf, total_duration_ms);
    assert_eq!(p1, vec![25.0]);

    buf.push_str("frame=60\nout_time_us=5000000\n"); // 5.0s -> 50%
    let p2 = parse_progress_chunk(&mut buf, total_duration_ms);
    assert_eq!(p2, vec![50.0]);

    buf.push_str("frame=120\nout_time_us=10000000\nprogress=end\n"); // 10.0s -> 100%
    let p3 = parse_progress_chunk(&mut buf, total_duration_ms);
    assert_eq!(p3, vec![100.0]);
}

#[test]
fn test_export_progress_split_across_packet_chunks() {
    let mut buf = String::new();
    let total_duration_ms = 5_000.0;

    // Packet 1: incomplete line
    buf.push_str("frame=10\nout_time_");
    let p1 = parse_progress_chunk(&mut buf, total_duration_ms);
    assert!(p1.is_empty());
    assert_eq!(buf, "out_time_");

    // Packet 2: remainder of line arriving
    buf.push_str("us=2500000\nfps=60\n");
    let p2 = parse_progress_chunk(&mut buf, total_duration_ms);
    assert_eq!(p2, vec![50.0]); // 2.5s of 5s = 50%
}

#[test]
fn test_export_progress_overshoot_clamped_to_100() {
    let mut buf = String::new();
    let total_duration_ms = 5_000.0;

    buf.push_str("out_time_us=6000000\n"); // 6s on a 5s clip
    let p = parse_progress_chunk(&mut buf, total_duration_ms);
    assert_eq!(p, vec![100.0]);
}

#[test]
fn test_export_progress_negative_or_zero_time() {
    let mut buf = String::new();
    let total_duration_ms = 5_000.0;

    buf.push_str("out_time_us=-50000\n");
    let p = parse_progress_chunk(&mut buf, total_duration_ms);
    assert_eq!(p, vec![0.0]);
}

#[test]
fn test_export_progress_malformed_lines_ignored() {
    let mut buf = String::new();
    let total_duration_ms = 5_000.0;

    buf.push_str("out_time_us=N/A\nout_time_us=invalid\nout_time_us=\nout_time_us=1000000\n");
    let p = parse_progress_chunk(&mut buf, total_duration_ms);
    assert_eq!(p, vec![20.0]); // Only the valid 1.0s line is parsed (20%)
}

#[test]
fn test_export_progress_very_short_subsecond_duration() {
    let mut buf = String::new();
    let total_duration_ms = 500.0; // 0.5s subclip

    buf.push_str("out_time_us=250000\n"); // 0.25s
    let p = parse_progress_chunk(&mut buf, total_duration_ms);
    assert_eq!(p, vec![50.0]);
}
