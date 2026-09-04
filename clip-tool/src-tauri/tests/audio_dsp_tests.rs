// ──────────────────────────────────────────────────────────────────────────────
// Audio DSP, RMS Metrics & Gain Math Test Suite
// ──────────────────────────────────────────────────────────────────────────────

fn compute_rms_and_metric(samples: &[f32]) -> (f32, f32, f32) {
    if samples.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let mut peak: f32 = 0.0;
    let mut sum_sq: f32 = 0.0;
    for &s in samples {
        let a = s.abs();
        if a > peak { peak = a; }
        sum_sq += s * s;
    }
    let rms = (sum_sq / samples.len() as f32).sqrt();
    let metric = peak.max(rms * 1.6);
    (peak, rms, metric)
}

#[test]
fn test_dsp_silence_all_zeros() {
    let samples = vec![0.0f32; 4800];
    let (peak, rms, metric) = compute_rms_and_metric(&samples);
    assert_eq!(peak, 0.0);
    assert_eq!(rms, 0.0);
    assert_eq!(metric, 0.0);
}

#[test]
fn test_dsp_square_wave_rms_equals_peak() {
    let mut samples = Vec::new();
    for i in 0..4800 {
        samples.push(if i % 2 == 0 { 0.5f32 } else { -0.5f32 });
    }
    let (peak, rms, metric) = compute_rms_and_metric(&samples);
    assert!((peak - 0.5).abs() < 0.001);
    assert!((rms - 0.5).abs() < 0.001); // For square wave: RMS == amplitude
    assert!((metric - 0.5 * 1.6).abs() < 0.001);
}

#[test]
fn test_dsp_sine_wave_rms_relation() {
    let count = 48000;
    let amplitude = 0.8f32;
    let mut samples = Vec::with_capacity(count);
    for i in 0..count {
        let angle = (i as f32 / 48000.0) * 2.0 * std::f32::consts::PI * 440.0; // 440 Hz A4
        samples.push(angle.sin() * amplitude);
    }
    let (peak, rms, _) = compute_rms_and_metric(&samples);
    assert!((peak - 0.8).abs() < 0.005);
    // For sine wave: RMS == amplitude / sqrt(2) ≈ 0.8 * 0.707106 = 0.56568
    let expected_rms = amplitude / 2.0f32.sqrt();
    assert!((rms - expected_rms).abs() < 0.005, "Expected RMS {}, got {}", expected_rms, rms);
}

#[test]
fn test_dsp_isolated_single_transient_peak() {
    let mut samples = vec![0.0f32; 4800];
    samples[100] = 1.0; // Single spike
    let (peak, rms, metric) = compute_rms_and_metric(&samples);
    assert_eq!(peak, 1.0);
    assert!(rms < 0.05); // RMS is very small because spike is brief
    assert_eq!(metric, 1.0); // Peak dominates because peak > rms * 1.6
}

#[test]
fn test_dsp_gain_scaling_unity_gain_is_exact() {
    let input = vec![0.1f32, -0.2, 0.3, -0.4, 0.5];
    let gain = 1.0f32;
    let output: Vec<f32> = input.iter().map(|&s| s * gain).collect();
    assert_eq!(input, output);
}

#[test]
fn test_dsp_gain_scaling_boost_multiplier() {
    let input = vec![0.1f32, -0.2, 0.3, -0.4, 0.5];
    let gain = 2.0f32;
    let output: Vec<f32> = input.iter().map(|&s| s * gain).collect();
    assert_eq!(output, vec![0.2f32, -0.4, 0.6, -0.8, 1.0]);
}

#[test]
fn test_dsp_gain_scaling_mute_multiplier() {
    let input = vec![0.1f32, -0.2, 0.3, -0.4, 0.5];
    let gain = 0.0f32;
    let output: Vec<f32> = input.iter().map(|&s| s * gain).collect();
    assert_eq!(output, vec![0.0f32; 5]);
}
