//! Cheap global CPU% and RAM via direct Win32, for the per-second metrics sampler.
//!
//! The overlay only shows **global CPU usage** and **used/total physical RAM**, so the
//! full `sysinfo::System` refresh (process tables, components, disks…) was overkill to
//! run every tick while a game is in the foreground. `GetSystemTimes` (idle/kernel/user
//! deltas) and `GlobalMemoryStatusEx` give exactly those two numbers in a couple of
//! syscalls, with no allocation and no process enumeration.
//!
//! Windows-only; `metrics.rs` keeps the `sysinfo` path for other targets.

#![cfg(windows)]

use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::GetSystemTimes;

const MB: u64 = 1024 * 1024;

fn ft_to_u64(ft: FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
}

/// Stateful global-CPU% meter. Each `pct()` returns the busy percentage over the
/// interval since the previous call, computed from `GetSystemTimes` deltas. The first
/// call has no previous sample to diff against and returns 0 (same as the old sysinfo
/// path, whose first reading also settled on the next tick).
#[derive(Default)]
pub struct CpuMeter {
    prev_idle: u64,
    prev_kernel: u64,
    prev_user: u64,
    primed: bool,
}

impl CpuMeter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Global CPU usage in 0..=100 since the last call.
    pub fn pct(&mut self) -> f32 {
        let mut idle = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        // `kernel` time already INCLUDES idle time, so busy = (kernel+user) - idle.
        if unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.is_err() {
            return 0.0;
        }
        let idle = ft_to_u64(idle);
        let kernel = ft_to_u64(kernel);
        let user = ft_to_u64(user);

        if !self.primed {
            self.prev_idle = idle;
            self.prev_kernel = kernel;
            self.prev_user = user;
            self.primed = true;
            return 0.0;
        }

        let d_idle = idle.saturating_sub(self.prev_idle);
        let d_kernel = kernel.saturating_sub(self.prev_kernel);
        let d_user = user.saturating_sub(self.prev_user);
        self.prev_idle = idle;
        self.prev_kernel = kernel;
        self.prev_user = user;

        let total = d_kernel + d_user; // includes idle
        if total == 0 {
            return 0.0;
        }
        let busy = total.saturating_sub(d_idle);
        ((busy as f64 / total as f64) * 100.0) as f32
    }
}

/// `(used_mb, total_mb)` physical RAM, or `(0, 0)` if the query fails.
pub fn mem_mb() -> (u64, u64) {
    let mut ms = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    if unsafe { GlobalMemoryStatusEx(&mut ms) }.is_ok() {
        let total = ms.ullTotalPhys / MB;
        let used = ms.ullTotalPhys.saturating_sub(ms.ullAvailPhys) / MB;
        (used, total)
    } else {
        (0, 0)
    }
}
