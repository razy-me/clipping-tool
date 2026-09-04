use rodio::{OutputStreamBuilder, Sink};
use windows::core::PWSTR;
use windows::Win32::Foundation::{HWND, MAX_PATH};
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_VM_READ, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

use rodio::source::{SineWave, Source};

pub fn play_notification_sound() {
    std::thread::spawn(|| {
        if let Ok(stream_handle) = OutputStreamBuilder::open_default_stream() {
            let sink = Sink::connect_new(&stream_handle.mixer());
            
            // Warm, relaxed 2-note sound (C4 -> G4)
            let note1 = SineWave::new(261.63).take_duration(std::time::Duration::from_millis(80)).amplify(0.12);
            let note2 = SineWave::new(392.00).take_duration(std::time::Duration::from_millis(180)).amplify(0.15);

            sink.append(note1);
            sink.append(note2);

            sink.sleep_until_end();
        }
    });
}

pub fn get_active_game_name() -> String {
    unsafe {
        let hwnd: HWND = GetForegroundWindow();
        if hwnd.0 == std::ptr::null_mut() {
            return "Desktop".to_string();
        }

        let mut process_id: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));

        if process_id == 0 {
            return "Desktop".to_string();
        }

        // Elevated games deny full query access; LIMITED suffices for the
        // image name and is what the old code effectively needed all along.
        let path = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id)
            .ok()
            .and_then(|handle| {
                let mut buffer = [0u16; 1024];
                let mut size = buffer.len() as u32;
                let res = QueryFullProcessImageNameW(
                    handle,
                    PROCESS_NAME_WIN32,
                    PWSTR(buffer.as_mut_ptr()),
                    &mut size,
                )
                .ok()
                .map(|_| String::from_utf16_lossy(&buffer[..size as usize]));
                let _ = windows::Win32::Foundation::CloseHandle(handle);
                res
            })
            .or_else(|| {
                // Legacy fallback
                OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, process_id)
                    .ok()
                    .and_then(|handle| {
                        let mut buffer = [0u16; MAX_PATH as usize];
                        let len = GetModuleFileNameExW(Some(handle), None, &mut buffer);
                        let res = if len > 0 {
                            Some(String::from_utf16_lossy(&buffer[..len as usize]))
                        } else {
                            None
                        };
                        let _ = windows::Win32::Foundation::CloseHandle(handle);
                        res
                    })
            });

        if let Some(path) = path {
            if let Some(filename) = std::path::Path::new(&path).file_name() {
                let name = filename.to_string_lossy().to_string();
                return name.strip_suffix(".exe")
                    .or_else(|| name.strip_suffix(".EXE"))
                    .unwrap_or(&name)
                    .to_string();
            }
        }
    }
    "Desktop".to_string()
}

