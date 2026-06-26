//! Process elevation helpers for the admin-only metrics (CPU temp via the LHM
//! sidecar, NVIDIA FPS via PresentMon). Windows can't elevate a running process,
//! so the UI offers a "Restart as admin" action that relaunches via the `runas`
//! verb (UAC prompt); the old instance then exits.

#![cfg(windows)]

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// True if this process is running with an elevated (administrator) token.
pub fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut size = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut core::ffi::c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    }
}

/// Relaunch our own executable elevated via the `runas` verb. Returns Ok once the
/// elevated process has been requested (the caller should then exit this one). An
/// `Err` means the user declined UAC or the launch failed.
pub fn relaunch_elevated() -> Result<(), String> {
    use windows::core::{w, HSTRING, PCWSTR};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_w = HSTRING::from(exe.as_os_str());

    let result = unsafe {
        ShellExecuteW(
            None,
            w!("runas"),
            PCWSTR(exe_w.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecuteW returns an HINSTANCE; values <= 32 indicate failure.
    if result.0 as isize > 32 {
        Ok(())
    } else {
        Err("No se pudo reiniciar como administrador (UAC cancelado).".into())
    }
}

/// Name of the Task Scheduler entry used to autostart Meteor elevated.
const AUTOSTART_TASK: &str = "MeteorAutostart";

/// Run `schtasks` without flashing a console window, returning whether it
/// exited successfully.
fn schtasks(args: &[&str]) -> std::io::Result<bool> {
    use std::os::windows::process::CommandExt;
    // CREATE_NO_WINDOW so the console of schtasks.exe never flashes.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let status = std::process::Command::new("schtasks")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    Ok(status.success())
}

/// Whether the elevated logon autostart task exists.
pub fn logon_task_exists() -> bool {
    schtasks(&["/Query", "/TN", AUTOSTART_TASK]).unwrap_or(false)
}

/// Create (or overwrite) a Task Scheduler entry that launches Meteor at logon
/// with highest privileges. This is the only way Windows will autostart an app
/// that requires elevation (UAC) without a prompt at every login — a plain
/// `HKCU\...\Run` entry is silently blocked for elevated apps. Requires the
/// current process to be elevated (creating a `/RL HIGHEST` task needs admin).
pub fn create_logon_task() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // schtasks parses its own /TR value, so wrap the path in quotes for spaces.
    let tr = format!("\"{}\"", exe.display());
    let ok = schtasks(&[
        "/Create", "/TN", AUTOSTART_TASK, "/TR", &tr, "/SC", "ONLOGON", "/RL",
        "HIGHEST", "/F",
    ])
    .map_err(|e| e.to_string())?;
    if ok {
        Ok(())
    } else {
        Err("No se pudo crear la tarea de inicio automático (schtasks).".into())
    }
}

/// Remove the elevated logon autostart task. No-op (Ok) if it doesn't exist.
pub fn delete_logon_task() -> Result<(), String> {
    if !logon_task_exists() {
        return Ok(());
    }
    let ok = schtasks(&["/Delete", "/TN", AUTOSTART_TASK, "/F"]).map_err(|e| e.to_string())?;
    if ok {
        Ok(())
    } else {
        Err("No se pudo borrar la tarea de inicio automático (schtasks).".into())
    }
}
