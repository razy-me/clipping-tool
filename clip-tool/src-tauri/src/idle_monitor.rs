use tauri::AppHandle;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::System::SystemInformation::GetTickCount;

static WAS_AUTO_PAUSED: AtomicBool = AtomicBool::new(false);
static IDLE_MONITOR_ENABLED: AtomicBool = AtomicBool::new(false);
static IDLE_THRESHOLD_SECS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(300);

pub fn update_idle_config(enabled: bool, minutes: u32) {
    IDLE_MONITOR_ENABLED.store(enabled, Ordering::Relaxed);
    IDLE_THRESHOLD_SECS.store((minutes.max(1) as u64) * 60, Ordering::Relaxed);
}

pub fn clear_auto_pause() {
    WAS_AUTO_PAUSED.store(false, Ordering::SeqCst);
}

pub fn get_idle_time_secs() -> u64 {
    unsafe {
        let mut lii = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if GetLastInputInfo(&mut lii).as_bool() {
            let tick = GetTickCount();
            return (tick.wrapping_sub(lii.dwTime) / 1000) as u64;
        }
        0
    }
}

pub fn start_idle_monitor(app: AppHandle) {
    let cfg = crate::config::get_config(app.clone());
    update_idle_config(cfg.auto_pause_idle, cfg.auto_pause_idle_minutes);

    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

            if !IDLE_MONITOR_ENABLED.load(Ordering::Relaxed) {
                WAS_AUTO_PAUSED.store(false, Ordering::Relaxed);
                continue;
            }

            let idle_secs = get_idle_time_secs();
            let threshold_secs = IDLE_THRESHOLD_SECS.load(Ordering::Relaxed);

            let state_str = crate::recorder::get_buffer_state();
            let is_active = state_str == "active";
            let is_paused = state_str == "paused";

            if is_active && idle_secs >= threshold_secs {
                println!("[idle_monitor] PC inaktiv seit {}s (Schwelle: {}s) -> Pausiere Puffer", idle_secs, threshold_secs);
                WAS_AUTO_PAUSED.store(true, Ordering::SeqCst);
                let _ = crate::recorder::pause_buffer(app.clone());
            } else if is_paused && WAS_AUTO_PAUSED.load(Ordering::SeqCst) && idle_secs < 3 {
                println!("[idle_monitor] Benutzeraktivität erkannt -> Reaktiviere Puffer");
                WAS_AUTO_PAUSED.store(false, Ordering::SeqCst);
                let app_c = app.clone();
                let _ = crate::recorder::resume_buffer(app_c).await;
            }
        }
    });
}
