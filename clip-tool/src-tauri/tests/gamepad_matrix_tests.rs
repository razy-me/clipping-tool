// ──────────────────────────────────────────────────────────────────────────────
// Gamepad Matrix Tests: False Positives & Button Combinations
// ──────────────────────────────────────────────────────────────────────────────

fn is_gamepad_clipping_combo(btns: u16) -> bool {
    // Combo 1: LB (0x0100) + RB (0x0200) + D-Pad Down (0x0002)
    let combo1 = (btns & 0x0100 != 0) && (btns & 0x0200 != 0) && (btns & 0x0002 != 0);
    // Combo 2: Back/View (0x0020) + Start (0x0010)
    let combo2 = (btns & 0x0020 != 0) && (btns & 0x0010 != 0);
    combo1 || combo2
}

const DPAD_UP: u16 = 0x0001;
const DPAD_DOWN: u16 = 0x0002;
const DPAD_LEFT: u16 = 0x0004;
const DPAD_RIGHT: u16 = 0x0008;
const START: u16 = 0x0010;
const BACK: u16 = 0x0020;
const LEFT_THUMB: u16 = 0x0040;
const RIGHT_THUMB: u16 = 0x0080;
const LEFT_SHOULDER: u16 = 0x0100;
const RIGHT_SHOULDER: u16 = 0x0200;
const A_BUTTON: u16 = 0x1000;
const B_BUTTON: u16 = 0x2000;
const X_BUTTON: u16 = 0x4000;
const Y_BUTTON: u16 = 0x8000;

#[test]
fn test_gamepad_single_buttons_never_trigger() {
    let all_buttons = [
        DPAD_UP, DPAD_DOWN, DPAD_LEFT, DPAD_RIGHT,
        START, BACK, LEFT_THUMB, RIGHT_THUMB,
        LEFT_SHOULDER, RIGHT_SHOULDER,
        A_BUTTON, B_BUTTON, X_BUTTON, Y_BUTTON,
    ];

    for btn in all_buttons {
        assert!(!is_gamepad_clipping_combo(btn), "Button 0x{:04X} alone should not trigger clip", btn);
    }
}

#[test]
fn test_gamepad_standard_gameplay_actions_never_trigger() {
    let combat_combos = [
        A_BUTTON | X_BUTTON,                   // Jump + Attack
        B_BUTTON | Y_BUTTON,                   // Dodge + Heavy
        LEFT_SHOULDER | A_BUTTON,              // Block + Jump
        RIGHT_SHOULDER | B_BUTTON,             // Shoot + Reload
        LEFT_THUMB | RIGHT_THUMB,              // Sprint + Crouch
        DPAD_UP | DPAD_RIGHT,                  // Weapon select
        DPAD_LEFT | DPAD_DOWN,                 // Emote / Ping
        LEFT_SHOULDER | RIGHT_SHOULDER,        // Dual Aim (without D-pad down!)
        LEFT_SHOULDER | DPAD_DOWN,             // LB + Down (without RB!)
        RIGHT_SHOULDER | DPAD_DOWN,            // RB + Down (without LB!)
        BACK | A_BUTTON,                       // Map + Select
        START | B_BUTTON,                      // Pause + Cancel
    ];

    for combo in combat_combos {
        assert!(!is_gamepad_clipping_combo(combo), "Standard gameplay combo 0x{:04X} falsely triggered", combo);
    }
}

#[test]
fn test_gamepad_combo1_with_all_other_buttons() {
    let combo1_base = LEFT_SHOULDER | RIGHT_SHOULDER | DPAD_DOWN;

    let other_buttons = [
        DPAD_UP, DPAD_LEFT, DPAD_RIGHT,
        START, BACK, LEFT_THUMB, RIGHT_THUMB,
        A_BUTTON, B_BUTTON, X_BUTTON, Y_BUTTON,
    ];

    for other in other_buttons {
        let combined = combo1_base | other;
        assert!(is_gamepad_clipping_combo(combined), "Combo 1 + 0x{:04X} must trigger", other);
    }
}

#[test]
fn test_gamepad_combo2_with_all_other_buttons() {
    let combo2_base = BACK | START;

    let other_buttons = [
        DPAD_UP, DPAD_DOWN, DPAD_LEFT, DPAD_RIGHT,
        LEFT_THUMB, RIGHT_THUMB, LEFT_SHOULDER, RIGHT_SHOULDER,
        A_BUTTON, B_BUTTON, X_BUTTON, Y_BUTTON,
    ];

    for other in other_buttons {
        let combined = combo2_base | other;
        assert!(is_gamepad_clipping_combo(combined), "Combo 2 + 0x{:04X} must trigger", other);
    }
}
