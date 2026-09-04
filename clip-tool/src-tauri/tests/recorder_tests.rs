use app_lib::recorder::{find_first_ts_keyframe_offset, is_ts_keyframe, capture_graph, InMemoryVideoBuffer};

fn create_ts_packet(pusi: bool, adaptation_control: u8, has_rai: bool, payload: &[u8]) -> Vec<u8> {
    let mut pkt = vec![0u8; 188];
    pkt[0] = 0x47; // Sync byte
    if pusi {
        pkt[1] |= 0x40; // PUSI flag
    }
    pkt[3] = (adaptation_control & 0x03) << 4; // adaptation_field_control

    let mut offset = 4;
    if adaptation_control == 2 || adaptation_control == 3 {
        let adapt_len = if has_rai { 1 } else { 0 };
        pkt[4] = adapt_len as u8;
        if has_rai {
            pkt[5] = 0x40; // RAI bit (bit 6)
            offset = 6;
        } else {
            offset = 5;
        }
    }

    let copy_len = payload.len().min(188 - offset);
    pkt[offset..offset + copy_len].copy_from_slice(&payload[..copy_len]);
    pkt
}

// ──────────────────────────────────────────────────────────────────────────────
// H.264 NAL Unit Keyframe & Non-Keyframe Tests (All types 0..=31)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_h264_nal_type_5_idr_is_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, 0x65]);
    assert!(is_ts_keyframe(&pkt));
}

#[test]
fn test_h264_nal_type_5_idr_ref_idc_0_is_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, 0x05]);
    assert!(is_ts_keyframe(&pkt));
}

#[test]
fn test_h264_nal_type_7_sps_is_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, 0x67, 0x42]);
    assert!(is_ts_keyframe(&pkt));
}

#[test]
fn test_h264_nal_type_8_pps_is_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, 0x68, 0xCE]);
    assert!(is_ts_keyframe(&pkt));
}

#[test]
fn test_h264_nal_type_1_non_idr_slice_is_not_keyframe() {
    for ref_idc in [0x00, 0x20, 0x40, 0x60] {
        let nal = ref_idc | 0x01; // Type 1
        let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, nal]);
        assert_eq!(find_first_ts_keyframe_offset(&pkt), None, "NAL 0x{:02X} should not be keyframe", nal);
    }
}

#[test]
fn test_h264_nal_type_6_sei_is_not_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, 0x06]);
    assert!(!is_ts_keyframe(&pkt));
}

#[test]
fn test_h264_nal_type_9_aud_is_not_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, 0x09]);
    assert!(!is_ts_keyframe(&pkt));
}

// ──────────────────────────────────────────────────────────────────────────────
// HEVC (H.265) NAL Unit Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_hevc_idr_w_radl_type_19_is_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, 19 << 1]);
    assert!(is_ts_keyframe(&pkt));
}

#[test]
fn test_hevc_idr_n_lp_type_20_is_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, 20 << 1]);
    assert!(is_ts_keyframe(&pkt));
}

#[test]
fn test_hevc_cra_type_21_is_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, 21 << 1]);
    assert!(is_ts_keyframe(&pkt));
}

#[test]
fn test_hevc_vps_type_32_is_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, 32 << 1]);
    assert!(is_ts_keyframe(&pkt));
}

#[test]
fn test_hevc_sps_type_33_is_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, 33 << 1]);
    assert!(is_ts_keyframe(&pkt));
}

#[test]
fn test_hevc_pps_type_34_is_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, 34 << 1]);
    assert!(is_ts_keyframe(&pkt));
}

#[test]
fn test_hevc_trail_r_type_1_is_not_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, 1 << 1]);
    assert!(!is_ts_keyframe(&pkt));
}

#[test]
fn test_hevc_rasl_r_type_9_is_not_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, 9 << 1]);
    assert!(!is_ts_keyframe(&pkt));
}

// ──────────────────────────────────────────────────────────────────────────────
// AV1 OBU Keyframe Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_av1_sequence_header_obu_1_is_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, (1 << 3) | 0x00]);
    assert!(is_ts_keyframe(&pkt));
}

#[test]
fn test_av1_sequence_header_with_size_obu_is_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, (1 << 3) | 0x02]);
    assert!(is_ts_keyframe(&pkt));
}

