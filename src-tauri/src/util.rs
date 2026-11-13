//! Tiny shared helpers.

use serde::Serialize;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn unix_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn unix_millis() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Write JSON via a temp file and rename, so a crash never leaves a half-written file.
pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T, pretty: bool) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let bytes = if pretty { serde_json::to_vec_pretty(value) } else { serde_json::to_vec(value) }
        .map_err(std::io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(tmp, path)
}

/// Read a JSON file, falling back to the default when it is missing or unreadable. Tolerates a
/// UTF-8 BOM left by hand-editing in older editors.
pub fn read_json_or_default<T: serde::de::DeserializeOwned + Default>(path: &Path) -> T {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(s.trim_start_matches('\u{feff}')).ok())
        .unwrap_or_default()
}

/// Seconds to add to UTC to get local time right now (DST included).
pub fn local_offset_secs() -> i64 {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::FILETIME;
        use windows::Win32::Storage::FileSystem::FileTimeToLocalFileTime;
        let ft = crate::platform::unix_to_filetime(unix_secs());
        let utc = FILETIME { dwLowDateTime: ft as u32, dwHighDateTime: (ft >> 32) as u32 };
        let mut local = FILETIME::default();
        if unsafe { FileTimeToLocalFileTime(&utc, &mut local) }.is_ok() {
            let l = ((local.dwHighDateTime as u64) << 32) | local.dwLowDateTime as u64;
            return (l as i64 - ft as i64) / 10_000_000;
        }
    }
    0
}

/// Local calendar day number (days since 1970-01-01 in local time).
pub fn local_day(unix: u64) -> u64 {
    ((unix as i64 + local_offset_secs()).max(0) as u64) / 86_400
}

/// `chrome.exe` -> `chrome` (case-insensitive suffix).
pub fn strip_exe(name: &str) -> &str {
    name.strip_suffix(".exe").or_else(|| name.strip_suffix(".EXE")).unwrap_or(name)
}

#[cfg(test)]
mod tests {
    #[test]
    fn strip_exe() {
        assert_eq!(super::strip_exe("Chrome.EXE"), "Chrome");
        assert_eq!(super::strip_exe("System"), "System");
    }
}
