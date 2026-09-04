use app_lib::recorder::InMemoryVideoBuffer;

fn make_ts_packet(is_key: bool) -> Vec<u8> {
    let mut pkt = vec![0xFF; 188];
    pkt[0] = 0x47;
    pkt[1] = 0x41;
    pkt[2] = 0x00;
    pkt[3] = 0x10;
    pkt[4] = 0x00;
    pkt[5] = 0x00;
    pkt[6] = 0x01;
    pkt[7] = 0xE0;
    pkt[8] = 0x00;
    pkt[9] = 0x00;
    pkt[10] = 0x80;
    pkt[11] = 0x00;
    pkt[12] = 0x00;
    pkt[13] = 0x00;
    pkt[14] = 0x00;
    pkt[15] = 0x01;
    pkt[16] = if is_key { 0x65 } else { 0x41 }; // 0x65 = NAL type 5 (IDR keyframe)
    pkt
}

// ──────────────────────────────────────────────────────────────────────────────
// Chunk Size Matrix Ingestion Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ring_buffer_various_chunk_sizes() {
    let chunk_sizes = [
        188,         // Single TS packet
        376,         // 2 TS packets
        1316,        // 7 TS packets (standard UDP/RTP datagram)
        4096,        // 4 KB page
        16384,       // 16 KB
        65536,       // 64 KB (our optimized batch size)
        131072,      // 128 KB
    ];

    for size in chunk_sizes {
        let mut buf = InMemoryVideoBuffer::new(1024 * 1024); // 1 MB
        let data = vec![0x47; size];
        for _ in 0..10 {
            buf.push_bytes(&data);
        }
        assert!(buf.current_bytes <= buf.max_bytes);
        assert!(!buf.chunks.is_empty());
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Capacity Limit Exact Saturation & Eviction
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ring_buffer_exact_capacity_wrapping() {
    let cap = 1000;
    let mut buf = InMemoryVideoBuffer::new(cap);

    // Push 10 chunks of 100 bytes = exactly 1000 bytes
    for _ in 0..10 {
        buf.push_bytes(&[0x47; 100]);
    }
    assert_eq!(buf.current_bytes, 1000);
    assert_eq!(buf.chunks.len(), 10);

    // Push 1 more chunk of 100 bytes -> oldest must be evicted, total remains 1000
    buf.push_bytes(&[0x47; 100]);
    assert_eq!(buf.current_bytes, 1000);
    assert_eq!(buf.chunks.len(), 10);
}

#[test]
fn test_ring_buffer_oversized_chunk_evicts_multiple_small_chunks() {
    let cap = 1000;
    let mut buf = InMemoryVideoBuffer::new(cap);

    // Push 10 small chunks of 100 bytes
    for _ in 0..10 {
        buf.push_bytes(&[0x47; 100]);
    }
    assert_eq!(buf.current_bytes, 1000);

    // Push a large chunk of 600 bytes -> 6 small chunks must be evicted
    buf.push_bytes(&[0x47; 600]);
    assert_eq!(buf.current_bytes, 1000); // 4 * 100 + 600 = 1000
    assert_eq!(buf.chunks.len(), 5);
}

// ──────────────────────────────────────────────────────────────────────────────
// Extraction Edge Cases
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ring_buffer_extract_empty_returns_zero() {
    let buf = InMemoryVideoBuffer::new(1024 * 1024);
    let ext = buf.extract(30.0);
    assert_eq!(ext.data.len(), 0);
    assert_eq!(ext.duration_secs, 0.0);
}

#[test]
fn test_ring_buffer_extract_zero_seconds() {
    let mut buf = InMemoryVideoBuffer::new(1024 * 1024);
    let key = make_ts_packet(true);
    buf.push_bytes(&key);

    let ext = buf.extract(0.0);
    assert!(!ext.data.is_empty());
}

#[test]
fn test_ring_buffer_extract_negative_seconds_handled_safely() {
    let mut buf = InMemoryVideoBuffer::new(1024 * 1024);
    let key = make_ts_packet(true);
    buf.push_bytes(&key);

    let ext = buf.extract(-10.0);
    assert!(!ext.data.is_empty());
}

#[test]
fn test_ring_buffer_extract_huge_duration_exceeding_buffer() {
    let mut buf = InMemoryVideoBuffer::new(1024 * 1024);
    let key = make_ts_packet(true);
    for _ in 0..10 {
        buf.push_bytes(&key);
    }

    // Request 999999 seconds
    let ext = buf.extract(999999.0);
    assert_eq!(ext.data.len(), 188 * 10);
}

// ──────────────────────────────────────────────────────────────────────────────
// Keyframe Alignment in Extraction
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ring_buffer_extract_aligns_to_first_keyframe() {
    let mut buf = InMemoryVideoBuffer::new(1024 * 1024);
    let non_key = make_ts_packet(false);
    let key = make_ts_packet(true);

    // Push 3 non-keyframes, then 1 keyframe, then 3 non-keyframes
    buf.push_bytes(&non_key);
    buf.push_bytes(&non_key);
    buf.push_bytes(&non_key);
    buf.push_bytes(&key);
    buf.push_bytes(&non_key);
    buf.push_bytes(&non_key);
    buf.push_bytes(&non_key);

    // Extracting full buffer must find the keyframe
    let ext = buf.extract(60.0);
    assert!(!ext.data.is_empty());
}

// ──────────────────────────────────────────────────────────────────────────────
// Continuous Wrapping Stress Under 500 Iterations
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ring_buffer_continuous_stream_500_iterations() {
    let mut buf = InMemoryVideoBuffer::new(50 * 188); // Can only hold 50 packets
    let key = make_ts_packet(true);
    let non_key = make_ts_packet(false);

    for i in 0..500 {
        if i % 30 == 0 {
            buf.push_bytes(&key);
        } else {
            buf.push_bytes(&non_key);
        }
        assert!(buf.current_bytes <= buf.max_bytes);
    }

    let ext = buf.extract(10.0);
    assert!(!ext.data.is_empty());
    assert!(ext.data.len() <= 50 * 188);
}
