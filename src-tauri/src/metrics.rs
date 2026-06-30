//! In-game metrics overlay sampler.
//!
//! A single background thread samples hardware telemetry and draws it straight into
//! the native HUD window via the `overlay` facade (DirectComposition when available,
//! GDI fallback). It is fully idle-cheap: it only samples + draws while the overlay is
//! enabled, a game is running, AND that game is the foreground window (the playtime
//! watcher publishes the current game + pid here). On systems without an NVIDIA GPU the
//! GPU fields are simply omitted; AMD GPUs are read via ADLX.
//!
//! FPS / frametime come from ADLX (AMD, no admin) or the PresentMon ETW controller
//! (NVIDIA, admin only) — both degrade silently to `None`. Everything else works
//! without admin.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use sysinfo::System;
use tauri::{AppHandle, Manager};

#[cfg(windows)]
use crate::overlay;

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
/// Whether ADLX is currently supplying FPS (AMD's native, admin-free counter). When
/// true the PresentMon controller stays idle — running an ETW session per frame for
/// a number we'd only discard is pure overhead (and needs admin).
static ADLX_FPS_ACTIVE: AtomicBool = AtomicBool::new(false);
/// Sampling interval in milliseconds.
static INTERVAL_MS: AtomicU64 = AtomicU64::new(1000);
/// PID of the running game's main process (for PresentMon). 0 = none.
static CURRENT_PID: AtomicU32 = AtomicU32::new(0);
/// Name of the running game the overlay should label, set by the playtime watcher.
static CURRENT_GAME: Mutex<Option<String>> = Mutex::new(None);
/// Which GPU to sample: "auto" | "nvml:<i>" | "adlx:<i>". None = "auto".
static GPU_SELECT: Mutex<Option<String>> = Mutex::new(None);
/// Whether the in-game overlay *settings* screen (WebView2 window) is open. While
/// it is, the native HUD hides so the two overlays don't fight for the z-order.
static SETTINGS_OPEN: AtomicBool = AtomicBool::new(false);
/// Full overlay render config (colors, font size, which metrics, position…),
/// snapshotted so the native HUD renderer can read it each tick.
static RENDER_CFG: Mutex<Option<crate::models::OverlaySettings>> = Mutex::new(None);

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

/// Mark the in-game overlay settings screen open/closed (hides/shows the native HUD).
pub fn set_settings_open(open: bool) {
    SETTINGS_OPEN.store(open, Ordering::Relaxed);
}

/// Snapshot the full overlay config for the native HUD renderer.
pub fn set_render_cfg(cfg: crate::models::OverlaySettings) {
    *RENDER_CFG.lock().unwrap() = Some(cfg);
}