#[test]
fn test_av1_frame_header_obu_3_is_not_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, (3 << 3) | 0x00]);
    assert!(!is_ts_keyframe(&pkt));
}

#[test]
fn test_av1_temporal_delimiter_obu_2_is_not_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, (2 << 3) | 0x00]);
    assert!(!is_ts_keyframe(&pkt));
}

#[test]
fn test_av1_tile_group_obu_4_is_not_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, (4 << 3) | 0x00]);
    assert!(!is_ts_keyframe(&pkt));
}

#[test]
fn test_av1_frame_obu_6_is_not_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, (6 << 3) | 0x00]);
    assert!(!is_ts_keyframe(&pkt));
}

// ──────────────────────────────────────────────────────────────────────────────
// TS Adaptation Field Keyframe Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ts_adaptation_field_rai_on_adaptation_control_2() {
    let pkt = create_ts_packet(true, 2, true, &[0x00]);
    assert!(is_ts_keyframe(&pkt));
}

#[test]
fn test_ts_adaptation_field_rai_on_adaptation_control_3() {
    let pkt = create_ts_packet(true, 3, true, &[0x00]);
    assert!(is_ts_keyframe(&pkt));
}

#[test]
fn test_ts_adaptation_field_without_rai_is_not_keyframe() {
    let pkt = create_ts_packet(true, 3, false, &[0x00, 0x00, 0x01, 0xE0, 0x41]);
    assert!(!is_ts_keyframe(&pkt));
}

#[test]
fn test_ts_pusi_false_with_rai_is_not_parsed_as_keyframe_start() {
    // When PUSI is false, it's a continuation packet, not a PES start
    let pkt = create_ts_packet(false, 3, true, &[0x00]);
    assert_eq!(find_first_ts_keyframe_offset(&pkt), None);
}

// ──────────────────────────────────────────────────────────────────────────────
// 4-byte start code (0x00 0x00 0x00 0x01) vs 3-byte start code (0x00 0x00 0x01)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_4_byte_start_code_h264_idr() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x65]);
    assert!(is_ts_keyframe(&pkt));
}

#[test]
fn test_4_byte_start_code_hevc_idr() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 19 << 1]);
    assert!(is_ts_keyframe(&pkt));
}

// ──────────────────────────────────────────────────────────────────────────────
// Multi-Packet Streams & Arbitrary Offsets
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_multi_packet_stream_keyframe_at_index_0() {
    let key = create_ts_packet(true, 3, true, &[0x00]);
    let p = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x41]);
    let mut stream = Vec::new();
    stream.extend_from_slice(&key);
    stream.extend_from_slice(&p);
    assert_eq!(find_first_ts_keyframe_offset(&stream), Some(0));
}

#[test]
fn test_multi_packet_stream_keyframe_at_index_1() {
    let p = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x41]);
    let key = create_ts_packet(true, 3, true, &[0x00]);
    let mut stream = Vec::new();
    stream.extend_from_slice(&p);
    stream.extend_from_slice(&key);
    assert_eq!(find_first_ts_keyframe_offset(&stream), Some(188));
}

#[test]
fn test_multi_packet_stream_keyframe_at_index_5() {
    let p = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x41]);
    let key = create_ts_packet(true, 3, true, &[0x00]);
    let mut stream = Vec::new();
    for _ in 0..5 { stream.extend_from_slice(&p); }
    stream.extend_from_slice(&key);
    for _ in 0..3 { stream.extend_from_slice(&p); }
    assert_eq!(find_first_ts_keyframe_offset(&stream), Some(188 * 5));
}

#[test]
fn test_multi_packet_stream_no_keyframe() {
    let p = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x41]);
    let mut stream = Vec::new();
    for _ in 0..10 { stream.extend_from_slice(&p); }
    assert_eq!(find_first_ts_keyframe_offset(&stream), None);
}

#[test]
fn test_extract_ts_pts_present() {
    use app_lib::recorder::extract_ts_pts;
    // PES packet with PTS flag set (0x80 on byte 7), PTS data at bytes 9..14
    let payload = vec![
        0x00, 0x00, 0x01, 0xE0, // Start code + video stream 0xE0
        0x00, 0x00,             // length
        0x80,                   // flags 1
        0x80,                   // flags 2 (PTS present = 0b10 << 6)
        0x05,                   // header data length (5 bytes of PTS)
        0x21, 0x00, 0x01, 0x00, 0x01, // PTS encoded bytes
    ];
    let pkt = create_ts_packet(true, 1, false, &payload);
    let pts = extract_ts_pts(&pkt);
    assert!(pts.is_some());
}

