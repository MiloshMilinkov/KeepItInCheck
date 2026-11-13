//! Startup apps: the Run keys and Startup folders, enabled/disabled the way Task Manager does it.
//!
//! Disabling never deletes anything. Windows keeps a 12-byte flag per entry under
//! `Explorer\StartupApproved\{Run,Run32,StartupFolder}`: the first byte is 2 (enabled) or 3
//! (disabled), followed by the time it was disabled. Explorer honours that flag at sign-in, so a
//! disabled entry can be turned back on at any time, here or in Task Manager / Settings.

use super::super::{StartupItem, StartupSource};
use super::{unix_to_filetime, wide};
use windows::core::{Owned, PCWSTR, PWSTR};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    RegCreateKeyExW, RegEnumValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE, KEY_WOW64_32KEY, KEY_WOW64_64KEY, REG_BINARY, REG_EXPAND_SZ,
    REG_OPTION_NON_VOLATILE, REG_SAM_FLAGS, REG_SZ,
};

const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved";

/// Where entries come from: a Run key (with its registry view) or a Startup folder.
enum Kind {
    Run { path: String, view: REG_SAM_FLAGS },
    Folder(String),
}

/// One place startup entries live, plus the key that holds their on/off flags.
pub struct Location {
    source: StartupSource,
    /// Prefix of item ids from this location (`user-run:Steam`).
    tag: &'static str,
    hive: HKEY,
    kind: Kind,
    approved: String,
}

/// The real locations on this PC.
pub fn system_locations() -> Vec<Location> {
    let run = |source, tag, hive, view, sub: &str| Location {
        source,
        tag,
        hive,
        kind: Kind::Run { path: RUN.into(), view },
        approved: format!(r"{APPROVED}\{sub}"),
    };
    let mut v = vec![
        run(StartupSource::UserRun, "user-run", HKEY_CURRENT_USER, KEY_WOW64_64KEY, "Run"),
        run(StartupSource::MachineRun, "machine-run", HKEY_LOCAL_MACHINE, KEY_WOW64_64KEY, "Run"),
        run(StartupSource::MachineRun32, "machine-run32", HKEY_LOCAL_MACHINE, KEY_WOW64_32KEY, "Run32"),
    ];
    let startup = r"Microsoft\Windows\Start Menu\Programs\Startup";
    let folder = |source, tag, hive, base: Result<String, std::env::VarError>| {
        base.ok().map(|b| Location {
            source,
            tag,
            hive,
            kind: Kind::Folder(format!(r"{b}\{startup}")),
            approved: format!(r"{APPROVED}\StartupFolder"),
        })
    };
    v.extend(folder(StartupSource::UserFolder, "user-folder", HKEY_CURRENT_USER, std::env::var("APPDATA")));
    v.extend(folder(StartupSource::CommonFolder, "common-folder", HKEY_LOCAL_MACHINE, std::env::var("ProgramData")));
    v
}

// ---- pure helpers (unit-tested) ---------------------------------------------------------------

/// A missing flag means enabled. Odd first byte (3, 7) means disabled.
fn approved_enabled(flag: Option<&[u8]>) -> bool {
    flag.and_then(|b| b.first()).is_none_or(|b| b & 1 == 0)
}

/// The 12-byte flag Windows expects. `filetime` (100 ns since 1601) records when it was disabled.
fn approved_flag(enabled: bool, filetime: u64) -> [u8; 12] {
    let mut out = [0u8; 12];
    out[0] = if enabled { 2 } else { 3 };
    if !enabled {
        out[4..].copy_from_slice(&filetime.to_le_bytes());
    }
    out
}

