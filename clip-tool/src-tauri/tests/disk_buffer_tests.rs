// ──────────────────────────────────────────────────────────────────────────────
// RAM Buffer Capacity & Disk Footprint Estimation Test Suite
// ──────────────────────────────────────────────────────────────────────────────

fn estimate_video_buffer_ram_bytes(duration_secs: u32, bitrate_preset_mbps: f64) -> usize {
    let bytes_per_sec = (bitrate_preset_mbps * 1_000_000.0 / 8.0) as usize;
    // Buffer holds configured duration + 30s safety headroom
    bytes_per_sec * (duration_secs as usize + 30)
}

fn estimate_audio_buffer_ram_bytes(sample_rate: u32, channels: u16, duration_secs: u32) -> usize {
    let bytes_per_sample = 4; // f32
    (sample_rate as usize) * (channels as usize) * (duration_secs as usize + 30) * bytes_per_sample
}

#[test]
fn test_ram_buffer_estimation_1080p60_60s() {
    let ram_bytes = estimate_video_buffer_ram_bytes(60, 18.0); // 18 Mbps
    let ram_mb = ram_bytes / (1024 * 1024);
    // (60s + 30s) * 2.25 MB/s = 202.5 MB
    assert_eq!(ram_mb, 193); // in binary MiB (202.5 / 1.048576 ≈ 193)
}

#[test]
fn test_ram_buffer_estimation_4k60_300s() {
    let ram_bytes = estimate_video_buffer_ram_bytes(300, 50.0); // 50 Mbps
    let ram_mb = ram_bytes / (1024 * 1024);
    // (300s + 30s) * 6.25 MB/s ≈ 2062.5 MB ≈ 1.96 GB
    assert!(ram_mb > 1900 && ram_mb < 2000);
}

#[test]
fn test_ram_buffer_estimation_audio_tracks_stereo_48k() {
    let audio_ram = estimate_audio_buffer_ram_bytes(48000, 2, 60); // 60s
    let audio_mb = audio_ram / (1024 * 1024);
    // (60s + 30s) * 48000 * 2 * 4 bytes = 34,560,000 bytes ≈ 32.9 MiB
    assert_eq!(audio_mb, 32);
}

#[test]
fn test_exported_file_size_estimation() {
    fn estimate_file_size_bytes(duration_secs: f64, video_bitrate_kbps: u32, audio_tracks: usize) -> u64 {
        let video_bps = (video_bitrate_kbps as u64) * 1000 / 8;
        let audio_bps = (audio_tracks as u64) * 192_000 / 8; // 192kbps AAC per track
        ((video_bps + audio_bps) as f64 * duration_secs) as u64
    }

    let size_30s_1080p = estimate_file_size_bytes(30.0, 18000, 2); // 30s, 18Mbps video, 2 audio tracks
    let size_mb = size_30s_1080p as f64 / 1_000_000.0;
    // 30s * (2.25MB/s + 0.048MB/s) ≈ 68.9 MB
    assert!((size_mb - 68.9).abs() < 1.0);
}
