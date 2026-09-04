// ──────────────────────────────────────────────────────────────────────────────
// HTTP Video & Audio Streaming Range Request Parser Test Suite
// ──────────────────────────────────────────────────────────────────────────────

fn parse_http_range(range_header: Option<&str>, total_size: u64) -> Result<(u64, u64, bool), ()> {
    if total_size == 0 {
        return Ok((0, 0, false));
    }
    let mut start = 0u64;
    let mut end_incl = total_size.saturating_sub(1);
    let mut is_range = false;

    if let Some(range) = range_header {
        let spec = range.trim_start_matches("bytes=");
        let mut it = spec.splitn(2, '-');
        if let Some(s) = it.next().and_then(|v| v.parse::<u64>().ok()) { start = s; is_range = true; }
        if let Some(e) = it.next().and_then(|v| v.split(',').next())
            .and_then(|v| v.parse::<u64>().ok()) { end_incl = e.min(total_size - 1); is_range = true; }
        if start >= total_size {
            return Err(()); // 416 Range Not Satisfiable
        }
        // Allow up to 32MB chunks for fast video buffering in browser
        end_incl = end_incl.min(start + 32_000_000 - 1).min(total_size.saturating_sub(1));
    }
    Ok((start, end_incl, is_range))
}

fn is_safe_wav_filename(name: &str) -> bool {
    !name.is_empty() && !name.contains("..") && !name.contains('/') && !name.contains('\\')
}

#[test]
fn test_range_no_header_returns_full_file() {
    let total = 50_000_000u64; // 50MB
    let (start, end_incl, is_range) = parse_http_range(None, total).unwrap();
    assert_eq!(start, 0);
    assert_eq!(end_incl, total - 1);
    assert!(!is_range);
}

#[test]
fn test_range_explicit_first_1024_bytes() {
    let total = 10_000_000u64;
    let (start, end_incl, is_range) = parse_http_range(Some("bytes=0-1023"), total).unwrap();
    assert_eq!(start, 0);
    assert_eq!(end_incl, 1023);
    assert!(is_range);
    assert_eq!(end_incl - start + 1, 1024);
}

#[test]
fn test_range_open_ended_start_1000() {
    let total = 10_000_000u64; // 10MB
    let (start, end_incl, is_range) = parse_http_range(Some("bytes=1000-"), total).unwrap();
    assert_eq!(start, 1000);
    assert_eq!(end_incl, total - 1);
    assert!(is_range);
}

#[test]
fn test_range_open_ended_clamped_to_32mb_chunk() {
    let total = 100_000_000u64; // 100MB
    let (start, end_incl, _is_range) = parse_http_range(Some("bytes=0-"), total).unwrap();
    assert_eq!(start, 0);
    assert_eq!(end_incl, 32_000_000 - 1); // Capped at 32MB
    assert_eq!(end_incl - start + 1, 32_000_000);
}

#[test]
fn test_range_out_of_bounds_start_returns_416_error() {
    let total = 5_000_000u64; // 5MB
    let res = parse_http_range(Some("bytes=6000000-"), total);
    assert!(res.is_err(), "Start beyond EOF should return Range Not Satisfiable");
}

#[test]
fn test_range_end_beyond_file_size_clamped_to_eof() {
    let total = 1_000_000u64;
    let (start, end_incl, _) = parse_http_range(Some("bytes=500000-2000000"), total).unwrap();
    assert_eq!(start, 500_000);
    assert_eq!(end_incl, total - 1);
}

#[test]
fn test_safe_wav_filename_path_traversal_prevention() {
    assert!(is_safe_wav_filename("clip_System.wav"));
    assert!(is_safe_wav_filename("2026-08-31_Game_Microphone.wav"));
    assert!(!is_safe_wav_filename("../evil.wav"));
    assert!(!is_safe_wav_filename("..\\evil.wav"));
    assert!(!is_safe_wav_filename("sub/dir/audio.wav"));
    assert!(!is_safe_wav_filename("sub\\dir\\audio.wav"));
    assert!(!is_safe_wav_filename(""));
}
