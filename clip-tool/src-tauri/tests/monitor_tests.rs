use app_lib::wgc_recorder::MonitorInfo;

// ──────────────────────────────────────────────────────────────────────────────
// Multi-Monitor Geometry, DPI & Aspect Ratio Test Suite
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_monitor_info_serialization_roundtrip() {
    let mon = MonitorInfo {
        index: 0,
        name: "Monitor 1 (2560×1440) - Hauptbildschirm".to_string(),
        width: 2560,
        height: 1440,
        is_primary: true,
    };
    let json = serde_json::to_string(&mon).unwrap();
    let decoded: MonitorInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.index, 0);
    assert_eq!(decoded.width, 2560);
    assert_eq!(decoded.height, 1440);
    assert!(decoded.is_primary);
}

#[test]
fn test_monitor_aspect_ratio_calculations() {
    fn compute_aspect_ratio(w: u32, h: u32) -> (u32, u32) {
        fn gcd(a: u32, b: u32) -> u32 {
            if b == 0 { a } else { gcd(b, a % b) }
        }
        let g = gcd(w, h);
        (w / g, h / g)
    }

    assert_eq!(compute_aspect_ratio(1920, 1080), (16, 9));
    assert_eq!(compute_aspect_ratio(2560, 1440), (16, 9));
    assert_eq!(compute_aspect_ratio(3840, 2160), (16, 9));
    assert_eq!(compute_aspect_ratio(1920, 1200), (8, 5)); // 16:10
    assert_eq!(compute_aspect_ratio(2560, 1600), (8, 5)); // 16:10
    assert_eq!(compute_aspect_ratio(3440, 1440), (43, 18)); // 21:9 ultrawide
    assert_eq!(compute_aspect_ratio(5120, 1440), (32, 9)); // 32:9 super ultrawide
    assert_eq!(compute_aspect_ratio(1080, 1920), (9, 16)); // Portrait stream monitor
}

#[test]
fn test_monitor_bounds_selection_fallback() {
    let monitors = vec![
        MonitorInfo { index: 0, name: "Mon 1".into(), width: 1920, height: 1080, is_primary: true },
        MonitorInfo { index: 1, name: "Mon 2".into(), width: 2560, height: 1440, is_primary: false },
    ];

    fn get_safe_monitor<'a>(list: &'a [MonitorInfo], selected_idx: u32) -> &'a MonitorInfo {
        list.iter()
            .find(|m| m.index == selected_idx)
            .or_else(|| list.iter().find(|m| m.is_primary))
            .unwrap_or(&list[0])
    }

    assert_eq!(get_safe_monitor(&monitors, 0).width, 1920);
    assert_eq!(get_safe_monitor(&monitors, 1).width, 2560);
    assert_eq!(get_safe_monitor(&monitors, 5).width, 1920); // Out of bounds falls back to primary
}

#[test]
fn test_monitor_pixel_density_megapixels() {
    fn megapixels(w: u32, h: u32) -> f64 {
        (w as f64 * h as f64) / 1_000_000.0
    }

    assert!((megapixels(1920, 1080) - 2.0736).abs() < 0.001);
    assert!((megapixels(2560, 1440) - 3.6864).abs() < 0.001);
    assert!((megapixels(3840, 2160) - 8.2944).abs() < 0.001);
}

#[test]
fn test_monitor_label_formatting() {
    let m_prim = MonitorInfo {
        index: 0,
        name: format!("Monitor {} ({}×{}) - Hauptbildschirm", 1, 1920, 1080),
        width: 1920,
        height: 1080,
        is_primary: true,
    };
    assert!(m_prim.name.contains("Hauptbildschirm"));

    let m_sec = MonitorInfo {
        index: 1,
        name: format!("Monitor {} ({}×{})", 2, 2560, 1440),
        width: 2560,
        height: 1440,
        is_primary: false,
    };
    assert!(!m_sec.name.contains("Hauptbildschirm"));
    assert!(m_sec.name.contains("2560×1440"));
}
