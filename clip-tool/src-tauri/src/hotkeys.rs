
use tauri::Emitter;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub fn register_all_hotkeys(app: &tauri::AppHandle, bindings: &[crate::config::HotkeyBinding]) -> Result<(), String> {
    let _ = app.global_shortcut().unregister_all();

    let mut seen = std::collections::HashSet::new();
    let mut registered_count = 0;
    for binding in bindings {
        let clean_str = binding.hotkey.trim();
        if clean_str.is_empty() { continue; }

        let normalized = clean_str.to_lowercase();
        if seen.contains(&normalized) {
            println!("[hotkey] duplicate shortcut '{}' in config skipped", clean_str);
            continue;
        }
        seen.insert(normalized);

        let shortcut = clean_str.parse::<Shortcut>().map_err(|e| {
            format!("Tastenkombination '{}' konnte nicht verarbeitet werden: {}", clean_str, e)
        })?;

        let duration = binding.duration_secs;
        app.global_shortcut().on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                println!("[hotkey] pressed (duration: {}s)", duration);
                let game = crate::audio::get_active_game_name();
                println!("[hotkey] active game: {}", game);

                let app_clone = app.clone();
                tauri::async_runtime::spawn(async move {
                    let cfg = crate::config::get_config(app_clone.clone());
                    let dur = if duration == 0 { cfg.buffer_length_secs } else { duration };
                    match crate::recorder::save_clip(app_clone.clone(), game, dur, cfg.custom_clip_path).await {
                        Ok(_) => println!("[hotkey] clip saved ({}s)", dur),
                        Err(e) => {
                            eprintln!("[hotkey] save error: {}", e);
                            crate::overlay::show_overlay_error(&app_clone, &format!("Fehler: {e}"));
                            let _ = app_clone.emit("clips://error", serde_json::json!({ "error": e }));
                        }
                    }
                });
            }
        }).map_err(|e| format!("Hotkey '{}' konnte nicht registriert werden (evtl. von anderem Programm belegt): {}", clean_str, e))?;
        registered_count += 1;
    }

    println!("[hotkey] successfully registered {} global hotkey(s)", registered_count);
    Ok(())
}

pub fn register_hotkey(app: &tauri::AppHandle, hotkey_str: &str) -> Result<(), String> {
    let cfg = crate::config::get_config(app.clone());
    let mut bindings = cfg.hotkeys.clone();
    if let Some(first) = bindings.first_mut() {
        first.hotkey = hotkey_str.to_string();
    } else {
        bindings.push(crate::config::HotkeyBinding {
            id: "slot_1".to_string(),
            hotkey: hotkey_str.to_string(),
            duration_secs: 30,
            label: "Clip".to_string(),
        });
    }
    register_all_hotkeys(app, &bindings)
}

#[tauri::command]
pub fn set_hotkey(app: tauri::AppHandle, new_hotkey: String) -> Result<(), String> {
    register_hotkey(&app, &new_hotkey)
}

#[tauri::command]
pub fn set_hotkey_bindings(app: tauri::AppHandle, bindings: Vec<crate::config::HotkeyBinding>) -> Result<(), String> {
    register_all_hotkeys(&app, &bindings)
}

#[repr(C)]
struct XINPUT_GAMEPAD {
    w_buttons: u16,
    b_left_trigger: u8,
    b_right_trigger: u8,
    s_thumb_lx: i16,
    s_thumb_ly: i16,
    s_thumb_rx: i16,
    s_thumb_ry: i16,
}

#[repr(C)]
struct XINPUT_STATE {
    dw_packet_number: u32,
    gamepad: XINPUT_GAMEPAD,
}

static CONTROLLER_ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn update_controller_enabled(enabled: bool) {
    CONTROLLER_ENABLED.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

pub fn is_controller_enabled() -> bool {
    CONTROLLER_ENABLED.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn start_controller_listener(app: tauri::AppHandle) {
    let cfg = crate::config::get_config(app.clone());
    update_controller_enabled(cfg.controller_clipping);

    std::thread::spawn(move || {
        type XInputGetStateFn = unsafe extern "system" fn(u32, *mut XINPUT_STATE) -> u32;

        use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
        use windows::core::PCSTR;

        let lib = unsafe {
            let name = b"xinput1_4.dll\0";
            let h = LoadLibraryA(PCSTR::from_raw(name.as_ptr()));
            if let Ok(handle) = h {
                handle
            } else {
                let name9 = b"xinput9_1_0.dll\0";
                match LoadLibraryA(PCSTR::from_raw(name9.as_ptr())) {
                    Ok(h9) => h9,
                    Err(_) => return,
                }
            }
        };

        let get_state: XInputGetStateFn = unsafe {
            let proc_name = b"XInputGetState\0";
            match GetProcAddress(lib, PCSTR::from_raw(proc_name.as_ptr())) {
                Some(p) => std::mem::transmute(p),
                None => return,
            }
        };

        let mut was_pressed = false;

        loop {
            if !CONTROLLER_ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(800));
                continue;
            }

            std::thread::sleep(std::time::Duration::from_millis(120));

            let mut state = XINPUT_STATE {
                dw_packet_number: 0,
                gamepad: XINPUT_GAMEPAD {
                    w_buttons: 0,
                    b_left_trigger: 0,
                    b_right_trigger: 0,
                    s_thumb_lx: 0,
                    s_thumb_ly: 0,
                    s_thumb_rx: 0,
                    s_thumb_ry: 0,
                },
            };

            let mut is_down = false;
            for user_idx in 0..4 {
                let ret = unsafe { get_state(user_idx, &mut state) };
                if ret == 0 {
                    let btns = state.gamepad.w_buttons;
                    // Combo: LB (0x0100) + RB (0x0200) + D-Pad Down (0x0002) OR Back/View (0x0020) + Start (0x0010)
                    let combo1 = (btns & 0x0100 != 0) && (btns & 0x0200 != 0) && (btns & 0x0002 != 0);
                    let combo2 = (btns & 0x0020 != 0) && (btns & 0x0010 != 0);
                    if combo1 || combo2 {
                        is_down = true;
                        break;
                    }
                }
            }

            if is_down && !was_pressed {
                was_pressed = true;
                let app_c = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = crate::recorder::save_clip_now(app_c).await;
                });
            } else if !is_down {
                was_pressed = false;
            }
        }
    });
}
