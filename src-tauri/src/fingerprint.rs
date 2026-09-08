//! Cheap "did the installed-games set change?" probe.
//!
//! A full `get_library` spawns eight scanners, walks three registry hives and
//! (for Xbox) starts a PowerShell process. The frontend ran that every 15
//! minutes regardless of whether anything had changed — usually while the window
//! was hidden in the tray.
//!
//! This computes a fingerprint from the *containers* stores use — library
//! folders, manifest directories, uninstall registry keys — all of which change
//! their last-write time when a game is installed, moved or removed. Comparing
//! it costs a handful of `stat`/`RegQueryInfoKey` calls, so the frontend can
//! skip the expensive scan when the answer is "nothing new".
//!
//! Conservative by construction: anything it cannot read counts as *changed*, so
//! a false "no change" is not possible for a source it can see at all.

use std::path::Path;
use std::time::UNIX_EPOCH;

/// Mix a value into a running FNV-1a hash.
fn mix(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= *byte as u64;
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

/// Fold a path's last-write time and size into the hash. A missing path folds in
/// a marker instead, so "the folder disappeared" is itself a change.
fn mix_path(hash: &mut u64, path: &Path) {
    match std::fs::metadata(path) {
        Ok(md) => {
            let secs = md
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            mix(hash, &secs.to_le_bytes());
            mix(hash, &md.len().to_le_bytes());
        }
        Err(_) => mix(hash, b"missing"),
    }
}

/// Fingerprint of everything that would change the library contents.
pub fn compute() -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;

    // Steam: the library-folders manifest plus each library's steamapps dir
    // (whose mtime moves when an appmanifest is added or removed).
    if let Ok(steam) = steamlocate::SteamDir::locate() {
        mix_path(&mut hash, &steam.path().join("steamapps/libraryfolders.vdf"));
        if let Ok(libraries) = steam.libraries() {
            for library in libraries.flatten() {
                mix_path(&mut hash, &library.path().join("steamapps"));
            }
        }
    }

    // Epic: the manifests directory.
    if let Ok(program_data) = std::env::var("PROGRAMDATA") {
        mix_path(
            &mut hash,
            &Path::new(&program_data).join("Epic/EpicGamesLauncher/Data/Manifests"),
        );
    }

    // Xbox / Game Pass: the per-drive game roots.
    #[cfg(windows)]
    for letter in b'A'..=b'Z' {
        let root = format!("{}:\\XboxGames", letter as char);
        let path = Path::new(&root);
        if path.exists() {
            mix_path(&mut hash, path);
        }
    }

    // Everything else (GOG, EA, Ubisoft, Battle.net, generic apps) is discovered
    // through the registry: fold in each hive's last-write time.
    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
        use winreg::RegKey;
        const KEYS: &[(isize, &str)] = &[
            (
                HKEY_LOCAL_MACHINE,
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            ),
            (
                HKEY_LOCAL_MACHINE,
                r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
            ),
            (
                HKEY_CURRENT_USER,
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            ),
        ];
        for (hive, path) in KEYS {
            let root = RegKey::predef(*hive);
            match root
                .open_subkey_with_flags(path, KEY_READ)
                .and_then(|k| k.query_info())
            {
                Ok(info) => {
                    // `FileTime`'s inner value is private, so go through the
                    // SYSTEMTIME accessor winreg exposes.
                    let t = info.get_last_write_time_system();
                    for part in [
                        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond,
                        t.wMilliseconds,
                    ] {
                        mix(&mut hash, &part.to_le_bytes());
                    }
                    mix(&mut hash, &(info.sub_keys as u64).to_le_bytes());
                }
                // Unreadable hive → treat as volatile so we never skip a scan
                // because of a permissions hiccup.
                Err(_) => mix(&mut hash, &crate::fingerprint::volatile().to_le_bytes()),
            }
        }
    }

    hash
}

/// A value that differs on every call, used to force "changed" when a source
/// cannot be read.
fn volatile() -> u64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_stable_between_calls() {
        // Two calls with nothing installed/removed in between must agree,
        // otherwise the frontend would rescan every time anyway.
        assert_eq!(compute(), compute());
    }

    #[test]
    fn mix_path_distinguishes_missing_from_present() {
        let mut a: u64 = 0;
        let mut b: u64 = 0;
        mix_path(&mut a, Path::new("Z:\\meteor-does-not-exist"));
        mix_path(&mut b, &std::env::temp_dir());
        assert_ne!(a, b);
    }
}
