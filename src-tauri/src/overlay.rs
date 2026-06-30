//! Overlay HUD facade (Windows only).
//!
//! Picks the **MPO-friendly DirectComposition backend** (`overlay_dcomp`) when it
//! initializes, otherwise falls back to the GDI layered window (`overlay_native`).
//! A DirectComposition + DXGI flip swapchain is the surface type DWM can promote to a
//! hardware overlay plane, so the game keeps its independent-flip (low-latency) path;
//! a GDI layered window forces desktop composition. The fallback keeps the HUD working
//! on machines where D3D/DComp init fails.
//!
//! All calls come from the single metrics sampler thread (it owns the HUD window), so
//! backend selection lives in a `thread_local` and the COM objects never cross threads.

#![cfg(windows)]

use std::cell::Cell;

use crate::metrics::MetricsSample;
use crate::models::OverlaySettings;

// Backend selection: 0 = not chosen yet, 1 = DirectComposition, 3 = disabled.
//
// There is intentionally **no GDI fallback for the in-game HUD**. A GDI layered
// window (`UpdateLayeredWindow`) is never MPO-eligible, so it *always* drops the
// game from independent-flip to composed-flip → input lag. That's the exact cost
// we're trying to avoid, so if DirectComposition can't init we disable the HUD
// rather than hand the user a guaranteed-laggy overlay. (`overlay_native` stays
// only for the backend-independent foreground helpers below.)
thread_local! {
    static BACKEND: Cell<u8> = const { Cell::new(0) };
}

/// One HUD line: label, formatted value, and the value's RGB color.
pub(crate) struct HudRow {
    pub label: &'static str,
    pub value: String,
    pub rgb: (u8, u8, u8),
}

/// Parse a CSS hex color ("#rrggbb") to (r, g, b). Bad input → white.
pub(crate) fn parse_rgb(hex: &str) -> (u8, u8, u8) {
    let h = hex.trim().trim_start_matches('#');
    if h.len() >= 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&h[0..2], 16),
            u8::from_str_radix(&h[2..4], 16),
            u8::from_str_radix(&h[4..6], 16),
        ) {
            return (r, g, b);
        }
    }
    (255, 255, 255)
}

/// Temperature → color (matches the web HUD): red ≥85, amber ≥75, else emerald.
pub(crate) fn temp_rgb(c: u32) -> (u8, u8, u8) {
    if c >= 85 {
        (0xf8, 0x71, 0x71)
    } else if c >= 75 {
        (0xfb, 0xbf, 0x24)
    } else {
        (0x34, 0xd3, 0x99)
    }
}

/// Build the HUD title + visible rows from the config + sample (single source of
/// truth shared by both backends, so the metric list never drifts).
pub(crate) fn build_rows(cfg: &OverlaySettings, m: &MetricsSample) -> (Option<String>, Vec<HudRow>) {
    let accent = parse_rgb(&cfg.accent_color);
    let value = parse_rgb(&cfg.value_color);
    let mut rows: Vec<HudRow> = Vec::new();
    let gb = |mb: u64| format!("{:.1}", mb as f64 / 1024.0);

    if cfg.show_fps {
        if let Some(f) = m.fps {
            rows.push(HudRow { label: "FPS", value: format!("{:.0}", f), rgb: accent });
        }
    }
    if cfg.show_frametime {
        if let Some(ft) = m.frametime_ms {
            rows.push(HudRow { label: "Frame", value: format!("{:.1} ms", ft), rgb: value });
        }
    }
    if cfg.show_gpu {
        if let Some(u) = m.gpu_usage {
            rows.push(HudRow { label: "GPU", value: format!("{}%", u), rgb: accent });
        }
    }
    if cfg.show_gpu_temp {
        if let Some(t) = m.gpu_temp_c {
            rows.push(HudRow { label: "GPU °C", value: format!("{}°", t), rgb: temp_rgb(t) });
        }
    }
    if cfg.show_vram {
        if let (Some(u), Some(t)) = (m.vram_used_mb, m.vram_total_mb) {
            rows.push(HudRow { label: "VRAM", value: format!("{}/{} GB", gb(u), gb(t)), rgb: value });
        }
    }
    if cfg.show_cpu {
        rows.push(HudRow { label: "CPU", value: format!("{:.0}%", m.cpu_usage), rgb: accent });
    }
    if cfg.show_cpu_temp {
        if let Some(t) = m.cpu_temp_c {
            rows.push(HudRow { label: "CPU °C", value: format!("{}°", t), rgb: temp_rgb(t) });
        }
    }
    if cfg.show_ram {
        rows.push(HudRow {
            label: "RAM",
            value: format!("{}/{} GB", gb(m.ram_used_mb), gb(m.ram_total_mb)),
            rgb: value,
        });
    }

    let title = m.game.as_deref().map(|g| g.to_uppercase());
    (title, rows)
}

fn current() -> u8 {
    BACKEND.with(|b| b.get())
}

/// Draw + present the HUD via DirectComposition. If DComp can't init (or fails at
/// runtime) the HUD is disabled for the session — no GDI fallback, so we never
/// force composition on the game (see the BACKEND comment).
pub fn render(cfg: &OverlaySettings, m: &MetricsSample, mon_w: i32, mon_h: i32, scale: f64) {
    let mut b = current();
    if b == 0 {
        b = if crate::overlay_dcomp::try_init() {
            crate::overlay_diag::log("backend: DirectComposition (MPO-friendly)");
            1
        } else {
            crate::overlay_diag::log(
                "DirectComposition no disponible → HUD desactivado (sin fallback GDI)",
            );
            3
        };
        BACKEND.with(|c| c.set(b));
    }
    if b == 1 && !crate::overlay_dcomp::render(cfg, m, mon_w, mon_h, scale) {
        // Runtime failure: disable the HUD for the rest of the session.
        crate::overlay_dcomp::hide();
        BACKEND.with(|c| c.set(3));
    }
}

/// Hide the HUD (no-op if no backend has rendered yet).
pub fn hide() {
    if current() == 1 {
        crate::overlay_dcomp::hide();
    }
}

/// Drain the HUD window's pending messages (no-op until a backend exists).
pub fn pump() {
    if current() == 1 {
        crate::overlay_dcomp::pump();
    }
}

/// Re-assert topmost after a foreground change (no-op until a backend exists).
pub fn reassert_topmost() {
    if current() == 1 {
        crate::overlay_dcomp::reassert_topmost();
    }
}

/// Raw composition mode of the HUD swapchain (0=COMPOSED, 1=OVERLAY, 2=NONE,
/// 3=FAILURE), or `None` if not measurable. The runtime MPO signal: OVERLAY = the HUD
/// is on a hardware plane (free), COMPOSED = DWM is compositing it (costing the game).
pub fn composition_mode() -> Option<i32> {
    if current() == 1 {
        crate::overlay_dcomp::composition_mode()
    } else {
        None
    }
}

/// Log the swapchain's actual composition mode (OVERLAY=hardware plane vs
/// COMPOSED=DWM compositing) — the definitive runtime MPO check. No-op unless the
/// DComp backend is active and diagnostics are enabled.
pub fn log_composition_mode() {
    if current() == 1 {
        crate::overlay_dcomp::log_composition_mode();
    } else {
        crate::overlay_diag::log("composición: HUD no activo en backend DComp");
    }
}

/// Foreground window handle (0 = none); backend-independent.
pub fn foreground() -> isize {
    crate::overlay_native::foreground()
}

/// PID owning the foreground window (0 = none); backend-independent.
pub fn foreground_pid() -> u32 {
    crate::overlay_native::foreground_pid()
}
