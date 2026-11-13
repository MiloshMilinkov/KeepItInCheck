//! Reads FileDescription / ProductName / CompanyName from an exe's version resource.

use super::super::FileInfo;
use super::wide;
use windows::core::{w, PCWSTR};
use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};

pub fn file_info(exe: &str) -> FileInfo {
    let path = wide(exe);
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(path.as_ptr()), None);
        if size == 0 {
            return FileInfo::default();
        }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(PCWSTR(path.as_ptr()), None, size, data.as_mut_ptr().cast()).is_err() {
            return FileInfo::default();
        }

        // Pick the first language/codepage pair, falling back to US English / Unicode.
        let mut langs: Vec<(u16, u16)> = Vec::new();
        let mut ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        if VerQueryValueW(data.as_ptr().cast(), w!("\\VarFileInfo\\Translation"), &mut ptr, &mut len).as_bool() && len >= 4 {
            let pairs = std::slice::from_raw_parts(ptr as *const u16, (len / 2) as usize);
            langs.extend(pairs.chunks_exact(2).map(|c| (c[0], c[1])));
        }
        langs.push((0x0409, 0x04B0));
        langs.push((0x0409, 0x04E4));

        let query = |name: &str| -> Option<String> {
            for (lang, cp) in &langs {
                let key = wide(&format!("\\StringFileInfo\\{lang:04x}{cp:04x}\\{name}"));
                let mut p: *mut core::ffi::c_void = std::ptr::null_mut();
                let mut l = 0u32;
                if VerQueryValueW(data.as_ptr().cast(), PCWSTR(key.as_ptr()), &mut p, &mut l).as_bool() && l > 1 {
                    let s = std::slice::from_raw_parts(p as *const u16, l as usize);
                    let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
                    let v = String::from_utf16_lossy(&s[..end]).trim().to_string();
                    if !v.is_empty() {
                        return Some(v);
                    }
                }
            }
            None
        };

        FileInfo {
            description: query("FileDescription"),
            product: query("ProductName"),
            company: query("CompanyName"),
        }
    }
}