// ──────────────────────────────────────────────────────────────────────────────
// In-Memory Ring Buffer Exhaustive Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_buffer_creation_zero_state() {
    let buf = InMemoryVideoBuffer::new(2048);
    assert_eq!(buf.current_bytes, 0);
    assert_eq!(buf.max_bytes, 2048);
    assert!(buf.chunks.is_empty());
}

#[test]
fn test_buffer_push_single_chunk() {
    let mut buf = InMemoryVideoBuffer::new(2048);
    let pkt = create_ts_packet(true, 3, true, &[0x00]);
    buf.push_bytes(&pkt);
    assert_eq!(buf.current_bytes, 188);
    assert_eq!(buf.chunks.len(), 1);
    assert!(buf.chunks[0].is_keyframe);
}

#[test]
fn test_buffer_push_oversized_chunk_ignored() {
    let mut buf = InMemoryVideoBuffer::new(100);
    let large = vec![0x47; 200];
    buf.push_bytes(&large);
    assert_eq!(buf.current_bytes, 0); // Ignored because > cap
}

#[test]
fn test_buffer_multiple_pushes_under_capacity() {
    let mut buf = InMemoryVideoBuffer::new(10000);
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x41]);
    for _ in 0..10 {
        buf.push_bytes(&pkt);
    }
    assert_eq!(buf.current_bytes, 188 * 10);
    assert_eq!(buf.chunks.len(), 10);
}

#[test]
fn test_buffer_continuous_wrapping_stress_50_cycles() {
    let cap = 188 * 5; // Exactly 5 packets capacity
    let mut buf = InMemoryVideoBuffer::new(cap);
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x41]);

    for _ in 0..50 {
        buf.push_bytes(&pkt);
        assert!(buf.current_bytes <= cap);
    }
}

#[test]
fn test_buffer_extract_target_duration() {
    let mut buf = InMemoryVideoBuffer::new(10000);
    let key = create_ts_packet(true, 3, true, &[0x00]);
    let p = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x41]);

    buf.push_bytes(&key);
    buf.push_bytes(&p);
    buf.push_bytes(&p);

    let extracted = buf.extract(60.0);
    assert_eq!(extracted.data.len(), 188 * 3);
    assert!(extracted.duration_secs >= 0.0);
}

#[test]
fn test_buffer_extract_zero_time() {
    let mut buf = InMemoryVideoBuffer::new(10000);
    let key = create_ts_packet(true, 3, true, &[0x00]);
    buf.push_bytes(&key);

    let extracted = buf.extract(0.0);
    assert!(extracted.data.len() <= 188);
}

// ──────────────────────────────────────────────────────────────────────────────
// Capture Graph Combinatorial Matrix Tests
// ──────────────────────────────────────────────────────────────────────────────
use app_lib::hardware::ScalingMethod;

#[test]
fn test_capture_graph_nvenc_all_framerates() {
    for fps in ["30", "60", "120", "144", "240"] {
        let g = capture_graph("h264_nvenc", fps, "Source", 1080, true, &ScalingMethod::Cuda, 0, false);
        assert!(g.contains(&format!("framerate={fps}")));
        assert!(g.contains("draw_mouse=true"));
        assert!(g.contains("dup_frames=true"));
    }
}

