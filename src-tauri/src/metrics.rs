//! In-game metrics overlay sampler.
//!
//! A single background thread samples hardware telemetry and pushes it to the
//! transparent `overlay` window via the `metrics-sample` event. It is fully
//! idle-cheap: it only samples while the overlay is enabled *and* a game is
//! running (the playtime watcher publishes the current game + pid here). On
//! systems without an NVIDIA GPU the GPU fields are simply omitted.
//!
//! FPS / frametime / present-latency are left as `None` here; they come from the
//! PresentMon (ETW) integration added in a later phase, which needs the bundled
//! PresentMon binary and elevation — everything else works without admin.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use sysinfo::System;
use tauri::{AppHandle, Emitter, Manager};

use nvml_wrapper::enum_wrappers::device::{Clock, TemperatureSensor};
use nvml_wrapper::Nvml;

/// Master switch, mirrors `AppSettings.overlay.enabled`.
static OVERLAY_ENABLED: AtomicBool = AtomicBool::new(false);
/// Whether any FPS-class metric (fps/frametime) is enabled, so PresentMon only
/// runs when its output is actually shown.
static FPS_WANTED: AtomicBool = AtomicBool::new(false);
/// Whether CPU temperature is enabled, so the LHM sidecar (kernel driver) only
/// runs when its output is actually shown.
static CPU_TEMP_WANTED: AtomicBool = AtomicBool::new(false);
/// Sampling interval in milliseconds.
static INTERVAL_MS: AtomicU64 = AtomicU64::new(1000);
/// PID of the running game's main process (for PresentMon). 0 = none.
static CURRENT_PID: AtomicU32 = AtomicU32::new(0);
/// Name of the running game the overlay should label, set by the playtime watcher.
static CURRENT_GAME: Mutex<Option<String>> = Mutex::new(None);
/// Which GPU to sample: "auto" | "nvml:<i>" | "adlx:<i>". None = "auto".
static GPU_SELECT: Mutex<Option<String>> = Mutex::new(None);

/// One telemetry sample sent to the overlay window.
#[derive(Clone, Serialize)]
pub struct MetricsSample {
    pub game: Option<String>,
    pub cpu_usage: f32,
    pub ram_used_mb: u64,
    pub ram_total_mb: u64,
    pub gpu_usage: Option<u32>,
    pub gpu_temp_c: Option<u32>,
    pub vram_used_mb: Option<u64>,
    pub vram_total_mb: Option<u64>,
    pub gpu_clock_mhz: Option<u32>,
    pub gpu_power_w: Option<f32>,
    // CPU temperature from the LibreHardwareMonitor sidecar (admin + driver).
    pub cpu_temp_c: Option<u32>,
    // Filled by the PresentMon integration in a later phase.
    pub fps: Option<f32>,
    pub frametime_ms: Option<f32>,
}

/// Apply overlay settings live (called on startup, on settings change, on hotkey).
pub fn configure(enabled: bool, interval_ms: u64, fps_wanted: bool, cpu_temp_wanted: bool) {
    OVERLAY_ENABLED.store(enabled, Ordering::Relaxed);
    FPS_WANTED.store(fps_wanted, Ordering::Relaxed);
    CPU_TEMP_WANTED.store(cpu_temp_wanted, Ordering::Relaxed);
    INTERVAL_MS.store(interval_ms.clamp(200, 5000), Ordering::Relaxed);
}

/// Set which GPU the sampler reads: "auto" | "nvml:<i>" | "adlx:<i>".
pub fn set_gpu(sel: String) {
    *GPU_SELECT.lock().unwrap() = Some(sel);
}

fn current_gpu() -> String {
    GPU_SELECT
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| "auto".to_string())
}

/// Published by the playtime watcher each poll: the foreground game (if any).
pub fn set_current_game(name: Option<String>, pid: Option<u32>) {
    CURRENT_PID.store(pid.unwrap_or(0), Ordering::Relaxed);
    *CURRENT_GAME.lock().unwrap() = name;
}

