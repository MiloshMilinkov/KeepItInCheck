//! Extracts an exe's shell icon (48 px from the system image list, 32 px fallback) as PNG.

use super::wide;
use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, HGDIOBJ,
};
use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Controls::{IImageList, ILD_TRANSPARENT};
use windows::Win32::UI::Shell::{
    SHGetFileInfoW, SHGetImageList, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_SYSICONINDEX, SHIL_EXTRALARGE,
};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};

unsafe fn hicon_to_rgba(icon: HICON) -> Option<(u32, u32, Vec<u8>)> {
    let mut info = ICONINFO::default();
    GetIconInfo(icon, &mut info).ok()?;
    let cleanup = |info: &ICONINFO| {
        if !info.hbmColor.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
        }
        if !info.hbmMask.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
        }
    };
    if info.hbmColor.is_invalid() {
        cleanup(&info);
        return None;
    }
    let mut bm = BITMAP::default();
    if GetObjectW(HGDIOBJ(info.hbmColor.0), std::mem::size_of::<BITMAP>() as i32, Some(&mut bm as *mut _ as *mut _)) == 0 {
        cleanup(&info);
        return None;
    }
    let (w, h) = (bm.bmWidth.max(1) as u32, bm.bmHeight.max(1) as u32);
    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w as i32,
            biHeight: -(h as i32), // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let dc = CreateCompatibleDC(None);
    let mut px = vec![0u8; (w * h * 4) as usize];
    let ok = GetDIBits(dc, info.hbmColor, 0, h, Some(px.as_mut_ptr().cast()), &mut bmi, DIB_RGB_COLORS) != 0;

    // Old-style icons have no alpha channel: derive it from the AND mask.
    if ok && px.chunks_exact(4).all(|p| p[3] == 0) && !info.hbmMask.is_invalid() {
        let mut mask = vec![0u8; (w * h * 4) as usize];
        if GetDIBits(dc, info.hbmMask, 0, h, Some(mask.as_mut_ptr().cast()), &mut bmi, DIB_RGB_COLORS) != 0 {
            for (p, m) in px.chunks_exact_mut(4).zip(mask.chunks_exact(4)) {
                p[3] = if m[0] == 0 { 255 } else { 0 };
            }
        } else {
            px.chunks_exact_mut(4).for_each(|p| p[3] = 255);
        }
    }
    let _ = DeleteDC(dc);
    cleanup(&info);
    if !ok {
        return None;
    }
    // BGRA -> RGBA
    for p in px.chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    Some((w, h, px))
}

fn encode_png(w: u32, h: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    Some(out)
}

/// Trim the transparent padding the 48 px list adds around 32 px-only icons, so small icons
/// are not drawn tiny. Returns the original if nothing to trim.
fn is_mostly_empty_48(w: u32, h: u32, rgba: &[u8]) -> bool {
    if w != 48 || h != 48 {
        return false;
    }
    // If nothing is drawn outside the top-left 32x32, the icon is a padded 32 px one.
    (0..h).all(|y| {
        (0..w).all(|x| (x < 32 && y < 32) || rgba[((y * w + x) * 4 + 3) as usize] == 0)
    })
}

pub fn icon_png(exe: &str) -> Option<Vec<u8>> {
    let path = wide(exe);
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let mut sfi = SHFILEINFOW::default();
        let size = std::mem::size_of::<SHFILEINFOW>() as u32;

        // 48 px from the system image list.
        if SHGetFileInfoW(PCWSTR(path.as_ptr()), FILE_ATTRIBUTE_NORMAL, Some(&mut sfi), size, SHGFI_SYSICONINDEX) != 0 {
            if let Ok(list) = SHGetImageList::<IImageList>(SHIL_EXTRALARGE as i32) {
                if let Ok(icon) = list.GetIcon(sfi.iIcon, ILD_TRANSPARENT.0) {
                    let rgba = hicon_to_rgba(icon);
                    let _ = DestroyIcon(icon);
                    if let Some((w, h, px)) = rgba {
                        if !is_mostly_empty_48(w, h, &px) {
                            return encode_png(w, h, &px);
                        }
                    }
                }
            }
        }

        // 32 px fallback.
        let mut sfi = SHFILEINFOW::default();
        if SHGetFileInfoW(PCWSTR(path.as_ptr()), FILE_ATTRIBUTE_NORMAL, Some(&mut sfi), size, SHGFI_ICON | SHGFI_LARGEICON) != 0
            && !sfi.hIcon.is_invalid()
        {
            let rgba = hicon_to_rgba(sfi.hIcon);
            let _ = DestroyIcon(sfi.hIcon);
            if let Some((w, h, px)) = rgba {
                return encode_png(w, h, &px);
            }
        }
        None
    }
}