fn render_cfg() -> Option<crate::models::OverlaySettings> {
    RENDER_CFG.lock().unwrap().clone()
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

/// Whether PresentMon should be running: overlay on, an FPS metric enabled, and
/// ADLX isn't already providing FPS (on AMD it is → no need for the ETW session).
pub fn want_fps() -> bool {
    OVERLAY_ENABLED.load(Ordering::Relaxed)
        && FPS_WANTED.load(Ordering::Relaxed)
        && !ADLX_FPS_ACTIVE.load(Ordering::Relaxed)
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
        // Last foreground window we re-asserted topmost against. We only restack
        // when the foreground actually changes (see the topmost block below) —
        // toggling NOTOPMOST→TOPMOST every tick forces DWM to recomposite and
        // knocks the game out of independent-flip, causing a periodic hitch.
        #[cfg(windows)]
        let mut last_fg: isize = 0;
        // Last GPU selection applied to ADLX, so we only re-select when it changes.
        #[cfg(windows)]
        let mut applied_gpu = String::new();

        loop {
            std::thread::sleep(Duration::from_millis(INTERVAL_MS.load(Ordering::Relaxed)));

            // Drain the HUD window's message queue (passive, but stays responsive).
            #[cfg(windows)]
            overlay::pump();

            // Idle path: overlay off, no game, or the in-game settings screen open →
            // keep the native HUD hidden (the WebView2 window shows the settings).
            let active = OVERLAY_ENABLED.load(Ordering::Relaxed);
            let settings_open = SETTINGS_OPEN.load(Ordering::Relaxed);
            let raw_game = if active && !settings_open { current_game() } else { None };
            // Only draw while the game is the *foreground* window. If you alt-tab out,
            // the game stays "running" (so playtime keeps counting) but drawing a
            // topmost HUD over the desktop would force composition for nothing — and we
            // skip sampling entirely too. If the pid is unknown (0) we don't gate, to
            // avoid hiding the HUD on a process we couldn't resolve.
            #[cfg(windows)]
            let pid = CURRENT_PID.load(Ordering::Relaxed);
            #[cfg(windows)]
            let fg_pid = if raw_game.is_some() { overlay::foreground_pid() } else { 0 };
            // Only draw while the game is the foreground window. Alt-tabbed out, drawing
            // a topmost HUD over the desktop forces composition for nothing — and on an
            // MPO-denied config that costs the same as in-game. If the pid is unknown (0)
            // we don't gate, to avoid hiding the HUD on a process we couldn't resolve.
            #[cfg(windows)]
            let game = if raw_game.is_some() && pid != 0 && fg_pid != pid {
                None
            } else {
                raw_game.clone()
            };
            #[cfg(not(windows))]
            let game = raw_game;
            if game.is_none() {
                if shown {
                    #[cfg(windows)]
                    overlay::hide();
                    shown = false;
                    #[cfg(windows)]
                    {
                        last_fg = 0; // re-assert topmost when we show again
                    }
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
            // Re-armed below only if the ADLX path actually supplies FPS this tick;
            // cleared otherwise so PresentMon takes over (e.g. NVML selected).
            ADLX_FPS_ACTIVE.store(false, Ordering::Relaxed);
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
                    // targeting / admin); prefer it over PresentMon when present and
                    // flag it so the PresentMon controller stays idle (no ETW session).
                    if let Some(f) = g.fps {
                        sample.fps = Some(f);
                        sample.frametime_ms = Some(1000.0 / f);
                        ADLX_FPS_ACTIVE.store(true, Ordering::Relaxed);
                    } else {
                        ADLX_FPS_ACTIVE.store(false, Ordering::Relaxed);
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

            // Draw the native HUD via the overlay facade: a content-sized window backed
            // by a DirectComposition flip swapchain (MPO-friendly → the game keeps its
            // independent-flip, low-latency path), or the GDI layered window as fallback.
            // No Chromium compositor either way. The window is owned + pumped by this
            // thread, and only drawn when its content changed (present-on-change).
            // NOTE: true exclusive-fullscreen (D3D independent flip) bypasses the DWM
            // compositor entirely — no HWND overlay can appear on top without DLL
            // injection into the game process.
            #[cfg(windows)]
            {
                if let Some(cfg) = render_cfg() {
                    let (mon_w, mon_h, scale) = app
                        .get_webview_window("overlay")
                        .and_then(|w| w.primary_monitor().ok().flatten())
                        .map(|m| (m.size().width as i32, m.size().height as i32, m.scale_factor()))
                        .unwrap_or((1920, 1080, 1.0));
                    overlay::render(&cfg, &sample, mon_w, mon_h, scale);
                    shown = true;
                    // Re-assert topmost only when the foreground window *changes*; the
                    // NOTOPMOST→TOPMOST toggle forces DWM to recomposite (per-tick
                    // toggling caused a periodic stutter), so gate it on a real change.
                    let fg = overlay::foreground();
                    if fg != 0 && fg != last_fg {
                        overlay::reassert_topmost();
                        last_fg = fg;
                    }
                }
            }
        }
    });
}
