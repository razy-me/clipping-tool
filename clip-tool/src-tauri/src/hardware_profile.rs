use std::sync::Mutex;
use sysinfo::{System, RefreshKind, MemoryRefreshKind};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::SystemInformation::{GetLogicalProcessorInformationEx, SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX, RelationProcessorCore};
use windows::Win32::System::Threading::{
    SetProcessInformation, ProcessPowerThrottling, PROCESS_POWER_THROTTLING_STATE,
    PROCESS_POWER_THROTTLING_EXECUTION_SPEED, GetCurrentProcess
};
use core::ffi::c_void;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardwareTier {
    Weak,
    Mid,
    Strong,
}

#[derive(Clone, Debug)]
pub struct HardwareProfile {
    pub tier: HardwareTier,
    pub logical_cores: u32,
    pub has_hw_encoder: bool,
    pub e_cores_mask: Option<usize>,
}

static PROFILE_CACHE: Mutex<Option<HardwareProfile>> = Mutex::new(None);

pub fn get_free_ram_bytes() -> u64 {
    let mut sys = System::new_with_specifics(RefreshKind::nothing().with_memory(MemoryRefreshKind::everything()));
    sys.refresh_memory();
    sys.available_memory()
}

pub fn is_on_battery() -> bool {
    unsafe {
        let mut status = SYSTEM_POWER_STATUS::default();
        if GetSystemPowerStatus(&mut status).is_ok() {
            return status.ACLineStatus == 0;
        }
    }
    false
}

pub fn apply_ecoqos() {
    unsafe {
        let mut state = PROCESS_POWER_THROTTLING_STATE {
            Version: 1, // PROCESS_POWER_THROTTLING_CURRENT_VERSION
            ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
            StateMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        };
        let _ = SetProcessInformation(
            GetCurrentProcess(),
            ProcessPowerThrottling,
            &mut state as *mut _ as *mut c_void,
            std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
        );
    }
}

fn get_e_cores_mask() -> Option<usize> {
    unsafe {
        let mut len = 0;
        let _ = GetLogicalProcessorInformationEx(RelationProcessorCore, None, &mut len);
        if len == 0 { return None; }
        let mut buffer: Vec<u8> = vec![0; len as usize];
        if GetLogicalProcessorInformationEx(
            RelationProcessorCore,
            Some(buffer.as_mut_ptr() as *mut SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX),
            &mut len,
        ).is_err() {
            return None;
        }

        // Pass 1: Collect all distinct efficiency classes across all cores
        let mut offset = 0;
        let mut min_class = u8::MAX;
        let mut max_class = u8::MIN;
        let mut core_entries = Vec::new();

        while offset < len as usize {
            let info = &*(buffer.as_ptr().add(offset) as *const SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX);
            if info.Relationship == RelationProcessorCore {
                let core_info = info.Anonymous.Processor;
                let class = core_info.EfficiencyClass;
                if class < min_class { min_class = class; }
                if class > max_class { max_class = class; }
                let group_mask = if core_info.GroupCount > 0 { core_info.GroupMask[0].Mask } else { 0 };
                core_entries.push((class, group_mask));
            }
            offset += info.Size as usize;
        }

        // Only hybrid CPUs have multiple different efficiency classes (e.g. min_class = E-cores, max_class = P-cores)
        if min_class < max_class {
            let mut mask = 0;
            for (class, group_mask) in core_entries {
                if class == min_class {
                    mask |= group_mask;
                }
            }
            if mask != 0 {
                return Some(mask);
            }
        }
        None
    }
}

pub async fn detect_hardware_profile(app: &tauri::AppHandle) -> HardwareProfile {
    {
        let guard = PROFILE_CACHE.lock().unwrap();
        if let Some(prof) = guard.as_ref() {
            return prof.clone();
        }
    }

    let mut sys = System::new_with_specifics(RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram()));
    sys.refresh_memory();
    let total_ram_mb = sys.total_memory() / 1024 / 1024;
    
    let logical_cores = std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(4);

    let mut has_hw_encoder = false;
    let cfg = crate::config::get_config(app.clone());
    let encoder = crate::hardware::detect_best_encoder_for_codec(app, &cfg.video_codec).await;
    if !encoder.starts_with("libx") && !encoder.starts_with("libsvt") {
        has_hw_encoder = true;
    }

    let tier = if !has_hw_encoder && (logical_cores <= 4 || total_ram_mb <= 8192) {
        HardwareTier::Weak
    } else if has_hw_encoder && logical_cores >= 6 && total_ram_mb >= 15000 {
        HardwareTier::Strong
    } else {
        HardwareTier::Mid
    };

    let prof = HardwareProfile {
        tier,
        logical_cores,
        has_hw_encoder,
        e_cores_mask: get_e_cores_mask(),
    };

    *PROFILE_CACHE.lock().unwrap() = Some(prof.clone());
    println!("[hardware_profile] Detected Tier: {:?} (Cores: {}, RAM: {} MB, HW Enc: {})", tier, logical_cores, total_ram_mb, has_hw_encoder);
    
    prof
}

pub fn get_cached_tier() -> HardwareTier {
    PROFILE_CACHE.lock().unwrap().as_ref().map(|p| p.tier).unwrap_or(HardwareTier::Mid)
}