/// Expand `%VAR%` references using this process's environment.
fn expand_env(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) => {
                let name = &after[..end];
                match std::env::var(name) {
                    Ok(v) if !name.is_empty() => out.push_str(&v),
                    _ => out.push_str(&rest[start..start + end + 2]),
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// The executable a Run command launches: quoted path, or everything up to `.exe`.
fn exe_from_command(cmd: &str) -> Option<String> {
    let cmd = expand_env(cmd.trim());
    if let Some(stripped) = cmd.strip_prefix('"') {
        return stripped.find('"').map(|end| stripped[..end].to_string()).filter(|s| !s.is_empty());
    }
    let lower = cmd.to_lowercase();
    if let Some(i) = lower.find(".exe") {
        return Some(cmd[..i + 4].to_string());
    }
    cmd.split_whitespace().next().map(str::to_string)
}

// ---- registry -----------------------------------------------------------------------------------

fn open(hive: HKEY, path: &str, access: REG_SAM_FLAGS) -> Option<Owned<HKEY>> {
    let p = wide(path);
    let mut k = HKEY::default();
    let rc = unsafe { RegOpenKeyExW(hive, PCWSTR(p.as_ptr()), None, access, &mut k) };
    (rc == ERROR_SUCCESS).then(|| unsafe { Owned::new(k) })
}

/// All values of a key as (name, type, data). One buffer is reused for every value.
fn values(key: HKEY) -> Vec<(String, u32, Vec<u8>)> {
    let mut out = Vec::new();
    let mut name = [0u16; 512];
    let mut data = vec![0u8; 8192];
    for i in 0.. {
        let mut name_len = name.len() as u32;
        let mut data_len = data.len() as u32;
        let mut ty = 0u32;
        let rc = unsafe {
            RegEnumValueW(
                key,
                i,
                Some(PWSTR(name.as_mut_ptr())),
                &mut name_len,
                None,
                Some(&mut ty),
                Some(data.as_mut_ptr()),
                Some(&mut data_len),
            )
        };
        if rc != ERROR_SUCCESS {
            break;
        }
        out.push((String::from_utf16_lossy(&name[..name_len as usize]), ty, data[..data_len as usize].to_vec()));
    }
    out
}

fn reg_string(data: &[u8]) -> String {
    let units: Vec<u16> = data.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    let end = units.iter().position(|&c| c == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end])
}

/// This user's entries can always be changed; all-users entries only as administrator.
fn writable(loc: &Location) -> bool {
    loc.hive == HKEY_CURRENT_USER || open(loc.hive, &loc.approved, KEY_SET_VALUE).is_some()
}

/// Raw entries of one location: (value or file name, command, exe).
fn entries(loc: &Location) -> Vec<(String, String, Option<String>)> {
    match &loc.kind {
        Kind::Run { path, view } => open(loc.hive, path, KEY_READ | *view)
            .map(|k| values(*k))
            .unwrap_or_default()
            .into_iter()
            .filter(|(_, ty, _)| *ty == REG_SZ.0 || *ty == REG_EXPAND_SZ.0)
            .map(|(name, _, data)| {
                let cmd = reg_string(&data);
                let exe = exe_from_command(&cmd);
                (name, cmd, exe)
            })
            .collect(),
        Kind::Folder(dir) => std::fs::read_dir(dir)
            .map(|rd| {
                rd.filter_map(Result::ok)
                    .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .filter(|n| !n.eq_ignore_ascii_case("desktop.ini"))
                    .map(|n| {
                        let path = format!(r"{dir}\{n}");
                        (n, path.clone(), Some(path))
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

pub fn list(locations: &[Location]) -> Vec<StartupItem> {
    let mut out = Vec::new();
    for loc in locations {
        let flags = open(loc.hive, &loc.approved, KEY_READ).map(|k| values(*k)).unwrap_or_default();
        let can_change = writable(loc);
        for (value, command, exe) in entries(loc) {
            let flag = flags.iter().find(|(n, _, _)| n.eq_ignore_ascii_case(&value)).map(|(_, _, d)| d.as_slice());
            let name = match loc.kind {
                Kind::Run { .. } => value.clone(),
                Kind::Folder(_) => std::path::Path::new(&value)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| value.clone()),
            };
            let publisher = exe
                .as_deref()
                .filter(|e| e.to_lowercase().ends_with(".exe"))
                .and_then(|e| super::version::file_info(e).company);
            out.push(StartupItem {
                id: format!("{}:{}", loc.tag, value),
                name,
                command,
                exe,
                publisher,
                source: loc.source,
                enabled: approved_enabled(flag),
                can_change,
            });
        }
    }
    out.sort_by_cached_key(|i| (!i.enabled, i.name.to_lowercase()));
    out
}

/// Flip one entry's StartupApproved flag. The entry must exist right now, and the Run value or
/// shortcut itself is never touched.
pub fn set_enabled(locations: &[Location], id: &str, enabled: bool) -> Result<(), String> {
    let (tag, value) = id.split_once(':').ok_or("Unknown startup entry")?;
    let loc = locations.iter().find(|l| l.tag == tag).ok_or("Unknown startup location")?;
    let exists = match &loc.kind {
        Kind::Run { path, view } => open(loc.hive, path, KEY_READ | *view).is_some_and(|k| {
            let v = wide(value);
            unsafe { RegQueryValueExW(*k, PCWSTR(v.as_ptr()), None, None, None, None) == ERROR_SUCCESS }
        }),
        // A shortcut name, never a path: refuse anything that could point outside the folder.
        Kind::Folder(dir) => !value.contains(['\\', '/']) && value != ".." && std::path::Path::new(dir).join(value).is_file(),
    };
    if !exists {
        return Err("That startup entry no longer exists".into());
    }
    if !writable(loc) {
        return Err("Changing this entry needs administrator rights".into());
    }
    let path = wide(&loc.approved);
    let mut key = HKEY::default();
    let rc = unsafe {
        RegCreateKeyExW(loc.hive, PCWSTR(path.as_ptr()), None, PCWSTR::null(), REG_OPTION_NON_VOLATILE, KEY_SET_VALUE, None, &mut key, None)
    };
    if rc != ERROR_SUCCESS {
        return Err(format!("Could not open the startup settings ({})", rc.0));
    }
    let key = unsafe { Owned::new(key) };
    let name = wide(value);
    let flag = approved_flag(enabled, unix_to_filetime(crate::util::unix_secs()));
    match unsafe { RegSetValueExW(*key, PCWSTR(name.as_ptr()), None, REG_BINARY, Some(&flag)) } {
        rc if rc == ERROR_SUCCESS => Ok(()),
        rc => Err(format!("Could not save the change ({})", rc.0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Registry::RegDeleteTreeW;

    #[test]
    fn flag_format_matches_task_manager() {
        assert!(approved_enabled(None));
        assert!(approved_enabled(Some(&[2, 0, 0, 0])));
        assert!(approved_enabled(Some(&[6, 0, 0, 0])));
        assert!(!approved_enabled(Some(&[3, 0, 0, 0])));
        assert!(!approved_enabled(Some(&[7, 0, 0, 0])));
        let f = approved_flag(false, 0x0102030405060708);
        assert_eq!(f[0], 3);
        assert_eq!(&f[4..], &0x0102030405060708u64.to_le_bytes());
        assert_eq!(approved_flag(true, 99), [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn exe_is_parsed_from_run_commands() {
        assert_eq!(
            exe_from_command(r#""C:\Program Files (x86)\Steam\steam.exe" -silent"#).as_deref(),
            Some(r"C:\Program Files (x86)\Steam\steam.exe")
        );
        assert_eq!(exe_from_command(r"C:\Tools\My App\app.EXE /background").as_deref(), Some(r"C:\Tools\My App\app.EXE"));
        std::env::set_var("KIC_TEST_DIR", r"D:\x");
        assert_eq!(exe_from_command(r"%KIC_TEST_DIR%\run.exe").as_deref(), Some(r"D:\x\run.exe"));
        assert_eq!(expand_env("100%"), "100%");
    }

    /// End to end against a scratch key under HKCU, never the real Run key.
    #[test]
    fn toggling_writes_only_the_approved_flag() {
        const BASE: &str = r"Software\KeepItInCheck-test";
        let locs = [Location {
            source: StartupSource::UserRun,
            tag: "user-run",
            hive: HKEY_CURRENT_USER,
            kind: Kind::Run { path: format!(r"{BASE}\Run"), view: KEY_WOW64_64KEY },
            approved: format!(r"{BASE}\StartupApproved\Run"),
        }];
        let base = wide(BASE);
        unsafe {
            let _ = RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(base.as_ptr()));
            let run = wide(&format!(r"{BASE}\Run"));
            let mut k = HKEY::default();
            assert_eq!(
                RegCreateKeyExW(HKEY_CURRENT_USER, PCWSTR(run.as_ptr()), None, PCWSTR::null(), REG_OPTION_NON_VOLATILE, KEY_SET_VALUE, None, &mut k, None),
                ERROR_SUCCESS
            );
            let k = Owned::new(k);
            let data: Vec<u8> = wide(r#""C:\Apps\demo.exe" --tray"#).iter().flat_map(|c| c.to_le_bytes()).collect();
            let name = wide("Demo");
            assert_eq!(RegSetValueExW(*k, PCWSTR(name.as_ptr()), None, REG_SZ, Some(&data)), ERROR_SUCCESS);
        }

        let items = list(&locs);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "user-run:Demo");
        assert_eq!(items[0].exe.as_deref(), Some(r"C:\Apps\demo.exe"));
        assert!(items[0].enabled && items[0].can_change);

        set_enabled(&locs, "user-run:Demo", false).unwrap();
        assert!(!list(&locs)[0].enabled);
        set_enabled(&locs, "user-run:Demo", true).unwrap();
        assert!(list(&locs)[0].enabled);
        assert!(set_enabled(&locs, "user-run:Missing", false).is_err());
        assert!(set_enabled(&locs, "machine-run:Demo", false).is_err());

        unsafe {
            let _ = RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(base.as_ptr()));
        }
    }
}
