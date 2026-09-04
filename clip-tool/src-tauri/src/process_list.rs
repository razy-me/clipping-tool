use sysinfo::{System, RefreshKind, ProcessRefreshKind, ProcessesToUpdate};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub exe: String,
}

/// Returns true if this process is a background/system service that the user
/// would never want to mute or audio-separate.
pub fn is_background_process(name: &str, exe: &str) -> bool {
    let name_owned = name.to_lowercase();
    let exe_owned = exe.to_lowercase();
    let name_lower = name_owned.as_str();
    let exe_lower = exe_owned.as_str();

    // ── Windows-eigene Systempfade ─────────────────────────────────────────
    if exe_lower.starts_with("c:\\windows\\system32\\")
        || exe_lower.starts_with("c:\\windows\\syswow64\\")
        || exe_lower.starts_with("c:\\windows\\winsxs\\")
        || exe_lower.starts_with("c:\\windows\\systemapps\\")
    {
        return true;
    }

    // ── Bekannte Windows-Kern-Prozesse (nach exaktem Namen) ───────────────
    const SYSTEM_EXACT: &[&str] = &[
        "svchost.exe",
        "conhost.exe",
        "runtimebroker.exe",
        "searchindexer.exe",
        "searchhost.exe",
        "backgroundtaskhost.exe",
        "dwm.exe",
        "winlogon.exe",
        "csrss.exe",
        "lsass.exe",
        "smss.exe",
        "wininit.exe",
        "spoolsv.exe",
        "taskhostw.exe",
        "taskeng.exe",
        "services.exe",
        "registry",
        "system",
        "idle",
        "fontdrvhost.exe",
        "sihost.exe",
        "ctfmon.exe",
        "dllhost.exe",
        "msdtc.exe",
        "lsm.exe",
        "wermgr.exe",
        "werfault.exe",
        "vssvc.exe",
        "audiodg.exe",         // Audio-Graph (kein App-Sound)
        "wmiprvse.exe",
        "wbengine.exe",
        "securityhealthservice.exe",
        "securityhealthsystray.exe",
        "wsappx",
        "mssense.exe",
        "sensorservice.exe",
        "unsecapp.exe",
        "ipfsvc.exe",
    ];
    if SYSTEM_EXACT.contains(&name_lower) {
        return true;
    }

    // ── Namenspräfixe typischer Hintergrunddienste ────────────────────────
    const BG_PREFIXES: &[&str] = &[
        "microsoft.",          // Microsoft Edge Update, Microsoft Office Click-to-Run, etc.
        "windows.",            // Windows Update, Windows Defender
        "msedgeupdate",        // Edge background updater
        "msedgewebview2",      // Edge WebView helper
    ];
    for prefix in BG_PREFIXES {
        if name_lower.starts_with(prefix) {
            return true;
        }
    }

    // ── Namentliche Schlüsselwörter für typische Hintergrundhelfer ────────
    // (Nur wenn der Name *nur* aus diesen Begriffen besteht, nicht wenn es
    //  z.B. „SpotifyHelper" betrifft → das wollen wir noch zeigen.)
    const BG_KEYWORDS: &[&str] = &[
        "crashhandler",
        "uninstall",
        "installer",
    ];
    // (Array intentionally left short – nur absolute Hintergrund-Keywords.)
    for kw in BG_KEYWORDS {
        if name_lower.contains(kw) {
            return true;
        }
    }

    // ── Keine GUI / kein Fenster → wahrscheinlich Dienst ─────────────────
    // Prozesse ohne .exe-Endung sind in der Regel Kernel-Prozesse
    if !name_lower.ends_with(".exe") {
        return true;
    }

    false
}

#[tauri::command]
pub fn get_running_applications() -> Vec<ProcessInfo> {
    let mut sys = System::new_with_specifics(
        RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing().with_exe(sysinfo::UpdateKind::OnlyIfNotSet)),
    );
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let mut apps = Vec::new();
    let mut seen_names = std::collections::HashSet::new();

    for (pid, process) in sys.processes() {
        let name = process.name().to_string_lossy().to_string();
        if name.is_empty() {
            continue;
        }

        let exe = process
            .exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let name_lower = name.to_lowercase();
        let exe_lower = exe.to_lowercase();

        if is_background_process(&name_lower, &exe_lower) {
            continue;
        }

        // Doppelte Namen überspringen (z.B. mehrere Chrome-Helper-Prozesse)
        if !seen_names.contains(&name) {
            seen_names.insert(name.clone());
            apps.push(ProcessInfo {
                pid: pid.as_u32(),
                name,
                exe,
            });
        }
    }

    // Alphabetisch sortieren
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