fn current_game() -> Option<String> {
    CURRENT_GAME.lock().unwrap().clone()
}

/// PID of the running game (0 = none). Read by the PresentMon controller.
pub fn current_pid() -> u32 {
    CURRENT_PID.load(Ordering::Relaxed)
}

/// Whether PresentMon should be running: overlay on and an FPS metric enabled.
pub fn want_fps() -> bool {
    OVERLAY_ENABLED.load(Ordering::Relaxed) && FPS_WANTED.load(Ordering::Relaxed)
}

/// Whether the CPU-temp sidecar should run: overlay on and CPU temp enabled.
pub fn want_cpu_temp() -> bool {
    OVERLAY_ENABLED.load(Ordering::Relaxed) && CPU_TEMP_WANTED.load(Ordering::Relaxed)
}

/// Whether a game is currently running (the overlay's gate for the sidecar).
pub fn has_game() -> bool {
    CURRENT_GAME.lock().unwrap().is_some()
}

const MB: u64 = 1024 * 1024;

/// Start the sampler thread. Spawned once from `setup`.
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        // nvml.dll is loaded once; absent/failed on non-NVIDIA systems → no GPU.
        let nvml = Nvml::init().ok();
        // Also init AMD's ADLX (loads amdadlx64.dll) so machines with both vendors
        // can pick either GPU. Fails gracefully (amd=false) without an AMD driver.
        // Init from this thread; all ADLX calls stay on it.
        #[cfg(windows)]
        let amd = crate::amd::init();
        let mut sys = System::new();
        // Tracks whether the overlay window is currently shown, to avoid spamming
        // show()/hide() every tick.
        let mut shown = false;
        // Last GPU selection applied to ADLX, so we only re-select when it changes.
        #[cfg(windows)]
        let mut applied_gpu = String::new();

        loop {
            std::thread::sleep(Duration::from_millis(INTERVAL_MS.load(Ordering::Relaxed)));

            // Idle path: overlay off or no game running → keep the window hidden.
            let active = OVERLAY_ENABLED.load(Ordering::Relaxed);
            let game = if active { current_game() } else { None };
            if game.is_none() {
                if shown {
                    if let Some(w) = app.get_webview_window("overlay") {
                        let _ = w.hide();
                    }
                    shown = false;
                }
                continue;
            }

            // CPU + RAM (sysinfo). The first reading after init may be 0%; it
            // settles on the next tick.
            sys.refresh_cpu();
            sys.refresh_memory();
            let cpu_usage = sys.global_cpu_info().cpu_usage();
            let ram_used_mb = sys.used_memory() / MB;
            let ram_total_mb = sys.total_memory() / MB;

            // GPU (NVIDIA via NVML), all best-effort.
            let mut sample = MetricsSample {
                game,
                cpu_usage,
                ram_used_mb,
                ram_total_mb,
                gpu_usage: None,
                gpu_temp_c: None,
                vram_used_mb: None,
                vram_total_mb: None,
                gpu_clock_mhz: None,
                gpu_power_w: None,
                cpu_temp_c: None,
                fps: None,
                frametime_ms: None,
            };
            // CPU temperature from the LibreHardwareMonitor sidecar (None unless
            // it's running with admin + a loadable driver).
            sample.cpu_temp_c = crate::cputemp::current();

            // FPS / frametime from the PresentMon controller (None unless it's
            // running with admin + the bundled binary). The ADLX path below may
            // override this with AMD's native FPS.
            let (fps, frametime) = crate::presentmon::current();
            sample.fps = fps;
            sample.frametime_ms = frametime;

            // Which GPU to read: "auto" | "nvml:<i>" | "adlx:<i>" (see set_gpu).
            let sel = current_gpu();
            let nvml_idx = sel.strip_prefix("nvml:").and_then(|s| s.parse::<u32>().ok());
            #[cfg(windows)]
            let want_adlx = sel.starts_with("adlx:");

            // Apply a changed ADLX selection (only when it changes — re-selecting
            // every tick is wasteful). "auto" on an AMD-only box prefers discrete.
            #[cfg(windows)]
            if amd && sel != applied_gpu {
                if let Some(i) = sel.strip_prefix("adlx:").and_then(|s| s.parse::<usize>().ok()) {
                    crate::amd::select(i);
                } else if nvml.is_none() {
                    let gpus = crate::amd::list_gpus();
                    let idx = gpus.iter().position(|g| g.kind == "Discreta").unwrap_or(0);
                    crate::amd::select(idx);
                }
                applied_gpu = sel.clone();
            }

            // Sample the chosen backend. ADLX wins when explicitly picked or when
            // there's no NVIDIA; otherwise NVML (default index 0, or the picked one).
            let mut gpu_filled = false;
            #[cfg(windows)]
            if amd && (want_adlx || nvml.is_none()) {
                if let Some(g) = crate::amd::sample() {
                    sample.gpu_usage = g.usage;
                    sample.gpu_temp_c = g.temp_c;
                    sample.vram_used_mb = g.vram_used_mb;
                    sample.vram_total_mb = g.vram_total_mb;
                    sample.gpu_clock_mhz = g.clock_mhz;
                    sample.gpu_power_w = g.power_w;
                    // ADLX reports FPS of the focused app natively (no PID
                    // targeting / admin); prefer it over PresentMon when present.
                    if let Some(f) = g.fps {
                        sample.fps = Some(f);
                        sample.frametime_ms = Some(1000.0 / f);
                    }
                    gpu_filled = true;
                }
            }
            if !gpu_filled {
                if let Some(nvml) = &nvml {
                    if let Ok(dev) = nvml.device_by_index(nvml_idx.unwrap_or(0)) {
                        if let Ok(u) = dev.utilization_rates() {
                            sample.gpu_usage = Some(u.gpu);
                        }
                        if let Ok(t) = dev.temperature(TemperatureSensor::Gpu) {
                            sample.gpu_temp_c = Some(t);
                        }
                        if let Ok(mem) = dev.memory_info() {
                            sample.vram_used_mb = Some(mem.used / MB);
                            sample.vram_total_mb = Some(mem.total / MB);
                        }
                        if let Ok(clk) = dev.clock_info(Clock::Graphics) {
                            sample.gpu_clock_mhz = Some(clk);
                        }
                        if let Ok(mw) = dev.power_usage() {
                            sample.gpu_power_w = Some(mw as f32 / 1000.0);
                        }
                    }
                }
            }

            // Show the overlay and push the sample to it. We re-assert topmost every
            // tick: a game that just took the foreground (especially borderless) lands
            // above us otherwise. Tauri's set_always_on_top is a no-op once we're
            // already topmost, so on Windows we toggle the z-order to restack to the
            // very top of the topmost band (see `force_topmost`).
            if let Some(w) = app.get_webview_window("overlay") {
                if !shown {
                    let _ = w.show();
                    shown = true;
                }
                let _ = w.set_always_on_top(true);
                #[cfg(windows)]
                force_topmost(&w);
                let _ = app.emit_to("overlay", "metrics-sample", &sample);
            }
        }
    });
}

/// Force a window to the very top of the topmost z-order band.
///
/// `SetWindowPos(HWND_TOPMOST)` on an already-topmost window does *not* restack
/// it, so a game activated after us stays on top. Toggling NOTOPMOST→TOPMOST
/// reinserts the overlay at the front of the band. `SWP_NOACTIVATE` keeps input
/// focus on the game (the overlay is click-through anyway). This fixes windowed
/// and borderless (DWM-composited) games; true exclusive fullscreen / independent
/// flip bypasses the compositor and can't be overlaid without injection.
#[cfg(windows)]
fn force_topmost(window: &tauri::WebviewWindow) {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    };
    let Ok(hwnd) = window.hwnd() else { return };
    let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
    unsafe {
        let _ = SetWindowPos(hwnd, Some(HWND_NOTOPMOST), 0, 0, 0, 0, flags);
        let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, flags);
    }
}
