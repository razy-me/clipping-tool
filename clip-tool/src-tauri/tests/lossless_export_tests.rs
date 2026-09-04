// ──────────────────────────────────────────────────────────────────────────────
// Lossless Stream-Copy vs Fast Re-Encode Decision Matrix Tests
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Eq)]
enum ExportStrategy {
    LosslessDirectCopy,      // -c:v copy -c:a copy
    LosslessVideoReencodeAudio, // -c:v copy -c:a aac (filter_complex)
    FullHardwareReencode,    // Hardware GPU re-encode
}

struct ExportDecisionInput {
    is_full_clip: bool,
    has_audio_volume_changes: bool,
    has_fades: bool,
    has_multiple_tracks: bool,
}

fn determine_export_strategy(input: &ExportDecisionInput) -> ExportStrategy {
    if !input.is_full_clip {
        // Subclip trims require precise keyframe cuts or re-encoding for exact timestamps
        return ExportStrategy::FullHardwareReencode;
    }

    if !input.has_audio_volume_changes && !input.has_fades && !input.has_multiple_tracks {
        // Untouched full clip -> instant lossless stream-copy
        ExportStrategy::LosslessDirectCopy
    } else {
        // Video is untouched, only audio needs filtering & mixing
        ExportStrategy::LosslessVideoReencodeAudio
    }
}

#[test]
fn test_export_strategy_untouched_full_clip_is_lossless_direct_copy() {
    let input = ExportDecisionInput {
        is_full_clip: true,
        has_audio_volume_changes: false,
        has_fades: false,
        has_multiple_tracks: false,
    };
    assert_eq!(determine_export_strategy(&input), ExportStrategy::LosslessDirectCopy);
}

#[test]
fn test_export_strategy_audio_volume_boosted_keeps_video_lossless() {
    let input = ExportDecisionInput {
        is_full_clip: true,
        has_audio_volume_changes: true,
        has_fades: false,
        has_multiple_tracks: false,
    };
    assert_eq!(determine_export_strategy(&input), ExportStrategy::LosslessVideoReencodeAudio);
}

#[test]
fn test_export_strategy_with_fade_in_keeps_video_lossless() {
    let input = ExportDecisionInput {
        is_full_clip: true,
        has_audio_volume_changes: false,
        has_fades: true,
        has_multiple_tracks: false,
    };
    assert_eq!(determine_export_strategy(&input), ExportStrategy::LosslessVideoReencodeAudio);
}

#[test]
fn test_export_strategy_with_multitrack_mixing_keeps_video_lossless() {
    let input = ExportDecisionInput {
        is_full_clip: true,
        has_audio_volume_changes: false,
        has_fades: false,
        has_multiple_tracks: true,
    };
    assert_eq!(determine_export_strategy(&input), ExportStrategy::LosslessVideoReencodeAudio);
}

#[test]
fn test_export_strategy_trimmed_subclip_is_full_reencode() {
    let input = ExportDecisionInput {
        is_full_clip: false,
        has_audio_volume_changes: false,
        has_fades: false,
        has_multiple_tracks: false,
    };
    assert_eq!(determine_export_strategy(&input), ExportStrategy::FullHardwareReencode);
}

#[test]
fn test_bitrate_scaling_by_resolution() {
    fn target_bitrate_kbps(width: u32, height: u32, is_high_preset: bool) -> u32 {
        let pixels = width * height;
        if pixels >= 3840 * 2160 { // 4K
            if is_high_preset { 50_000 } else { 35_000 }
        } else if pixels >= 2560 * 1440 { // 1440p
            if is_high_preset { 28_000 } else { 18_000 }
        } else if pixels >= 1920 * 1080 { // 1080p
            if is_high_preset { 18_000 } else { 12_000 }
        } else { // 720p or lower
            if is_high_preset { 8_000 } else { 5_000 }
        }
    }

    assert_eq!(target_bitrate_kbps(1920, 1080, true), 18_000);
    assert_eq!(target_bitrate_kbps(1920, 1080, false), 12_000);
    assert_eq!(target_bitrate_kbps(2560, 1440, true), 28_000);
    assert_eq!(target_bitrate_kbps(3840, 2160, true), 50_000);
    assert_eq!(target_bitrate_kbps(1280, 720, true), 8_000);
}