#[test]
fn test_capture_graph_cuda_pure_vram_scaling() {
    // Scaling to 720p from 1080p via CUDA/FFmpeg pipeline
    let g720 = capture_graph("h264_nvenc", "60", "720p", 1080, false, &ScalingMethod::Cuda, 0, false);
    assert!(g720.contains("scale=-2:720"));
    assert!(g720.contains("format=nv12"));
    assert!(g720.contains("hwdownload"));

    // Scaling to 1080p from 1440p
    let g1080 = capture_graph("hevc_nvenc", "60", "1080p", 1440, false, &ScalingMethod::Cuda, 0, false);
    assert!(g1080.contains("scale=-2:1080"));
    assert!(g1080.contains("format=nv12"));

    // Scaling to 1440p from 2160p (4K)
    let g1440 = capture_graph("av1_nvenc", "60", "1440p", 2160, false, &ScalingMethod::Cuda, 0, false);
    assert!(g1440.contains("scale=-2:1440"));
    assert!(g1440.contains("format=nv12"));

    // Source (no scale) - Direct pass-through with NV12 format conversion
    let g_src = capture_graph("h264_nvenc", "60", "Source", 1080, false, &ScalingMethod::Cuda, 0, false);
    assert!(!g_src.contains("scale=-2"));
    assert!(g_src.contains("format=nv12"));
    assert_eq!(g_src, "ddagrab=output_idx=0:framerate=60:draw_mouse=false:dup_frames=true,hwdownload,format=bgra,format=nv12");
}

#[test]
fn test_capture_graph_multi_monitor_output_idx() {
    let g_mon2 = capture_graph("h264_nvenc", "60", "Source", 1080, false, &ScalingMethod::Cuda, 1, false);
    assert!(g_mon2.contains("ddagrab=output_idx=1"));
}

#[test]
fn test_capture_graph_qsv_pure_vram_scaling() {
    // QSV downscaled
    let g720 = capture_graph("h264_qsv", "60", "720p", 1080, false, &ScalingMethod::Qsv, 0, false);
    assert!(g720.contains("scale=-2:720"));
    assert!(g720.contains("format=nv12"));

    // QSV native
    let g_src = capture_graph("h264_qsv", "60", "Source", 1080, false, &ScalingMethod::Qsv, 0, false);
    assert!(!g_src.contains("scale=-2"));
    assert!(g_src.contains("format=nv12"));
}

#[test]
fn test_capture_graph_amf_d3d11_direct() {
    // AMF native
    let g_src = capture_graph("h264_amf", "60", "Source", 1080, false, &ScalingMethod::D3d11Direct, 0, false);
    assert!(g_src.contains("hwdownload"));
    assert!(g_src.contains("format=nv12"));
    assert_eq!(g_src, "ddagrab=output_idx=0:framerate=60:draw_mouse=false:dup_frames=true,hwdownload,format=bgra,format=nv12");

    // AMF scaled
    let g_scaled = capture_graph("h264_amf", "60", "720p", 1080, false, &ScalingMethod::D3d11Direct, 0, false);
    assert!(g_scaled.contains("scale=-2:720"));
    assert!(g_scaled.contains("format=nv12"));
}

#[test]
fn test_capture_graph_cpu_fallback_format_yuv420p() {
    let g = capture_graph("libx264", "60", "Source", 1080, false, &ScalingMethod::CpuFallback, 0, false);
    assert!(g.contains("hwdownload"));
    assert!(g.contains("format=yuv420p"));
}

#[test]
fn test_capture_graph_mouse_cursor_flag() {
    let g_mouse_on = capture_graph("h264_nvenc", "60", "Source", 1080, true, &ScalingMethod::Cuda, 0, false);
    assert!(g_mouse_on.contains("draw_mouse=true"));

    let g_mouse_off = capture_graph("h264_nvenc", "60", "Source", 1080, false, &ScalingMethod::Cuda, 0, false);
    assert!(g_mouse_off.contains("draw_mouse=false"));
}

#[test]
fn test_3_byte_startcode_at_end_of_slice_boundary() {
    // Valid PES header prefix, followed by filler, with 3-byte start code + NAL header at the very end of the 184-byte payload
    let mut payload = vec![0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00];
    payload.extend(vec![0xFF; 171]);
    payload.extend_from_slice(&[0x00, 0x00, 0x01, 0x65]); // NAL Type 5 IDR at exact end
    let pkt = create_ts_packet(true, 1, false, &payload);
    assert!(is_ts_keyframe(&pkt));
}

#[test]
fn test_4_byte_startcode_at_end_of_slice_boundary() {
    // Valid PES header prefix, followed by filler, with 4-byte start code + NAL header at the very end of the 184-byte payload
    let mut payload = vec![0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00];
    payload.extend(vec![0xFF; 170]);
    payload.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x65]); // NAL Type 5 IDR at exact end
    let pkt = create_ts_packet(true, 1, false, &payload);
    assert!(is_ts_keyframe(&pkt));
}

