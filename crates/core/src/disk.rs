use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// (total, available) bytes of the volume holding `path`.
pub fn usage(path: &Path) -> (u64, u64) {
    let Ok(c) = CString::new(path.as_os_str().as_bytes()) else { return (0, 0) };
    let mut s: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(c.as_ptr(), &mut s) } != 0 {
        return (0, 0);
    }
    let bs = s.f_bsize as u64;
    (s.f_blocks as u64 * bs, s.f_bavail as u64 * bs)
}
