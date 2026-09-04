pub mod config;
pub mod hardware;
pub mod hardware_profile;
pub mod recorder;
pub mod audio;
pub mod library;
pub mod editor;
pub mod hotkeys;
pub mod audio_engine;
pub mod wgc_recorder;
pub mod process_list;
pub mod overlay;
pub mod idle_monitor;
pub mod performance;

use tauri::Manager;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;

#[tauri::command]
fn enable_autostart(app: tauri::AppHandle) {
    use tauri_plugin_autostart::ManagerExt;
    let _ = app.autolaunch().enable();
}

#[tauri::command]
fn disable_autostart(app: tauri::AppHandle) {
    use tauri_plugin_autostart::ManagerExt;
    let _ = app.autolaunch().disable();
}

#[tauri::command]
fn app_minimize(window: tauri::WebviewWindow) {
    let _ = window.minimize();
}

#[tauri::command]
fn app_toggle_maximize(window: tauri::WebviewWindow) {
    if let Ok(is_max) = window.is_maximized() {
        if is_max {
            let _ = window.unmaximize();
        } else {
            let _ = window.maximize();
        }
    }
}

#[tauri::command]
fn app_close(window: tauri::WebviewWindow) {
    let _ = window.close();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_shell::init())
    .plugin(tauri_plugin_global_shortcut::Builder::new().build())
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_autostart::init(
        tauri_plugin_autostart::MacosLauncher::LaunchAgent,
        Some(vec!["--minimized"])
    ))
    .manage(recorder::RecorderState {
        child: std::sync::Mutex::new(None),
    })
    .manage(performance::PerformanceMonitorState::new())
    .invoke_handler(tauri::generate_handler![
        config::get_config,
        config::save_config,
        hardware::get_encoder,
        hardware::get_scaling_method,
        hardware::get_available_encoders,
        recorder::start_buffer,
        recorder::pause_buffer,
        recorder::resume_buffer,
        recorder::stop_buffer,
        recorder::get_buffer_state,
        recorder::get_last_error_log,
        recorder::save_clip_now,
        enable_autostart,
        disable_autostart,
        audio_engine::get_mic_level,
        audio_engine::set_mic_volume,
        audio_engine::get_active_split_apps,
        audio_engine::get_active_mic_device,
        library::get_all_clips,
        library::get_disk_space_info,
        library::delete_clip,
        library::toggle_favorite,
        library::copy_clip_to_clipboard,
        library::trigger_auto_cleanup,
        editor::open_editor_window,
        hotkeys::set_hotkey,
        hotkeys::set_hotkey_bindings,
        process_list::get_running_applications,
        wgc_recorder::get_available_monitors,
        performance::get_tool_performance,
        performance::save_perf_snapshot,
        app_minimize,
        app_toggle_maximize,
        app_close
    ])
    .setup(|app| {
      unsafe {
          let _ = windows::Win32::Media::timeBeginPeriod(1);
      }
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }

      // Initialize transparent in-game overlay notification window
      overlay::create_overlay_window_if_needed(app.handle());

      // Start background Gamepad/Controller listener
      hotkeys::start_controller_listener(app.handle().clone());

      // Start PC idle watchdog for auto-pausing buffer
      idle_monitor::start_idle_monitor(app.handle().clone());

      // Load config and register all hotkey slots
      let cfg = config::get_config(app.handle().clone());
      if let Err(e) = hotkeys::register_all_hotkeys(app.handle(), &cfg.hotkeys) {
        log::warn!("[setup] hotkeys failed to register: {e}");
      }

      // Startup and periodic auto-cleanup task
      let app_clean = app.handle().clone();
      tauri::async_runtime::spawn(async move {
          tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
          let _ = library::run_auto_cleanup(&app_clean);
          loop {
              tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
              let _ = library::run_auto_cleanup(&app_clean);
          }
      });

      // System tray
      let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
      let show_i = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
      let hide_i = MenuItem::with_id(app, "hide", "Hide", true, None::<&str>)?;
      let menu = Menu::with_items(app, &[&show_i, &hide_i, &quit_i])?;

      if let Some(icon) = app.default_window_icon().cloned() {
          TrayIconBuilder::new()
              .menu(&menu)
              .show_menu_on_left_click(true)
              .icon(icon)
              .on_menu_event(|app, event| match event.id.as_ref() {
                  "quit" => {
                      // Don't corrupt an in-flight save: wait briefly.
                      for _ in 0..40 {
                          if !recorder::is_saving() { break; }
                          std::thread::sleep(std::time::Duration::from_millis(250));
                      }
                      let state = app.state::<recorder::RecorderState>();
                      if let Some(child_arc) = state.child.lock().unwrap().take() {
                          if let Ok(mut opt) = child_arc.lock() {
                              if let Some(child) = opt.take() {
                                  let _ = child.kill();
                              }
                          }
                      }
                      unsafe {
                          let _ = windows::Win32::Media::timeEndPeriod(1);
                      }
                      std::process::exit(0);
                  }
                  "show" => {
                      if let Some(window) = app.get_webview_window("main") {
                          let _ = window.show();
                          let _ = window.set_focus();
                      }
                  }
                  "hide" => {
                      if let Some(window) = app.get_webview_window("main") {
                          let _ = window.hide();
                      }
                  }
                  _ => {}
              })
              .build(app)?;
      }

      // Auto-start recording buffer on app launch
      let handle_buf = app.handle().clone();
      tauri::async_runtime::spawn(async move {
          // Short delay to let window/events initialize cleanly
          tokio::time::sleep(std::time::Duration::from_millis(300)).await;
          let _ = recorder::start_buffer(handle_buf).await;
      });

      Ok(())
    })
    .on_window_event(|window, event| match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            // Main window hides to tray; other windows (editor) close normally.
            if window.label() == "main" {
                api.prevent_close();
                let _ = window.hide();
            }
        }
        _ => {}
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