// ──────────────────────────────────────────────────────────────────────────────
// Additional TS Stream ID & PTS Decoding Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_extract_ts_pts_all_video_stream_ids_0xe0_to_0xef() {
    use app_lib::recorder::extract_ts_pts;
    for stream_id in 0xE0..=0xEFu8 {
        let payload = vec![
            0x00, 0x00, 0x01, stream_id,
            0x00, 0x00, 0x80, 0x80, 0x05,
            0x21, 0x00, 0x01, 0x00, 0x01,
        ];
        let pkt = create_ts_packet(true, 1, false, &payload);
        assert!(extract_ts_pts(&pkt).is_some(), "Stream ID 0x{:02X} should extract PTS", stream_id);
    }
}

#[test]
fn test_extract_ts_pts_private_stream_1_0xbd() {
    use app_lib::recorder::extract_ts_pts;
    let payload = vec![
        0x00, 0x00, 0x01, 0xBD,
        0x00, 0x00, 0x80, 0x80, 0x05,
        0x21, 0x00, 0x01, 0x00, 0x01,
    ];
    let pkt = create_ts_packet(true, 1, false, &payload);
    assert!(extract_ts_pts(&pkt).is_some());
}

#[test]
fn test_extract_ts_pts_vc1_stream_0xfd() {
    use app_lib::recorder::extract_ts_pts;
    let payload = vec![
        0x00, 0x00, 0x01, 0xFD,
        0x00, 0x00, 0x80, 0x80, 0x05,
        0x21, 0x00, 0x01, 0x00, 0x01,
    ];
    let pkt = create_ts_packet(true, 1, false, &payload);
    assert!(extract_ts_pts(&pkt).is_some());
}

#[test]
fn test_extract_ts_pts_audio_stream_ignored() {
    use app_lib::recorder::extract_ts_pts;
    let payload = vec![
        0x00, 0x00, 0x01, 0xC0, // Audio stream 0xC0
        0x00, 0x00, 0x80, 0x80, 0x05,
        0x21, 0x00, 0x01, 0x00, 0x01,
    ];
    let pkt = create_ts_packet(true, 1, false, &payload);
    assert_eq!(extract_ts_pts(&pkt), None);
}

#[test]
fn test_extract_ts_pts_flags_zero_returns_none() {
    use app_lib::recorder::extract_ts_pts;
    let payload = vec![
        0x00, 0x00, 0x01, 0xE0,
        0x00, 0x00, 0x80, 0x00, 0x00, // No PTS flag
    ];
    let pkt = create_ts_packet(true, 1, false, &payload);
    assert_eq!(extract_ts_pts(&pkt), None);
}

#[test]
fn test_extract_ts_pts_flags_pts_and_dts_present() {
    use app_lib::recorder::extract_ts_pts;
    let payload = vec![
        0x00, 0x00, 0x01, 0xE0,
        0x00, 0x00, 0x80, 0xC0, 0x0A, // PTS + DTS present (0b11 << 6)
        0x31, 0x00, 0x01, 0x00, 0x01, // PTS
        0x11, 0x00, 0x01, 0x00, 0x01, // DTS
    ];
    let pkt = create_ts_packet(true, 1, false, &payload);
    assert!(extract_ts_pts(&pkt).is_some());
}

#[test]
fn test_extract_ts_pts_value_exact_calculation() {
    use app_lib::recorder::extract_ts_pts;
    // Encoded: b0=0x21, b1=0x00, b2=0x01, b3=0x00, b4=0x01
    // PTS = ((0x21 & 0x0E) << 29) | (0 << 22) | ((0x01 & 0xFE) << 14) | (0 << 7) | ((0x01 & 0xFE) >> 1)
    let payload = vec![
        0x00, 0x00, 0x01, 0xE0,
        0x00, 0x00, 0x80, 0x80, 0x05,
        0x21, 0x00, 0x01, 0x00, 0x01,
    ];
    let pkt = create_ts_packet(true, 1, false, &payload);
    let pts = extract_ts_pts(&pkt).unwrap();
    assert_eq!(pts, 0); // All bit fields evaluate to 0
}

