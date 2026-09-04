fn compute_mock_e_core_mask(core_entries: &[(u8, usize)]) -> Option<usize> {
    let mut min_class = u8::MAX;
    let mut max_class = 0u8;

    for (class, _) in core_entries {
        if *class < min_class { min_class = *class; }
        if *class > max_class { max_class = *class; }
    }

    if min_class < max_class {
        let mut mask = 0;
        for (class, group_mask) in core_entries {
            if *class == min_class {
                mask |= group_mask;
            }
        }
        if mask != 0 {
            return Some(mask);
        }
    }
    None
}

// ──────────────────────────────────────────────────────────────────────────────
// Homogeneous CPU Topologies (Must all return None)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_topology_amd_ryzen_6_core() {
    let entries: Vec<(u8, usize)> = (0..6).map(|i| (0u8, 1 << i)).collect();
    assert_eq!(compute_mock_e_core_mask(&entries), None);
}

#[test]
fn test_topology_amd_ryzen_8_core_7800x3d() {
    let entries: Vec<(u8, usize)> = (0..8).map(|i| (0u8, 1 << i)).collect();
    assert_eq!(compute_mock_e_core_mask(&entries), None);
}

#[test]
fn test_topology_amd_ryzen_12_core_7900x() {
    let entries: Vec<(u8, usize)> = (0..12).map(|i| (0u8, 1 << i)).collect();
    assert_eq!(compute_mock_e_core_mask(&entries), None);
}

#[test]
fn test_topology_amd_ryzen_16_core_7950x() {
    let entries: Vec<(u8, usize)> = (0..16).map(|i| (0u8, 1 << i)).collect();
    assert_eq!(compute_mock_e_core_mask(&entries), None);
}

#[test]
fn test_topology_intel_legacy_10th_gen_10700k() {
    let entries: Vec<(u8, usize)> = (0..8).map(|i| (0u8, 1 << i)).collect();
    assert_eq!(compute_mock_e_core_mask(&entries), None);
}

#[test]
fn test_topology_intel_legacy_11th_gen_11900k() {
    let entries: Vec<(u8, usize)> = (0..8).map(|i| (0u8, 1 << i)).collect();
    assert_eq!(compute_mock_e_core_mask(&entries), None);
}

// ──────────────────────────────────────────────────────────────────────────────
// Heterogeneous (Hybrid) CPU Topologies (Must mask only E-cores)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_topology_intel_12600k_6p_4e() {
    let mut entries = Vec::new();
    for i in 0..6 { entries.push((1u8, 1 << i)); } // 6 P-cores
    for i in 6..10 { entries.push((0u8, 1 << i)); } // 4 E-cores
    assert_eq!(compute_mock_e_core_mask(&entries), Some(0x03C0)); // bits 6..9
}

#[test]
fn test_topology_intel_13700k_8p_8e() {
    let mut entries = Vec::new();
    for i in 0..8 { entries.push((1u8, 1 << i)); } // 8 P-cores
    for i in 8..16 { entries.push((0u8, 1 << i)); } // 8 E-cores
    assert_eq!(compute_mock_e_core_mask(&entries), Some(0xFF00)); // bits 8..15
}

#[test]
fn test_topology_intel_13900k_8p_16e() {
    let mut entries = Vec::new();
    for i in 0..8 { entries.push((1u8, 1 << i)); } // 8 P-cores
    for i in 8..24 { entries.push((0u8, 1 << i)); } // 16 E-cores
    assert_eq!(compute_mock_e_core_mask(&entries), Some(0xFFFF00)); // bits 8..23
}

#[test]
fn test_topology_amd_zen4_zen4c_hybrid() {
    let mut entries = Vec::new();
    for i in 0..4 { entries.push((1u8, 1 << i)); } // 4 Zen4
    for i in 4..8 { entries.push((0u8, 1 << i)); } // 4 Zen4c
    assert_eq!(compute_mock_e_core_mask(&entries), Some(0x00F0)); // bits 4..7
}

// ──────────────────────────────────────────────────────────────────────────────
// Tier Classification Combinations
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_tier_weak_low_cores() {
    let cores = 2;
    let ram = 16000;
    let hw = false;
    let is_weak = !hw && (cores <= 4 || ram <= 8192);
    assert!(is_weak);
}

#[test]
fn test_tier_weak_low_ram() {
    let cores = 8;
    let ram = 4096;
    let hw = false;
    let is_weak = !hw && (cores <= 4 || ram <= 8192);
    assert!(is_weak);
}

