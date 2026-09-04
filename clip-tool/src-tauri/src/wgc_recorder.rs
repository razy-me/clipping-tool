use windows::Win32::Graphics::Gdi::{
    GetDC, ReleaseDC, GetDeviceCaps, DESKTOPHORZRES, DESKTOPVERTRES,
};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MonitorInfo {
    pub index: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
}

/// Physical pixel size of the primary monitor.
pub fn get_primary_monitor_size() -> (u32, u32) {
    unsafe {
        let hdc = GetDC(None);
        if !hdc.is_invalid() {
            let width = GetDeviceCaps(Some(hdc), DESKTOPHORZRES) as u32;
            let height = GetDeviceCaps(Some(hdc), DESKTOPVERTRES) as u32;
            let _ = ReleaseDC(None, hdc);
            if width > 0 && height > 0 {
                return (width, height);
            }
        }
    }
    (1920, 1080)
}

#[tauri::command]
pub fn get_available_monitors() -> Vec<MonitorInfo> {
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
    };
    use windows::Win32::Foundation::{LPARAM, RECT};
    use windows::core::BOOL;

    let mut monitors: Vec<MonitorInfo> = Vec::new();

    unsafe extern "system" fn enum_proc(
        hmonitor: HMONITOR,
        _: HDC,
        _: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let list = &mut *(lparam.0 as *mut Vec<MonitorInfo>);
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

        if GetMonitorInfoW(hmonitor, &mut info as *mut _ as *mut _).as_bool() {
            let width = (info.monitorInfo.rcMonitor.right - info.monitorInfo.rcMonitor.left).abs() as u32;
            let height = (info.monitorInfo.rcMonitor.bottom - info.monitorInfo.rcMonitor.top).abs() as u32;
            let is_primary = (info.monitorInfo.dwFlags & 1) != 0; // MONITORINFOF_PRIMARY = 1
            let idx = list.len() as u32;

            let label = if is_primary {
                format!("Monitor {} ({width}×{height}) - Hauptbildschirm", idx + 1)
            } else {
                format!("Monitor {} ({width}×{height})", idx + 1)
            };

            list.push(MonitorInfo {
                index: idx,
                name: label,
                width,
                height,
                is_primary,
            });
        }
        BOOL(1)
    }

    unsafe {
        let ptr = &mut monitors as *mut _ as isize;
        let _ = EnumDisplayMonitors(None, None, Some(enum_proc), LPARAM(ptr));
    }

    if monitors.is_empty() {
        monitors.push(MonitorInfo {
            index: 0,
            name: "Monitor 1 (1920×1080) - Hauptbildschirm".into(),
            width: 1920,
            height: 1080,
            is_primary: true,
        });
    }

    monitors
}
