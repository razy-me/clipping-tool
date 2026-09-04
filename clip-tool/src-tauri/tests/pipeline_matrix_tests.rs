use app_lib::recorder::capture_graph;
use app_lib::hardware::ScalingMethod;

// ──────────────────────────────────────────────────────────────────────────────
// Framerate Matrix (10 to 360 FPS)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_matrix_framerates_common_standard() {
    let standard_fps = ["24", "25", "30", "48", "50", "60", "75", "90", "120", "144", "165", "240", "360"];
    for fps in standard_fps {
        let g = capture_graph("h264_nvenc", fps, "Original", 1080, true, &ScalingMethod::Cuda, 0, false);
        assert!(g.contains(&format!("framerate={fps}")));
        assert!(g.contains("draw_mouse=true"));
        assert!(g.contains("dup_frames=true"));
    }
}

#[test]
fn test_matrix_framerates_uncommon_and_low() {
    let odd_fps = ["10", "12", "15", "18", "20", "22", "27", "33", "40", "55", "70", "85", "100", "110", "135", "180", "200", "280", "300", "320"];
    for fps in odd_fps {
        let g = capture_graph("h264_qsv", fps, "Original", 1080, false, &ScalingMethod::Qsv, 0, false);
        assert!(g.contains(&format!("framerate={fps}")));
        assert!(g.contains("draw_mouse=false"));
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Resolution Matrix (Standard 16:9, Ultrawide 21:9, Super-Ultrawide 32:9, 4:3)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_matrix_resolutions_16_by_9_downscaling() {
    let resolutions = [
        ("1440p", 1440),
        ("1080p", 1080),
        ("900p", 900),
        ("720p", 720),
        ("540p", 540),
        ("480p", 480),
        ("360p", 360),
        ("240p", 240),
    ];

    for (res_str, target_h) in resolutions {
        // From 4K (2160p) monitor down to target
        let g = capture_graph("h264_nvenc", "60", res_str, 2160, true, &ScalingMethod::Cuda, 0, false);
        assert!(g.contains(&format!("scale=-2:{target_h}")), "Failed for {}", res_str);
        assert!(g.contains("format=nv12"));
        assert!(g.contains("hwdownload"));
    }
}

#[test]
fn test_matrix_resolutions_native_never_scales() {
    let monitor_heights = [720, 900, 1080, 1200, 1440, 1600, 2160, 2880, 4320];
    for h in monitor_heights {
        let g = capture_graph("h264_nvenc", "60", "Original", h, true, &ScalingMethod::Cuda, 0, false);
        assert!(!g.contains("scale=-2"), "Native resolution scaled unexpectedly for height {}", h);
        assert!(g.contains("format=nv12"));
    }
}

#[test]
fn test_matrix_resolutions_upscaling_prevention() {
    // If monitor height is 1080, selecting 1440p should NOT upscale
    let g = capture_graph("h264_nvenc", "60", "1440p", 1080, false, &ScalingMethod::Cuda, 0, false);
    assert!(!g.contains("scale=-2"), "Upscaling should be prevented when target > monitor height");
}

#[test]
fn test_matrix_resolutions_same_height_never_scales() {
    // 1080p target on 1080p monitor
    let g1080 = capture_graph("h264_nvenc", "60", "1080p", 1080, false, &ScalingMethod::Cuda, 0, false);
    assert!(!g1080.contains("scale=-2"), "1080p on 1080p monitor should not invoke scale");

    // 720p target on 720p monitor
    let g720 = capture_graph("h264_qsv", "60", "720p", 720, false, &ScalingMethod::Qsv, 0, false);
    assert!(!g720.contains("scale=-2"), "720p on 720p monitor should not invoke scale");

    // 1440p target on 1440p monitor
    let g1440 = capture_graph("h264_amf", "60", "1440p", 1440, false, &ScalingMethod::D3d11Direct, 0, false);
    assert!(!g1440.contains("scale=-2"), "1440p on 1440p monitor should not invoke scale");
}

// ──────────────────────────────────────────────────────────────────────────────
// Encoder Pixel Format Matrix
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_matrix_encoder_pixel_formats() {
    let hardware_encoders = [
        "h264_nvenc", "hevc_nvenc", "av1_nvenc",
        "h264_qsv", "hevc_qsv", "av1_qsv",
        "h264_amf", "hevc_amf", "av1_amf",
    ];

    for enc in hardware_encoders {
        let g = capture_graph(enc, "60", "Original", 1080, true, &ScalingMethod::Cuda, 0, false);
        assert!(g.contains("format=nv12"), "Hardware encoder {} should output nv12", enc);
    }

    let software_encoders = ["libx264", "libx265", "libsvtav1"];
    for enc in software_encoders {
        let g = capture_graph(enc, "60", "Original", 1080, true, &ScalingMethod::CpuFallback, 0, false);
        assert!(g.contains("format=yuv420p"), "Software encoder {} should output yuv420p", enc);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Monitor Output Index Matrix (0 to 15)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_matrix_monitor_indices() {
    for idx in 0..=15 {
        let g = capture_graph("h264_nvenc", "60", "Original", 1080, false, &ScalingMethod::Cuda, idx, false);
        assert!(g.contains(&format!("output_idx={idx}")), "Monitor index {} missing", idx);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Mouse Cursor Matrix
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_matrix_mouse_cursor_combinations() {
    let encoders = ["h264_nvenc", "h264_qsv", "h264_amf", "libx264"];
    for enc in encoders {
        let g_on = capture_graph(enc, "60", "Original", 1080, true, &ScalingMethod::Cuda, 0, false);
        assert!(g_on.contains("draw_mouse=true"));

        let g_off = capture_graph(enc, "60", "Original", 1080, false, &ScalingMethod::Cuda, 0, false);
        assert!(g_off.contains("draw_mouse=false"));
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// HDR Tonemapping Combinations
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_matrix_hdr_tonemapping_with_downscale() {
    let g = capture_graph("h264_nvenc", "60", "720p", 1080, true, &ScalingMethod::Cuda, 0, true);
    assert!(g.contains("tonemap=tonemap=hable:desat=0"));
    assert!(g.contains("scale=-2:720"));
    assert!(g.contains("format=nv12"));
}

#[test]
fn test_matrix_hdr_tonemapping_native_resolution() {
    let g = capture_graph("h264_nvenc", "60", "Original", 1080, true, &ScalingMethod::Cuda, 0, true);
    assert!(g.contains("tonemap=tonemap=hable:desat=0"));
    assert!(!g.contains("scale=-2"));
    assert!(g.contains("format=nv12"));
}

#[test]
fn test_matrix_hdr_tonemapping_software_encoder() {
    let g = capture_graph("libx264", "60", "Original", 1080, false, &ScalingMethod::CpuFallback, 0, true);
    assert!(g.contains("tonemap=tonemap=hable:desat=0"));
    assert!(g.contains("format=yuv420p"));
}
