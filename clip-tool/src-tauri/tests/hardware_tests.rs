use app_lib::config::VideoCodec;
use app_lib::hardware::ScalingMethod;

// ──────────────────────────────────────────────────────────────────────────────
// Hardware Encoder & GPU Acceleration Matrix Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_scaling_method_serialization_roundtrip_cuda() {
    let sm = ScalingMethod::Cuda;
    let json = serde_json::to_string(&sm).unwrap();
    let decoded: ScalingMethod = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, ScalingMethod::Cuda);
}

#[test]
fn test_scaling_method_serialization_roundtrip_qsv() {
    let sm = ScalingMethod::Qsv;
    let json = serde_json::to_string(&sm).unwrap();
    let decoded: ScalingMethod = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, ScalingMethod::Qsv);
}

#[test]
fn test_scaling_method_serialization_roundtrip_d3d11() {
    let sm = ScalingMethod::D3d11Direct;
    let json = serde_json::to_string(&sm).unwrap();
    let decoded: ScalingMethod = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, ScalingMethod::D3d11Direct);
}

#[test]
fn test_scaling_method_serialization_roundtrip_cpu_fallback() {
    let sm = ScalingMethod::CpuFallback;
    let json = serde_json::to_string(&sm).unwrap();
    let decoded: ScalingMethod = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, ScalingMethod::CpuFallback);
}

#[test]
fn test_codec_candidate_mapping_h264() {
    let (candidates, fallback) = match VideoCodec::H264 {
        VideoCodec::H264 => (&["h264_nvenc", "h264_amf", "h264_qsv"][..], "libx264"),
        _ => panic!("Expected H264"),
    };
    assert_eq!(candidates, &["h264_nvenc", "h264_amf", "h264_qsv"]);
    assert_eq!(fallback, "libx264");
}

#[test]
fn test_codec_candidate_mapping_hevc() {
    let (candidates, fallback) = match VideoCodec::HEVC {
        VideoCodec::HEVC => (&["hevc_nvenc", "hevc_amf", "hevc_qsv"][..], "libx265"),
        _ => panic!("Expected HEVC"),
    };
    assert_eq!(candidates, &["hevc_nvenc", "hevc_amf", "hevc_qsv"]);
    assert_eq!(fallback, "libx265");
}

#[test]
fn test_codec_candidate_mapping_av1() {
    let (candidates, fallback) = match VideoCodec::AV1 {
        VideoCodec::AV1 => (&["av1_nvenc", "av1_amf", "av1_qsv"][..], "libsvtav1"),
        _ => panic!("Expected AV1"),
    };
    assert_eq!(candidates, &["av1_nvenc", "av1_amf", "av1_qsv"]);
    assert_eq!(fallback, "libsvtav1");
}

#[test]
fn test_hardware_cache_json_structure() {
    let json = r#"{
        "encoders": {
            "H264": "h264_nvenc",
            "HEVC": "hevc_nvenc",
            "AV1": "av1_nvenc"
        },
        "scaling_methods": {
            "h264_nvenc": "Cuda",
            "h264_qsv": "Qsv",
            "h264_amf": "D3d11Direct",
            "libx264": "CpuFallback"
        }
    }"#;
    let val: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(val["encoders"]["H264"], "h264_nvenc");
    assert_eq!(val["scaling_methods"]["h264_nvenc"], "Cuda");
}

#[test]
fn test_encoder_scaling_compatibility() {
    fn is_pure_vram(sm: &ScalingMethod) -> bool {
        matches!(sm, ScalingMethod::Cuda | ScalingMethod::Qsv | ScalingMethod::D3d11Direct)
    }

    assert!(is_pure_vram(&ScalingMethod::Cuda));
    assert!(is_pure_vram(&ScalingMethod::Qsv));
    assert!(is_pure_vram(&ScalingMethod::D3d11Direct));
    assert!(!is_pure_vram(&ScalingMethod::CpuFallback));
}
