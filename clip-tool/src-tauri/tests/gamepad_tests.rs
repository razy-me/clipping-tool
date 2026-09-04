// ──────────────────────────────────────────────────────────────────────────────
// Gamepad & XInput Controller Clipping Combos Test Suite
// ──────────────────────────────────────────────────────────────────────────────

fn is_gamepad_clipping_combo(btns: u16) -> bool {
    // Combo 1: LB (0x0100) + RB (0x0200) + D-Pad Down (0x0002)
    let combo1 = (btns & 0x0100 != 0) && (btns & 0x0200 != 0) && (btns & 0x0002 != 0);
    // Combo 2: Back/View (0x0020) + Start (0x0010)
    let combo2 = (btns & 0x0020 != 0) && (btns & 0x0010 != 0);
    combo1 || combo2
}

#[test]
fn test_gamepad_combo_1_exact_lb_rb_dpad_down() {
    let btns = 0x0100 | 0x0200 | 0x0002;
    assert!(is_gamepad_clipping_combo(btns));
}

#[test]
fn test_gamepad_combo_1_with_other_buttons_pressed() {
    // LB + RB + DPad Down + A Button (0x1000)
    let btns = 0x0100 | 0x0200 | 0x0002 | 0x1000;
    assert!(is_gamepad_clipping_combo(btns));
}

#[test]
fn test_gamepad_combo_2_exact_back_start() {
    let btns = 0x0020 | 0x0010;
    assert!(is_gamepad_clipping_combo(btns));
}

#[test]
fn test_gamepad_combo_2_with_other_buttons_pressed() {
    // Back + Start + Right Thumbstick Button (0x0080)
    let btns = 0x0020 | 0x0010 | 0x0080;
    assert!(is_gamepad_clipping_combo(btns));
}

#[test]
fn test_gamepad_incomplete_combo_lb_rb_only() {
    let btns = 0x0100 | 0x0200; // Missing D-Pad Down
    assert!(!is_gamepad_clipping_combo(btns));
}

#[test]
fn test_gamepad_incomplete_combo_lb_dpad_down_only() {
    let btns = 0x0100 | 0x0002; // Missing RB
    assert!(!is_gamepad_clipping_combo(btns));
}

#[test]
fn test_gamepad_incomplete_combo_rb_dpad_down_only() {
    let btns = 0x0200 | 0x0002; // Missing LB
    assert!(!is_gamepad_clipping_combo(btns));
}

#[test]
fn test_gamepad_incomplete_combo_back_only() {
    let btns = 0x0020; // Missing Start
    assert!(!is_gamepad_clipping_combo(btns));
}

#[test]
fn test_gamepad_incomplete_combo_start_only() {
    let btns = 0x0010; // Missing Back
    assert!(!is_gamepad_clipping_combo(btns));
}

#[test]
fn test_gamepad_normal_face_buttons_no_combo() {
    for btn in [0x1000u16, 0x2000, 0x4000, 0x8000, 0x0001, 0x0004, 0x0008] {
        assert!(!is_gamepad_clipping_combo(btn));
    }
}

#[test]
fn test_gamepad_zero_buttons_no_combo() {
    assert!(!is_gamepad_clipping_combo(0));
}

#[test]
fn test_gamepad_debouncing_state_machine() {
    let mut was_pressed = false;
    let mut save_triggers = 0;

    let frames = vec![
        0,                               // Frame 1: Released
        0x0100 | 0x0200 | 0x0002,        // Frame 2: Pressed (Trigger 1)
        0x0100 | 0x0200 | 0x0002,        // Frame 3: Held Down (No repeat)
        0x0100 | 0x0200 | 0x0002,        // Frame 4: Held Down (No repeat)
        0,                               // Frame 5: Released
        0,                               // Frame 6: Released
        0x0020 | 0x0010,                 // Frame 7: Pressed Back+Start (Trigger 2)
        0x0020 | 0x0010,                 // Frame 8: Held Down (No repeat)
        0,                               // Frame 9: Released
    ];

    for btns in frames {
        let is_down = is_gamepad_clipping_combo(btns);
        if is_down && !was_pressed {
            was_pressed = true;
            save_triggers += 1;
        } else if !is_down {
            was_pressed = false;
        }
    }

    assert_eq!(save_triggers, 2);
}