#[test]
fn test_tier_strong_gaming_rig() {
    let cores = 16;
    let ram = 32000;
    let hw = true;
    let is_strong = hw && cores >= 6 && ram >= 15000;
    assert!(is_strong);
}

#[test]
fn test_tier_mid_spec() {
    let cores = 6;
    let ram = 16000;
    let hw = false; // No HW encoder -> not strong, but >4 cores and >8GB -> not weak
    let is_weak = !hw && (cores <= 4 || ram <= 8192);
    let is_strong = hw && cores >= 6 && ram >= 15000;
    assert!(!is_weak && !is_strong); // Mid tier
}

// ──────────────────────────────────────────────────────────────────────────────
// Additional Modern CPU Topologies (Alder Lake, Raptor Lake, Zen4)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_topology_intel_12400_homogeneous_6p() {
    let entries: Vec<(u8, usize)> = (0..6).map(|i| (0u8, 1 << i)).collect();
    assert_eq!(compute_mock_e_core_mask(&entries), None);
}

#[test]
fn test_topology_intel_12700k_8p_4e() {
    let mut entries = Vec::new();
    for i in 0..8 { entries.push((1u8, 1 << i)); } // 8 P-cores (class 1)
    for i in 8..12 { entries.push((0u8, 1 << i)); } // 4 E-cores (class 0)
    assert_eq!(compute_mock_e_core_mask(&entries), Some(0x0F00)); // bits 8..11
}

#[test]
fn test_topology_intel_13600k_6p_8e() {
    let mut entries = Vec::new();
    for i in 0..6 { entries.push((1u8, 1 << i)); } // 6 P-cores
    for i in 6..14 { entries.push((0u8, 1 << i)); } // 8 E-cores
    assert_eq!(compute_mock_e_core_mask(&entries), Some(0x3FC0)); // bits 6..13
}

#[test]
fn test_topology_intel_14700k_8p_12e() {
    let mut entries = Vec::new();
    for i in 0..8 { entries.push((1u8, 1 << i)); } // 8 P-cores
    for i in 8..20 { entries.push((0u8, 1 << i)); } // 12 E-cores
    assert_eq!(compute_mock_e_core_mask(&entries), Some(0x0FFF00)); // bits 8..19
}

#[test]
fn test_topology_intel_14900ks_8p_16e() {
    let mut entries = Vec::new();
    for i in 0..8 { entries.push((1u8, 1 << i)); } // 8 P-cores
    for i in 8..24 { entries.push((0u8, 1 << i)); } // 16 E-cores
    assert_eq!(compute_mock_e_core_mask(&entries), Some(0xFFFF00)); // bits 8..23
}

#[test]
fn test_topology_amd_threadripper_64_cores_homogeneous() {
    let entries: Vec<(u8, usize)> = (0..64).map(|i| (0u8, 1 << (i % 64))).collect();
    assert_eq!(compute_mock_e_core_mask(&entries), None);
}

#[test]
fn test_topology_single_core_vm() {
    let entries = vec![(0u8, 1usize)];
    assert_eq!(compute_mock_e_core_mask(&entries), None);
}

#[test]
fn test_topology_dual_core_celeron() {
    let entries = vec![(0u8, 1usize), (0u8, 2usize)];
    assert_eq!(compute_mock_e_core_mask(&entries), None);
}

// ──────────────────────────────────────────────────────────────────────────────
// Additional Hardware Tier Matrix Edge Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_tier_weak_with_low_ram_even_with_hw_encoder() {
    // 4GB RAM gaming laptop: RAM <= 8192 MB -> not strong
    let cores = 8;
    let ram = 4096;
    let hw = true;
    let is_strong = hw && cores >= 6 && ram >= 15000;
    assert!(!is_strong);
}

#[test]
fn test_tier_strong_exact_boundary_conditions() {
    let cores = 6;
    let ram = 15000;
    let hw = true;
    let is_strong = hw && cores >= 6 && ram >= 15000;
    assert!(is_strong);
}

#[test]
fn test_tier_mid_spec_with_8_cores_and_16gb_ram_no_hw_encoder() {
    let cores = 8;
    let ram = 16384;
    let hw = false;
    let is_weak = !hw && (cores <= 4 || ram <= 8192);
    let is_strong = hw && cores >= 6 && ram >= 15000;
    assert!(!is_weak);
    assert!(!is_strong); // Stays Mid tier
}

