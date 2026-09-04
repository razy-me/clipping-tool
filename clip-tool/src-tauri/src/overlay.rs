use tauri::{AppHandle, Manager, Emitter};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE,
    WS_EX_NOACTIVATE, WS_EX_TRANSPARENT, WS_EX_TOPMOST, WS_EX_TOOLWINDOW,
    ShowWindow, SW_SHOWNOACTIVATE, SetWindowPos, HWND_TOPMOST,
    SWP_NOACTIVATE, SWP_SHOWWINDOW, SWP_NOSIZE, SWP_NOMOVE,
};
use windows::Win32::Foundation::HWND;

pub fn setup_overlay_window(window: &tauri::WebviewWindow) {
    if let Ok(hwnd_ptr) = window.hwnd() {
        unsafe {
            let hwnd = HWND(hwnd_ptr.0);
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let new_style = ex_style
                | (WS_EX_NOACTIVATE.0 as isize)
                | (WS_EX_TRANSPARENT.0 as isize)
                | (WS_EX_TOPMOST.0 as isize)
                | (WS_EX_TOOLWINDOW.0 as isize);
            let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
        }
    }
}

pub fn create_overlay_window_if_needed(app: &AppHandle) {
    if app.get_webview_window("overlay").is_some() {
        return;
    }

    let (screen_w, _) = crate::wgc_recorder::get_primary_monitor_size();
    let width = 360.0;
    let height = 110.0;
    let pos_x = (screen_w as f64 - width - 24.0).max(10.0);
    let pos_y = 24.0;

    if let Ok(win) = tauri::WebviewWindowBuilder::new(
        app,
        "overlay",
        tauri::WebviewUrl::App("overlay.html".into()),
    )
    .title("ClipTool Overlay")
    .inner_size(width, height)
    .position(pos_x, pos_y)
    .transparent(true)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .shadow(false)
    .resizable(false)
    .visible(false)
    .focused(false)
    .build()
    {
        setup_overlay_window(&win);
    }
}

static OVERLAY_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn show_clip_overlay(app: &AppHandle, game: &str, duration_secs: u32) {
    let cfg = crate::config::get_config(app.clone());
    if !cfg.overlay_notification {
        return;
    }

    create_overlay_window_if_needed(app);

    if let Some(win) = app.get_webview_window("overlay") {
        setup_overlay_window(&win);
        let _ = win.emit("overlay://show", serde_json::json!({
            "game": game,
            "duration": duration_secs,
        }));

        if let Ok(hwnd_ptr) = win.hwnd() {
            unsafe {
                let hwnd = HWND(hwnd_ptr.0);
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    0, 0, 0, 0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
        }

        let seq = OVERLAY_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        let win_clone = win.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(3500)).await;
            if OVERLAY_SEQ.load(std::sync::atomic::Ordering::SeqCst) == seq {
                let _ = win_clone.hide();
            }
        });
    }
}

pub fn show_overlay_error(app: &AppHandle, message: &str) {
    let cfg = crate::config::get_config(app.clone());
    if !cfg.overlay_notification {
        return;
    }

    create_overlay_window_if_needed(app);

    if let Some(win) = app.get_webview_window("overlay") {
        setup_overlay_window(&win);
        let _ = win.emit("overlay://error", serde_json::json!({
            "message": message,
        }));

        if let Ok(hwnd_ptr) = win.hwnd() {
            unsafe {
                let hwnd = HWND(hwnd_ptr.0);
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    0, 0, 0, 0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
        }

        let seq = OVERLAY_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        let win_clone = win.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(4000)).await;
            if OVERLAY_SEQ.load(std::sync::atomic::Ordering::SeqCst) == seq {
                let _ = win_clone.hide();
            }
        });
    }
}
