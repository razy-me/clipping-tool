use app_lib::recorder::{is_ts_keyframe, extract_ts_pts, find_first_ts_keyframe_offset};

fn create_packet(pusi: bool, adaptation_control: u8, rai: bool, payload: &[u8]) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(188);
    pkt.push(0x47); // Sync byte
    let b1 = if pusi { 0x40 } else { 0x00 } | 0x01; // PID high = 1
    pkt.push(b1);
    pkt.push(0x00); // PID low
    let b3 = (adaptation_control & 0x03) << 4;
    pkt.push(b3);

    match adaptation_control {
        2 | 3 => {
            let af_len = if rai { 1 } else { 0 };
            pkt.push(af_len);
            if rai {
                pkt.push(0x40); // RAI bit 6
            }
        }
        _ => {}
    }

    pkt.extend_from_slice(payload);
    while pkt.len() < 188 {
        pkt.push(0xFF); // Stuffing bytes
    }
    pkt.truncate(188);
    pkt
}

// ──────────────────────────────────────────────────────────────────────────────
// Sync Byte Robustness Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_sync_byte_corruptions_rejected() {
    let bad_syncs: [u8; 16] = [
        0x00, 0x01, 0x46, 0x48, 0x7F, 0x80, 0xA5, 0xC3,
        0xE0, 0xF0, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE, 0xFF,
    ];

    for sync in bad_syncs {
        let mut pkt = create_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, 0x05]);
        pkt[0] = sync;
        assert!(!is_ts_keyframe(&pkt), "Packet with sync 0x{:02X} should not be recognized", sync);
        assert_eq!(extract_ts_pts(&pkt), None, "PTS on sync 0x{:02X} should fail", sync);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Truncated & Partial Packet Lengths
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_sub_188_byte_packets_safe_and_ignored() {
    let valid = create_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, 0x05]);

    for len in 0..188 {
        let truncated = &valid[..len];
        assert!(!is_ts_keyframe(truncated), "Truncated packet of len {} should return false", len);
        assert_eq!(extract_ts_pts(truncated), None, "PTS on truncated len {} should return None", len);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// PES Video Stream ID Matrix (0xE0 to 0xEF)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_pes_all_video_stream_ids_e0_to_ef() {
    for stream_id in 0xE0..=0xEFu8 {
        let payload = [
            0x00, 0x00, 0x01, stream_id,
            0x00, 0x00, 0x80, 0x80, 0x05,
            0x21, 0x00, 0x01, 0x00, 0x01,
            0x00, 0x00, 0x01, 0x05, // IDR slice
        ];
        let pkt = create_packet(true, 1, false, &payload);
        assert!(is_ts_keyframe(&pkt), "Stream ID 0x{:02X} should identify keyframe", stream_id);
        assert!(extract_ts_pts(&pkt).is_some(), "Stream ID 0x{:02X} should parse PTS", stream_id);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Non-Video Stream IDs with Audio Payload Must NOT Trigger Keyframe
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_pes_non_video_stream_ids_never_keyframe() {
    let non_video_ids = [
        0xBC, // program_stream_map
        0xBE, // padding_stream
        0xBF, // private_stream_2
        0xC0, 0xC1, 0xD0, 0xDF, // ISO/IEC 13818-3 / 11172-3 audio
        0xF0, // ECM
        0xF1, // EMM
        0xF2, // DSMCC_stream
        0xF8, // ISO/IEC 13522_stream
        0xFF, // program_stream_directory
    ];

    for stream_id in non_video_ids {
        let payload = [
            0x00, 0x00, 0x01, stream_id,
            0x00, 0x00, 0x80, 0x80, 0x05,
            0x21, 0x00, 0x01, 0x00, 0x01,
            0xFF, 0xF1, 0x50, 0x80, // Audio AAC ADTS header
        ];
        let pkt = create_packet(true, 1, false, &payload);
        assert!(!is_ts_keyframe(&pkt), "Non-video stream ID 0x{:02X} must not trigger keyframe", stream_id);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// H.264 NAL Unit Complete Matrix (0 through 31)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_h264_all_nal_units() {
    let keyframe_nals = [5u8, 7, 8];

    for nal in 0..=31u8 {
        let byte = 0x60 | nal;
        let payload = [
            0x00, 0x00, 0x01, 0xE0,
            0x00, 0x00, 0x80, 0x00, 0x00,
            0x00, 0x00, 0x01, byte,
        ];
        let pkt = create_packet(true, 1, false, &payload);
        let expected = keyframe_nals.contains(&nal);
        assert_eq!(
            is_ts_keyframe(&pkt),
            expected,
            "H.264 NAL {} expectation mismatch (expected {})",
            nal,
            expected
        );
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// HEVC / H.265 Specific Keyframe NAL Units (19, 20, 21, 32, 33, 34)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_hevc_keyframe_nal_units() {
    let keyframe_nals = [
        19u8, // IDR_W_RADL
        20,   // IDR_N_LP
        21,   // CRA_NUT
        32,   // VPS_NUT
        33,   // SPS_NUT
        34,   // PPS_NUT
    ];

    for nal in keyframe_nals {
        let byte = (nal << 1) & 0x7E;
        let payload = [
            0x00, 0x00, 0x01, 0xE0,
            0x00, 0x00, 0x80, 0x00, 0x00,
            0x00, 0x00, 0x01, byte, 0x01,
        ];
        let pkt = create_packet(true, 1, false, &payload);
        assert!(
            is_ts_keyframe(&pkt),
            "HEVC keyframe NAL {} should be recognized as keyframe",
            nal
        );
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// AV1 OBU Sequence Header vs Non-Keyframe OBUs
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_av1_sequence_header_is_keyframe() {
    // AV1 OBU 1 (Sequence Header): (1 << 3) = 0x08.
    // In our parser, (byte & 1) == 0 && ((byte >> 3) & 0x0F) == 1.
    // 0x08 has bit 0 == 0 and obu_type == 1.
    let payload = [
        0x00, 0x00, 0x01, 0xE0,
        0x00, 0x00, 0x80, 0x00, 0x00,
        0x00, 0x00, 0x01, 0x08, 0x00,
    ];
    let pkt = create_packet(true, 1, false, &payload);
    assert!(is_ts_keyframe(&pkt), "AV1 Sequence Header OBU 1 must be detected as keyframe");
}

#[test]
fn test_av1_non_keyframe_obus() {
    let non_key_obus = [
        (2u8, 0x10), // Temporal Delimiter
        (3u8, 0x18), // Frame Header
        (4u8, 0x20), // Tile Group
        (6u8, 0x30), // Frame
        (7u8, 0x38), // Redundant Frame Header
        (15u8, 0x78), // Padding
    ];

    for (obu, byte) in non_key_obus {
        // Ensure this byte does not trigger H.264 (5, 7, 8) or HEVC (19, 20, 21, 32, 33, 34)
        let h264_type = byte & 0x1F;
        let hevc_type = (byte >> 1) & 0x3F;
        let collides = h264_type == 5 || h264_type == 7 || h264_type == 8
            || hevc_type == 19 || hevc_type == 20 || hevc_type == 21 || hevc_type == 32 || hevc_type == 33 || hevc_type == 34;

        if !collides {
            let payload = [
                0x00, 0x00, 0x01, 0xE0,
                0x00, 0x00, 0x80, 0x00, 0x00,
                0x00, 0x00, 0x01, byte, 0x00,
            ];
            let pkt = create_packet(true, 1, false, &payload);
            assert!(!is_ts_keyframe(&pkt), "AV1 non-keyframe OBU {} (0x{:02X}) falsely triggered", obu, byte);
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// find_first_ts_keyframe_offset Search Exhaustive Checks
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_find_first_ts_keyframe_offset_at_various_packet_indices() {
    let non_key = create_packet(false, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x01]);
    let key = create_packet(true, 1, false, &[0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, 0x05]);

    for target_pkt_idx in [0, 1, 2, 5, 10, 20, 50] {
        let mut stream = Vec::new();
        for _ in 0..target_pkt_idx {
            stream.extend_from_slice(&non_key);
        }
        stream.extend_from_slice(&key);
        for _ in 0..5 {
            stream.extend_from_slice(&non_key);
        }

        let found = find_first_ts_keyframe_offset(&stream);
        assert_eq!(found, Some(target_pkt_idx * 188), "Failed for index {}", target_pkt_idx);
    }
}

#[test]
fn test_find_first_ts_keyframe_offset_empty_and_zero() {
    assert_eq!(find_first_ts_keyframe_offset(&[]), None);
    assert_eq!(find_first_ts_keyframe_offset(&[0x47; 100]), None);
}
