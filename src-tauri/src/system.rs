//! System / hardware info for the in-app "Mi equipo" panel and the metrics GPU
//! picker. CPU/RAM/OS/disks come from `sysinfo`; the motherboard from the BIOS
//! registry key; displays from Win32 GDI; and the GPU list (with metric-capable
//! keys for the picker) from NVML (NVIDIA) and ADLX (AMD).

use serde::Serialize;
use sysinfo::{Disks, System};

/// A GPU as shown in the panel. `key` ("nvml:<i>" / "adlx:<i>") is set only for
/// metric-capable GPUs — those can be picked for the overlay; empty otherwise.
#[derive(Serialize)]
pub struct GpuInfo {
    pub name: String,
    pub vendor: String,
    pub vram_mb: u64,
    pub kind: String,
    pub key: String,
}

#[derive(Serialize)]
pub struct DiskInfo {
    pub name: String,
    pub fs: String,
    pub total_mb: u64,
    pub available_mb: u64,
}

#[derive(Serialize)]
pub struct DisplayInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub refresh_hz: u32,
    pub primary: bool,
}

#[derive(Serialize)]
pub struct SystemInfo {
    pub cpu: String,
    pub cpu_cores: usize,
    pub cpu_threads: usize,
    pub ram_total_mb: u64,
    pub os: String,
    pub motherboard: Option<String>,
    pub gpus: Vec<GpuInfo>,
    pub disks: Vec<DiskInfo>,
    pub displays: Vec<DisplayInfo>,
}

const MB: u64 = 1024 * 1024;

/// Gather everything for the panel. Best-effort: any source that fails is just
/// omitted (empty list / `None`), never an error.
pub fn collect() -> SystemInfo {
    let mut sys = System::new();
    sys.refresh_cpu();
    sys.refresh_memory();

    let cpu = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Desconocido".to_string());

    let os = {
        let name = System::name().unwrap_or_default();
        let ver = System::os_version().unwrap_or_default();
        let joined = format!("{name} {ver}").trim().to_string();
        if joined.is_empty() {
            "Desconocido".to_string()
        } else {
            joined
        }
    };

    let disks = Disks::new_with_refreshed_list()
        .list()
        .iter()
        .map(|d| DiskInfo {
            name: d.name().to_string_lossy().to_string(),
            fs: d.file_system().to_string_lossy().to_string(),
            total_mb: d.total_space() / MB,
            available_mb: d.available_space() / MB,
        })
        .collect();

    SystemInfo {
        cpu,
        cpu_cores: sys.physical_core_count().unwrap_or(0),
        cpu_threads: sys.cpus().len(),
        ram_total_mb: sys.total_memory() / MB,
        os,
        motherboard: motherboard(),
        gpus: gpus(),
        disks,
        displays: displays(),
    }
}

/// Metric-capable GPUs (NVML + ADLX), each tagged with its picker key.
fn gpus() -> Vec<GpuInfo> {
    let mut out = Vec::new();

    // NVIDIA via NVML.
    if let Ok(nvml) = nvml_wrapper::Nvml::init() {
        if let Ok(count) = nvml.device_count() {
            for i in 0..count {
                if let Ok(dev) = nvml.device_by_index(i) {
                    let name = dev.name().unwrap_or_else(|_| "NVIDIA GPU".to_string());
                    let vram_mb = dev.memory_info().map(|m| m.total / MB).unwrap_or(0);
                    out.push(GpuInfo {
                        name,
                        vendor: "NVIDIA".to_string(),
                        vram_mb,
                        kind: "Discreta".to_string(),
                        key: format!("nvml:{i}"),
                    });
                }
            }
        }
    }

    // AMD via ADLX.
    #[cfg(windows)]
    {
        if crate::amd::init() {
            for (i, g) in crate::amd::list_gpus().into_iter().enumerate() {
                out.push(GpuInfo {
                    name: g.name,
                    vendor: "AMD".to_string(),
                    vram_mb: g.vram_mb,
                    kind: g.kind,
                    key: format!("adlx:{i}"),
                });
            }
        }
    }

    out
}