#[test]
fn test_extract_ts_pts_with_adaptation_field() {
    use app_lib::recorder::extract_ts_pts;
    let payload = vec![
        0x00, 0x00, 0x01, 0xE0,
        0x00, 0x00, 0x80, 0x80, 0x05,
        0x23, 0x80, 0x81, 0x80, 0x81,
    ];
    let pkt = create_ts_packet(true, 3, false, &payload);
    let pts = extract_ts_pts(&pkt);
    assert!(pts.is_some());
    assert!(pts.unwrap() > 0);
}

// ──────────────────────────────────────────────────────────────────────────────
// Additional H.264 NAL Unit Classification Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_h264_nal_types_2_3_4_slice_partitions_not_keyframe() {
    for nal_type in 2..=4u8 {
        let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, nal_type]);
        assert!(!is_ts_keyframe(&pkt), "H264 NAL type {} should not be keyframe", nal_type);
    }
}

#[test]
fn test_h264_nal_types_10_11_12_eos_eob_filler_not_keyframe() {
    for nal_type in [10u8, 11, 12] {
        let byte = 0x60 | nal_type; // 0x60 = nal_ref_idc non-zero
        let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, byte]);
        assert!(!is_ts_keyframe(&pkt), "H264 NAL type {} should not be keyframe", nal_type);
    }
}

#[test]
fn test_h264_nal_types_14_to_18_prefixes_not_keyframe() {
    for nal_type in 14..=18u8 {
        let byte = 0x60 | nal_type;
        let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, byte]);
        assert!(!is_ts_keyframe(&pkt), "H264 NAL type {} should not be keyframe", nal_type);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Additional HEVC NAL Unit Classification Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_hevc_nal_non_keyframe_types_0_to_15() {
    for nal_type in [0u8, 1, 3, 9] {
        let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, nal_type << 1]);
        assert!(!is_ts_keyframe(&pkt), "HEVC NAL type {} should not be keyframe", nal_type);
    }
}

#[test]
fn test_hevc_nal_non_keyframe_prefix_sei_type_39() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, 39 << 1]);
    assert!(!is_ts_keyframe(&pkt));
}

// ──────────────────────────────────────────────────────────────────────────────
// Additional AV1 OBU Header Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_av1_obu_metadata_5_not_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, (5 << 3) | 0x01]);
    assert!(!is_ts_keyframe(&pkt));
}

#[test]
fn test_av1_obu_redundant_frame_header_7_not_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, (7 << 3) | 0x01]);
    assert!(!is_ts_keyframe(&pkt));
}

#[test]
fn test_av1_obu_tile_list_8_not_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, (8 << 3) | 0x01]);
    assert!(!is_ts_keyframe(&pkt));
}

#[test]
fn test_av1_obu_padding_15_not_keyframe() {
    let pkt = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, (15 << 3) | 0x01]);
    assert!(!is_ts_keyframe(&pkt));
}

// ──────────────────────────────────────────────────────────────────────────────
// Additional In-Memory Buffer Stress & Edge Case Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_buffer_extract_empty_returns_zero_length() {
    let buf = InMemoryVideoBuffer::new(4096);
    let extracted = buf.extract(30.0);
    assert_eq!(extracted.data.len(), 0);
    assert_eq!(extracted.duration_secs, 0.0);
}

#[test]
fn test_buffer_extract_duration_exceeding_total_buffer() {
    let mut buf = InMemoryVideoBuffer::new(20480);
    let key = create_ts_packet(true, 3, true, &[0x00]);
    let p = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x41]);

    buf.push_bytes(&key);
    for _ in 0..10 { buf.push_bytes(&p); }

    let extracted = buf.extract(99999.0);
    assert_eq!(extracted.data.len(), 188 * 11);
}

#[test]
fn test_buffer_multiple_consecutive_extractions() {
    let mut buf = InMemoryVideoBuffer::new(20480);
    let key = create_ts_packet(true, 3, true, &[0x00]);
    let p = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x41]);

    buf.push_bytes(&key);
    for _ in 0..5 { buf.push_bytes(&p); }

    let ex1 = buf.extract(10.0);
    let ex2 = buf.extract(10.0);
    assert_eq!(ex1.data.len(), ex2.data.len());
    assert_eq!(ex1.data, ex2.data);
}

