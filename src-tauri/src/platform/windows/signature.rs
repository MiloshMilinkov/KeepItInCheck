//! Authenticode verification (embedded signature first, then the system catalog).
//!
//! Revocation checks are disabled and URL retrieval is cache-only, so verification never touches
//! the network. A signature only counts if `WinVerifyTrust` says the chain is trusted, so a
//! self-signed certificate that merely *claims* to be Microsoft is rejected.

use super::super::Signature;
use super::{wide, Owned};
use windows::core::{w, GUID, PCWSTR};
use windows::Win32::Foundation::{HWND, INVALID_HANDLE_VALUE};
use windows::Win32::Security::Cryptography::Catalog::{
    CryptCATAdminAcquireContext2, CryptCATAdminCalcHashFromFileHandle2, CryptCATAdminEnumCatalogFromHash,
    CryptCATAdminReleaseCatalogContext, CryptCATAdminReleaseContext, CryptCATCatalogInfoFromContext, CATALOG_INFO,
};
use windows::Win32::Security::Cryptography::{CertGetNameStringW, CERT_NAME_SIMPLE_DISPLAY_TYPE};
use windows::Win32::Security::WinTrust::{
    WTHelperGetProvSignerFromChain, WTHelperProvDataFromStateData, WinVerifyTrust, WINTRUST_ACTION_GENERIC_VERIFY_V2,
    WINTRUST_CATALOG_INFO, WINTRUST_DATA, WINTRUST_DATA_0, WINTRUST_FILE_INFO, WTD_CACHE_ONLY_URL_RETRIEVAL,
    WTD_CHOICE_CATALOG, WTD_CHOICE_FILE, WTD_REVOCATION_CHECK_NONE, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE,
    WTD_STATEACTION_VERIFY, WTD_UI_NONE,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING,
};

/// Runs WinVerifyTrust and, on success, returns the leaf signer's display name.
unsafe fn verify(data: &mut WINTRUST_DATA) -> Option<String> {
    let mut action: GUID = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    let hwnd = HWND(INVALID_HANDLE_VALUE.0); // no UI, ever
    data.dwStateAction = WTD_STATEACTION_VERIFY;
    let status = WinVerifyTrust(hwnd, &mut action, data as *mut _ as *mut _);
    let mut signer = None;
    if status == 0 {
        let prov = WTHelperProvDataFromStateData(data.hWVTStateData);
        if !prov.is_null() {
            let sgnr = WTHelperGetProvSignerFromChain(prov, 0, false, 0);
            if !sgnr.is_null() && (*sgnr).csCertChain > 0 && !(*sgnr).pasCertChain.is_null() {
                let cert = (*(*sgnr).pasCertChain).pCert;
                if !cert.is_null() {
                    let mut buf = [0u16; 256];
                    let n = CertGetNameStringW(cert, CERT_NAME_SIMPLE_DISPLAY_TYPE, 0, None, Some(&mut buf));
                    if n > 1 {
                        signer = Some(String::from_utf16_lossy(&buf[..(n - 1) as usize]));
                    }
                }
            }
        }
    }
    data.dwStateAction = WTD_STATEACTION_CLOSE;
    let _ = WinVerifyTrust(hwnd, &mut action, data as *mut _ as *mut _);
    signer
}

fn base_data() -> WINTRUST_DATA {
    WINTRUST_DATA {
        cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL | WTD_REVOCATION_CHECK_NONE,
        ..Default::default()
    }
}

unsafe fn embedded(path: &[u16]) -> Option<String> {
    let mut file = WINTRUST_FILE_INFO {
        cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: PCWSTR(path.as_ptr()),
        ..Default::default()
    };
    let mut data = base_data();
    data.dwUnionChoice = WTD_CHOICE_FILE;
    data.Anonymous = WINTRUST_DATA_0 { pFile: &mut file };
    verify(&mut data)
}

/// Many Windows binaries are not signed themselves; their hash is listed in a signed catalog.
unsafe fn catalog(path: &[u16]) -> Option<String> {
    let file = Owned(
        CreateFileW(
            PCWSTR(path.as_ptr()),
            FILE_GENERIC_READ.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
        .ok()?,
    );
    let mut admin: isize = 0;
    CryptCATAdminAcquireContext2(&mut admin, None, w!("SHA256"), None, None).ok()?;

    let mut result = None;
    let mut hash_len = 0u32;
    let _ = CryptCATAdminCalcHashFromFileHandle2(admin, file.0, &mut hash_len, None, None);
    if hash_len > 0 && hash_len <= 128 {
        let mut hash = vec![0u8; hash_len as usize];
        if CryptCATAdminCalcHashFromFileHandle2(admin, file.0, &mut hash_len, Some(hash.as_mut_ptr()), None).is_ok() {
            let cat = CryptCATAdminEnumCatalogFromHash(admin, &hash, None, None);
            if cat != 0 {
                let mut info = CATALOG_INFO { cbStruct: std::mem::size_of::<CATALOG_INFO>() as u32, ..Default::default() };
                if CryptCATCatalogInfoFromContext(cat, &mut info, 0).is_ok() {
                    let tag: String = hash.iter().map(|b| format!("{b:02X}")).collect();
                    let tag = wide(&tag);
                    let mut ci = WINTRUST_CATALOG_INFO {
                        cbStruct: std::mem::size_of::<WINTRUST_CATALOG_INFO>() as u32,
                        pcwszCatalogFilePath: PCWSTR(info.wszCatalogFile.as_ptr()),
                        pcwszMemberTag: PCWSTR(tag.as_ptr()),
                        pcwszMemberFilePath: PCWSTR(path.as_ptr()),
                        hMemberFile: file.0,
                        pbCalculatedFileHash: hash.as_mut_ptr(),
                        cbCalculatedFileHash: hash_len,
                        hCatAdmin: admin,
                        ..Default::default()
                    };
                    let mut data = base_data();
                    data.dwUnionChoice = WTD_CHOICE_CATALOG;
                    data.Anonymous = WINTRUST_DATA_0 { pCatalog: &mut ci };
                    result = verify(&mut data);
                }
                let _ = CryptCATAdminReleaseCatalogContext(admin, cat, 0);
            }
        }
    }
    let _ = CryptCATAdminReleaseContext(admin, 0);
    result
}

pub fn check(exe: &str) -> Signature {
    if !std::path::Path::new(exe).is_file() {
        return Signature::Other;
    }
    let path = wide(exe);
    match unsafe { embedded(&path).or_else(|| catalog(&path)) } {
        // "Microsoft Corporation", "Microsoft Windows", "Microsoft Windows Publisher", ...
        Some(signer) if signer.starts_with("Microsoft ") => Signature::Microsoft,
        _ => Signature::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notepad_is_microsoft_signed() {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
        let sig = check(&format!(r"{windir}\System32\notepad.exe"));
        // notepad.exe is either embedded- or catalog-signed by Microsoft on every Windows build.
        assert_eq!(sig, Signature::Microsoft);
    }

    #[test]
    fn missing_file_is_unknown() {
        assert_eq!(check(r"C:\definitely\not\here.exe"), Signature::Other);
    }
}