/// Motherboard manufacturer + product from the BIOS registry key (no WMI).
#[cfg(windows)]
fn motherboard() -> Option<String> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;
    let bios = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"HARDWARE\DESCRIPTION\System\BIOS")
        .ok()?;
    let vendor: String = bios.get_value("BaseBoardManufacturer").unwrap_or_default();
    let product: String = bios.get_value("BaseBoardProduct").unwrap_or_default();
    let joined = format!("{} {}", vendor.trim(), product.trim());
    let joined = joined.trim().to_string();
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

#[cfg(not(windows))]
fn motherboard() -> Option<String> {
    None
}

/// Active displays (resolution + refresh) via Win32 GDI.
#[cfg(windows)]
fn displays() -> Vec<DisplayInfo> {
    use windows::core::PCWSTR;
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayDevicesW, EnumDisplaySettingsW, DEVMODEW, DISPLAY_DEVICEW,
        DISPLAY_DEVICE_ATTACHED_TO_DESKTOP, DISPLAY_DEVICE_PRIMARY_DEVICE, ENUM_CURRENT_SETTINGS,
    };

    fn wide_to_string(w: &[u16]) -> String {
        let end = w.iter().position(|&c| c == 0).unwrap_or(w.len());
        String::from_utf16_lossy(&w[..end])
    }

    let mut out = Vec::new();
    let mut i = 0u32;
    loop {
        let mut dev = DISPLAY_DEVICEW {
            cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32,
            ..Default::default()
        };
        let ok = unsafe { EnumDisplayDevicesW(PCWSTR::null(), i, &mut dev, 0) };
        if !ok.as_bool() {
            break;
        }
        i += 1;

        let attached = (dev.StateFlags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP).0 != 0;
        if !attached {
            continue;
        }
        let primary = (dev.StateFlags & DISPLAY_DEVICE_PRIMARY_DEVICE).0 != 0;

        let mut mode = DEVMODEW {
            dmSize: std::mem::size_of::<DEVMODEW>() as u16,
            ..Default::default()
        };
        let got = unsafe {
            EnumDisplaySettingsW(
                PCWSTR::from_raw(dev.DeviceName.as_ptr()),
                ENUM_CURRENT_SETTINGS,
                &mut mode,
            )
        };
        if !got.as_bool() {
            continue;
        }
        let mut name = wide_to_string(&dev.DeviceString);
        let mut mon = DISPLAY_DEVICEW {
            cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32,
            ..Default::default()
        };
        let ok_mon = unsafe { EnumDisplayDevicesW(PCWSTR::from_raw(dev.DeviceName.as_ptr()), 0, &mut mon, 0) };
        if ok_mon.as_bool() {
            let mon_name = wide_to_string(&mon.DeviceString);
            if !mon_name.is_empty() {
                name = mon_name;
            }
        }

        out.push(DisplayInfo {
            name,
            width: mode.dmPelsWidth,
            height: mode.dmPelsHeight,
            refresh_hz: mode.dmDisplayFrequency,
            primary,
        });
    }
    out
}

#[cfg(not(windows))]
fn displays() -> Vec<DisplayInfo> {
    Vec::new()
}

/// The Windows user's display/full name (e.g. "Diego Chicoma") via GetUserNameExW
/// with `NameDisplay`. Returns `None` if it's empty or the call fails (common on
/// plain local accounts with no full name set), so the caller falls back to the
/// login name.
#[cfg(windows)]
pub fn display_name() -> Option<String> {
    use windows::Win32::Security::Authentication::Identity::{GetUserNameExW, NameDisplay};

    // First call with a zero size fails and reports the buffer length needed.
    let mut size: u32 = 0;
    unsafe { GetUserNameExW(NameDisplay, None, &mut size) };
    if size == 0 {
        return None;
    }
    let mut buf = vec![0u16; size as usize];
    let ok = unsafe {
        GetUserNameExW(
            NameDisplay,
            Some(windows::core::PWSTR(buf.as_mut_ptr())),
            &mut size,
        )
    };
    if !ok {
        return None;
    }
    let name = String::from_utf16_lossy(&buf[..size as usize]);
    let name = name.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}
