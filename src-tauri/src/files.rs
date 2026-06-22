use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Total size in bytes of a directory tree. Walks iteratively (no recursion
/// limit) and skips entries it can't read instead of failing the whole call.
pub fn dir_size(path: &str) -> Result<u64, String> {
    let mut total: u64 = 0;
    let mut stack = vec![PathBuf::from(path)];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_dir() {
                stack.push(entry.path());
            } else if ft.is_file() {
                if let Ok(md) = entry.metadata() {
                    total += md.len();
                }
            }
        }
    }
    Ok(total)
}

/// Open a folder (or a file's location) in the OS file manager.
pub fn open_path(path: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|e| format!("No se pudo abrir «{path}»: {e}"))?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("No se pudo abrir «{path}»: {e}"))?;
    }
    Ok(())
}
