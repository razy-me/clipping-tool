use tauri_plugin_global_shortcut::Shortcut;

#[test]
fn test_valid_hotkey_alt_shift_c() {
    assert!("Alt+Shift+C".parse::<Shortcut>().is_ok());
}

#[test]
fn test_valid_hotkey_ctrl_shift_s() {
    assert!("Ctrl+Shift+S".parse::<Shortcut>().is_ok());
}

#[test]
fn test_valid_hotkey_function_keys() {
    assert!("F9".parse::<Shortcut>().is_ok());
    assert!("F10".parse::<Shortcut>().is_ok());
    assert!("F11".parse::<Shortcut>().is_ok());
    assert!("F12".parse::<Shortcut>().is_ok());
}

#[test]
fn test_valid_hotkey_ctrl_f12() {
    assert!("Ctrl+F12".parse::<Shortcut>().is_ok());
}

#[test]
fn test_valid_hotkey_alt_f10() {
    assert!("Alt+F10".parse::<Shortcut>().is_ok());
}

#[test]
fn test_valid_hotkey_ctrl_alt_delete_syntax() {
    assert!("Ctrl+Alt+C".parse::<Shortcut>().is_ok());
}

#[test]
fn test_invalid_hotkey_empty() {
    assert!("".parse::<Shortcut>().is_err());
}

#[test]
fn test_invalid_hotkey_gibberish() {
    assert!("NotAValidKeyCombination".parse::<Shortcut>().is_err());
    assert!("SuperHyperUltraKey".parse::<Shortcut>().is_err());
}

#[test]
fn test_invalid_hotkey_trailing_plus() {
    assert!("Ctrl+Alt+".parse::<Shortcut>().is_err());
}

// ──────────────────────────────────────────────────────────────────────────────
// Function Keys F1..F8 & Specialized Key Combinations
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_valid_hotkey_f1_to_f8() {
    assert!("F1".parse::<Shortcut>().is_ok());
    assert!("F2".parse::<Shortcut>().is_ok());
    assert!("F3".parse::<Shortcut>().is_ok());
    assert!("F4".parse::<Shortcut>().is_ok());
    assert!("F5".parse::<Shortcut>().is_ok());
    assert!("F6".parse::<Shortcut>().is_ok());
    assert!("F7".parse::<Shortcut>().is_ok());
    assert!("F8".parse::<Shortcut>().is_ok());
}

#[test]
fn test_valid_hotkey_ctrl_alt_letters() {
    for letter in ['A', 'B', 'D', 'E', 'G', 'H', 'K', 'L', 'M', 'P', 'R', 'T', 'V', 'X', 'Y', 'Z'] {
        assert!(format!("Ctrl+Alt+{letter}").parse::<Shortcut>().is_ok(), "Failed for letter {}", letter);
    }
}

#[test]
fn test_valid_hotkey_alt_shift_letters() {
    for letter in ['Q', 'W', 'E', 'R', 'T', 'Z', 'U', 'I', 'O', 'P'] {
        assert!(format!("Alt+Shift+{letter}").parse::<Shortcut>().is_ok());
    }
}

#[test]
fn test_valid_hotkey_three_modifiers_ctrl_shift_alt() {
    assert!("Ctrl+Shift+Alt+S".parse::<Shortcut>().is_ok());
    assert!("Ctrl+Shift+Alt+R".parse::<Shortcut>().is_ok());
    assert!("Ctrl+Shift+Alt+F12".parse::<Shortcut>().is_ok());
}

#[test]
fn test_valid_hotkey_numpad_keys() {
    assert!("Ctrl+Numpad0".parse::<Shortcut>().is_ok());
    assert!("Ctrl+Numpad1".parse::<Shortcut>().is_ok());
    assert!("Ctrl+Numpad5".parse::<Shortcut>().is_ok());
    assert!("Ctrl+Numpad9".parse::<Shortcut>().is_ok());
    assert!("Alt+NumpadAdd".parse::<Shortcut>().is_ok());
    assert!("Alt+NumpadSubtract".parse::<Shortcut>().is_ok());
}

#[test]
fn test_valid_hotkey_navigation_keys() {
    assert!("Ctrl+Insert".parse::<Shortcut>().is_ok());
    assert!("Ctrl+Delete".parse::<Shortcut>().is_ok());
    assert!("Alt+Home".parse::<Shortcut>().is_ok());
    assert!("Alt+End".parse::<Shortcut>().is_ok());
    assert!("Ctrl+PageUp".parse::<Shortcut>().is_ok());
    assert!("Ctrl+PageDown".parse::<Shortcut>().is_ok());
}

// ──────────────────────────────────────────────────────────────────────────────
// Additional Invalid Shortcut Error Handling Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_invalid_hotkey_standalone_plus() {
    assert!("+".parse::<Shortcut>().is_err());
}

#[test]
fn test_invalid_hotkey_double_plus() {
    assert!("Ctrl++S".parse::<Shortcut>().is_err());
}

#[test]
fn test_invalid_hotkey_modifier_only() {
    assert!("Ctrl".parse::<Shortcut>().is_err());
    assert!("Alt".parse::<Shortcut>().is_err());
    assert!("Shift".parse::<Shortcut>().is_err());
    assert!("Ctrl+Shift".parse::<Shortcut>().is_err());
}

#[test]
fn test_invalid_hotkey_random_unicode_emoji() {
    assert!("Ctrl+Alt+🔥".parse::<Shortcut>().is_err());
    assert!("🎮".parse::<Shortcut>().is_err());
}

