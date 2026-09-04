use app_lib::config::VideoCodec;

// ──────────────────────────────────────────────────────────────────────────────
// Video Codec Hardware Fallback Hierarchy & Compatibility Tests
// ──────────────────────────────────────────────────────────────────────────────

fn resolve_encoder_fallback_chain<'a>(codec: &VideoCodec, available_hardware: &[&'a str]) -> &'a str {
    let (candidates, fallback) = match codec {
        VideoCodec::H264 => (&["h264_nvenc", "h264_amf", "h264_qsv"][..], "libx264"),
        VideoCodec::HEVC => (&["hevc_nvenc", "hevc_amf", "hevc_qsv"][..], "libx265"),
        VideoCodec::AV1 => (&["av1_nvenc", "av1_amf", "av1_qsv"][..], "libsvtav1"),
    };

    for candidate in candidates {
        if available_hardware.contains(candidate) {
            return candidate;
        }
    }
    fallback
}

#[test]
fn test_codec_fallback_nvidia_rtx_40_series() {
    // RTX 4080 has all NVENC encoders (H264, HEVC, AV1)
    let hw = ["h264_nvenc", "hevc_nvenc", "av1_nvenc"];

    assert_eq!(resolve_encoder_fallback_chain(&VideoCodec::H264, &hw), "h264_nvenc");
    assert_eq!(resolve_encoder_fallback_chain(&VideoCodec::HEVC, &hw), "hevc_nvenc");
    assert_eq!(resolve_encoder_fallback_chain(&VideoCodec::AV1, &hw), "av1_nvenc");
}

#[test]
fn test_codec_fallback_nvidia_gtx_10_series_no_av1() {
    // GTX 1080 has H264 & HEVC NVENC, but NO AV1 hardware encoder
    let hw = ["h264_nvenc", "hevc_nvenc"];

    assert_eq!(resolve_encoder_fallback_chain(&VideoCodec::H264, &hw), "h264_nvenc");
    assert_eq!(resolve_encoder_fallback_chain(&VideoCodec::HEVC, &hw), "hevc_nvenc");
    assert_eq!(resolve_encoder_fallback_chain(&VideoCodec::AV1, &hw), "libsvtav1"); // Falls back to CPU SVT-AV1
}

#[test]
fn test_codec_fallback_amd_radeon_rx_7000() {
    // RX 7800 XT has AMF H264, HEVC and AV1
    let hw = ["h264_amf", "hevc_amf", "av1_amf"];

    assert_eq!(resolve_encoder_fallback_chain(&VideoCodec::H264, &hw), "h264_amf");
    assert_eq!(resolve_encoder_fallback_chain(&VideoCodec::HEVC, &hw), "hevc_amf");
    assert_eq!(resolve_encoder_fallback_chain(&VideoCodec::AV1, &hw), "av1_amf");
}

#[test]
fn test_codec_fallback_intel_arc_gpu() {
    // Intel Arc A770 has QSV H264, HEVC and AV1
    let hw = ["h264_qsv", "hevc_qsv", "av1_qsv"];

    assert_eq!(resolve_encoder_fallback_chain(&VideoCodec::H264, &hw), "h264_qsv");
    assert_eq!(resolve_encoder_fallback_chain(&VideoCodec::HEVC, &hw), "hevc_qsv");
    assert_eq!(resolve_encoder_fallback_chain(&VideoCodec::AV1, &hw), "av1_qsv");
}

#[test]
fn test_codec_fallback_software_only_cpu_system() {
    let hw = []; // No GPU encoders

    assert_eq!(resolve_encoder_fallback_chain(&VideoCodec::H264, &hw), "libx264");
    assert_eq!(resolve_encoder_fallback_chain(&VideoCodec::HEVC, &hw), "libx265");
    assert_eq!(resolve_encoder_fallback_chain(&VideoCodec::AV1, &hw), "libsvtav1");
}