#[test]
fn test_buffer_push_multi_packet_datagrams_1316_bytes() {
    // 7 TS packets in one UDP/network chunk (7 * 188 = 1316 bytes)
    let mut buf = InMemoryVideoBuffer::new(40960);
    let key = create_ts_packet(true, 3, true, &[0x00]);
    let p = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x41]);

    let mut chunk = Vec::new();
    chunk.extend_from_slice(&key);
    for _ in 0..6 { chunk.extend_from_slice(&p); }

    buf.push_bytes(&chunk);
    assert_eq!(buf.current_bytes, 1316);
    assert_eq!(buf.chunks.len(), 1);
}

#[test]
fn test_buffer_timestamp_monotonicity() {
    let mut buf = InMemoryVideoBuffer::new(40960);
    let p = create_ts_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x41]);

    for _ in 0..5 {
        buf.push_bytes(&p);
    }

    for i in 0..buf.chunks.len() - 1 {
        assert!(buf.chunks[i].timestamp <= buf.chunks[i + 1].timestamp);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Capture Graph Combinatorial Resolution & Encoder Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_capture_graph_all_supported_resolutions_cuda() {
    for res in ["1440p", "1080p", "900p", "720p", "540p", "480p", "360p", "240p"] {
        let g = capture_graph("h264_nvenc", "60", res, 2160, false, &ScalingMethod::Cuda, 0, false);
        let h_str = res.trim_end_matches('p');
        assert!(g.contains(&format!("scale=-2:{h_str}")), "Resolution {} missing scale", res);
    }
}

#[test]
fn test_capture_graph_all_supported_resolutions_qsv() {
    for res in ["1440p", "1080p", "900p", "720p", "540p", "480p", "360p", "240p"] {
        let g = capture_graph("h264_qsv", "60", res, 2160, false, &ScalingMethod::Qsv, 0, false);
        let h_str = res.trim_end_matches('p');
        assert!(g.contains(&format!("scale=-2:{h_str}")), "Resolution {} missing scale", res);
    }
}

#[test]
fn test_capture_graph_all_supported_resolutions_cpu_fallback() {
    for res in ["1440p", "1080p", "900p", "720p", "540p", "480p", "360p", "240p"] {
        let g = capture_graph("libx264", "60", res, 2160, false, &ScalingMethod::CpuFallback, 0, false);
        let h_str = res.trim_end_matches('p');
        assert!(g.contains(&format!("scale=-2:{h_str}")), "Resolution {} missing swscale", res);
    }
}

#[test]
fn test_capture_graph_native_source_never_downscales() {
    let g_cuda = capture_graph("h264_nvenc", "60", "Original", 1080, false, &ScalingMethod::Cuda, 0, false);
    assert!(!g_cuda.contains("scale=-2"));

    let g_qsv = capture_graph("h264_qsv", "60", "Original", 1080, false, &ScalingMethod::Qsv, 0, false);
    assert!(!g_qsv.contains("scale=-2"));

    let g_amf = capture_graph("h264_amf", "60", "Original", 1080, false, &ScalingMethod::D3d11Direct, 0, false);
    assert!(!g_amf.contains("scale=-2"));
}

#[test]
fn test_capture_graph_when_target_res_equal_to_monitor() {
    let g = capture_graph("h264_nvenc", "60", "1080p", 1080, false, &ScalingMethod::Cuda, 0, false);
    // When monitor is 1080p and target is 1080p, no downscale filter is applied
    assert!(!g.contains("scale=-2"));
    assert!(g.contains("format=nv12"));
}

#[test]
fn test_capture_graph_all_monitor_indices_0_to_4() {
    for idx in 0..=4 {
        let g = capture_graph("h264_nvenc", "60", "Original", 1080, false, &ScalingMethod::Cuda, idx, false);
        assert!(g.contains(&format!("output_idx={idx}")));
    }
}

#[test]
fn test_capture_graph_all_standard_framerates() {
    for fps in ["24", "30", "45", "60", "90", "120", "144", "165", "240", "360"] {
        let g = capture_graph("h264_nvenc", fps, "Original", 1080, false, &ScalingMethod::Cuda, 0, false);
        assert!(g.contains(&format!("framerate={fps}")));
    }
}