// ══════════════════════════════════════════════════════════════════════════
// Visible-application discovery for automatic per-app audio splitting.
//
// A "user app" = any process owning a visible top-level window with a title,
// excluding our own process, Windows shell infrastructure and embedded webview
// hosts. Child processes (e.g. Chrome renderers) roll up into their
// same-executable root ancestor so one app == one audio track.
// ══════════════════════════════════════════════════════════════════════════

use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowThreadProcessId,
    IsWindowVisible, GWL_EXSTYLE, WS_EX_TOOLWINDOW,
};

#[derive(Clone, Debug)]
pub struct VisibleApp {
    /// Root PID to target for WASAPI process(-tree) loopback.
    pub pid: u32,
    /// Track label derived from the executable file name.
    pub name: String,
}

const SHELL_DENYLIST: &[&str] = &[
    "applicationframehost", "textinputhost", "searchhost",
    "shellexperiencehost", "startmenuexperiencehost", "runtimebroker",
    "widgets", "widgetservice", "msedgewebview2",
    "systemsettings", "lockapp", "explorer", "clip-tool",
];

fn is_windows_dir(exe_lower: &str) -> bool {
    exe_lower.starts_with("c:\\windows\\system32\\")
        || exe_lower.starts_with("c:\\windows\\syswow64\\")
        || exe_lower.starts_with("c:\\windows\\winsxs\\")
}

extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let list = &mut *(lparam.0 as *mut Vec<u32>);
        if !IsWindowVisible(hwnd).as_bool() {
            return BOOL(1);
        }
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
            return BOOL(1);
        }
        if GetWindowTextLengthW(hwnd) == 0 {
            return BOOL(1);
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid != 0 {
            list.push(pid);
        }
    }
    BOOL(1)
}

fn collect_window_pids() -> Vec<u32> {
    let mut pids: Vec<u32> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut pids as *mut Vec<u32> as isize));
    }
    pids
}

/// Shared, throttled full-process snapshot (one refresh serves every scanner).
pub(crate) fn with_process_snapshot<T>(f: impl FnOnce(&System) -> T) -> T {
    static SHARED: std::sync::Mutex<Option<(System, std::time::Instant)>> = std::sync::Mutex::new(None);
    let mut guard = SHARED.lock().unwrap();
    let needs_refresh = match guard.as_ref() {
        Some((_, at)) => at.elapsed() > std::time::Duration::from_millis(3500),
        None => true,
    };
    if needs_refresh {
        let mut sys = System::new_with_specifics(
            RefreshKind::nothing()
                .with_processes(ProcessRefreshKind::nothing().with_exe(sysinfo::UpdateKind::OnlyIfNotSet)),
        );
        sys.refresh_processes(ProcessesToUpdate::All, true);
        *guard = Some((sys, std::time::Instant::now()));
    }
    f(&guard.as_ref().unwrap().0)
}

pub fn visible_user_apps() -> Vec<VisibleApp> {
    let window_pids = collect_window_pids();
    let me = std::process::id();

    let mut out: Vec<VisibleApp> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    with_process_snapshot(|sys| {
        for wpid in window_pids {
            if wpid == me { continue; }
            let Some(proc_entry) = sys.process(sysinfo::Pid::from_u32(wpid)) else { continue };

            let exe_path = proc_entry.exe()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            if exe_path.is_empty() { continue; }

            let exe_lower = exe_path.to_lowercase();
            if is_windows_dir(&exe_lower) { continue; }

            let fname = std::path::Path::new(&exe_lower)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default();
            if fname.is_empty() { continue; }
            if SHELL_DENYLIST.iter().any(|d| fname == *d || fname == format!("{d}.exe")) {
                continue;
            }

            // Roll children up into the root ancestor sharing this exe path
            // (Chrome/Firefox/Discord spawn many same-named helpers).
            let mut cur = proc_entry;
            let mut root_pid = wpid;
            let mut hops = 0;
            loop {
                hops += 1;
                if hops > 16 { break; }
                let Some(parent_pid) = cur.parent() else { break };
                let Some(parent) = sys.process(parent_pid) else { break };
                let parent_exe = parent.exe()
                    .map(|p| p.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                if parent_exe == exe_lower {
                    root_pid = parent_pid.as_u32();
                    cur = parent;
                } else {
                    break;
                }
            }

            let track_name = fname.trim_end_matches(".exe").to_string();
            if seen.insert(track_name.clone()) {
                out.push(VisibleApp { pid: root_pid, name: track_name });
            }
        }
    });

    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}
