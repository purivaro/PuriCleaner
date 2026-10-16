use std::path::Path;

/// (total, available) bytes of the volume holding `path`.
#[cfg(unix)]
pub fn usage(path: &Path) -> (u64, u64) {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = CString::new(path.as_os_str().as_bytes()) else { return (0, 0) };
    let mut s: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(c.as_ptr(), &mut s) } != 0 {
        return (0, 0);
    }
    let bs = s.f_bsize as u64;
    (s.f_blocks as u64 * bs, s.f_bavail as u64 * bs)
}

#[cfg(windows)]
fn wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}

/// (total, available) bytes of the volume holding `path`.
#[cfg(windows)]
pub fn usage(path: &Path) -> (u64, u64) {
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
    let ok = unsafe { GetDiskFreeSpaceExW(wide(path).as_ptr(), &mut avail, &mut total, &mut free) };
    if ok == 0 {
        (0, 0)
    } else {
        (total, avail)
    }
}

/// Recycle Bin contents on all drives: (bytes, items).
#[cfg(windows)]
pub fn recycle_bin() -> Option<(u64, u64)> {
    use windows_sys::Win32::UI::Shell::{SHQueryRecycleBinW, SHQUERYRBINFO};
    let mut info = SHQUERYRBINFO { cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32, i64Size: 0, i64NumItems: 0 };
    let hr = unsafe { SHQueryRecycleBinW(std::ptr::null(), &mut info) };
    (hr == 0).then_some((info.i64Size.max(0) as u64, info.i64NumItems.max(0) as u64))
}

/// The macOS Trash is a normal folder in home and is handled as one.
#[cfg(not(windows))]
pub fn recycle_bin() -> Option<(u64, u64)> {
    None
}

/// Empty the Recycle Bin on all drives, without a confirmation dialog
/// (the app already asked).
#[cfg(windows)]
pub fn empty_recycle_bin() -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::{SHEmptyRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND};
    let hr = unsafe {
        SHEmptyRecycleBinW(std::ptr::null_mut(), std::ptr::null(), SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND)
    };
    if hr == 0 {
        Ok(())
    } else {
        Err(format!("SHEmptyRecycleBin failed (0x{hr:08x})"))
    }
}

#[cfg(not(windows))]
pub fn empty_recycle_bin() -> Result<(), String> {
    Err("not available on this system".into())
}
