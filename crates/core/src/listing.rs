//! Read a whole directory's metadata in as few system calls as possible.
//!
//! macOS: `getattrlistbulk` returns names, types, sizes and times for many
//! entries per call, instead of one `lstat` per file. On a home folder with
//! millions of files that is the difference between minutes and seconds.
//! Volumes that do not support it fall back to `read_dir`.
//!
//! Windows: `read_dir` already gets size, time and attributes for a whole
//! directory from FindFirstFile/FindNextFile, so nothing extra is needed.

use std::fs;
use std::io;
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    File,
    Dir,
    Link,
    Other,
}

pub struct Entry {
    pub name: Box<str>,
    pub kind: Kind,
    /// Bytes allocated on disk.
    pub alloc: u64,
    pub mtime: i64,
    pub nlink: u32,
    pub fileid: u64,
    pub flags: u32,
}

#[cfg(target_os = "macos")]
mod bulk {
    use super::*;
    use std::cell::RefCell;
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    #[repr(C)]
    struct AttrList {
        bitmapcount: u16,
        reserved: u16,
        commonattr: u32,
        volattr: u32,
        dirattr: u32,
        fileattr: u32,
        forkattr: u32,
    }

    extern "C" {
        fn getattrlistbulk(fd: i32, attr_list: *mut AttrList, buf: *mut u8, size: usize, options: u64) -> i32;
    }

    const ATTR_BIT_MAP_COUNT: u16 = 5;
    const ATTR_CMN_NAME: u32 = 0x0000_0001;
    const ATTR_CMN_OBJTYPE: u32 = 0x0000_0008;
    const ATTR_CMN_MODTIME: u32 = 0x0000_0400;
    const ATTR_CMN_FLAGS: u32 = 0x0004_0000;
    const ATTR_CMN_FILEID: u32 = 0x0200_0000;
    const ATTR_CMN_ERROR: u32 = 0x2000_0000;
    const ATTR_CMN_RETURNED_ATTRS: u32 = 0x8000_0000;
    const ATTR_FILE_LINKCOUNT: u32 = 0x0000_0001;
    const ATTR_FILE_ALLOCSIZE: u32 = 0x0000_0004;

    const VREG: u32 = 1;
    const VDIR: u32 = 2;
    const VLNK: u32 = 5;

    const BUF_SIZE: usize = 128 * 1024;

    thread_local! {
        static BUF: RefCell<Vec<u8>> = RefCell::new(vec![0u8; BUF_SIZE]);
    }

    fn rd<T: Copy>(buf: &[u8], at: usize) -> T {
        assert!(at + std::mem::size_of::<T>() <= buf.len());
        unsafe { std::ptr::read_unaligned(buf.as_ptr().add(at) as *const T) }
    }

    pub fn list(path: &Path) -> io::Result<Vec<Entry>> {
        match list_bulk(path) {
            Ok(v) => Ok(v),
            Err(e) if e.raw_os_error() == Some(libc::ENOTSUP) || e.raw_os_error() == Some(libc::EINVAL) => {
                list_slow(path)
            }
            Err(e) => Err(e),
        }
    }

    fn list_bulk(path: &Path) -> io::Result<Vec<Entry>> {
        let c = CString::new(path.as_os_str().as_bytes()).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        struct Fd(i32);
        impl Drop for Fd {
            fn drop(&mut self) {
                unsafe { libc::close(self.0) };
            }
        }
        let fd = Fd(fd);

        let mut al = AttrList {
            bitmapcount: ATTR_BIT_MAP_COUNT,
            reserved: 0,
            commonattr: ATTR_CMN_RETURNED_ATTRS
                | ATTR_CMN_NAME
                | ATTR_CMN_ERROR
                | ATTR_CMN_OBJTYPE
                | ATTR_CMN_MODTIME
                | ATTR_CMN_FLAGS
                | ATTR_CMN_FILEID,
            volattr: 0,
            dirattr: 0,
            fileattr: ATTR_FILE_LINKCOUNT | ATTR_FILE_ALLOCSIZE,
            forkattr: 0,
        };

        BUF.with(|cell| {
            let mut buf = cell.borrow_mut();
            let mut out = Vec::new();
            loop {
                let n = unsafe { getattrlistbulk(fd.0, &mut al, buf.as_mut_ptr(), buf.len(), 0) };
                if n < 0 {
                    return Err(io::Error::last_os_error());
                }
                if n == 0 {
                    break;
                }
                let mut off = 0usize;
                for _ in 0..n {
                    let len: u32 = rd(&buf, off);
                    let entry = &buf[off..off + len as usize];
                    off += len as usize;
                    if let Some(e) = parse(entry) {
                        out.push(e);
                    }
                }
            }
            Ok(out)
        })
    }

    fn parse(b: &[u8]) -> Option<Entry> {
        // length, then the attribute_set_t of what was actually returned.
        let mut p = 4;
        let common: u32 = rd(b, p);
        let file: u32 = rd(b, p + 12);
        p += 20;

        // ATTR_CMN_ERROR, when present, sits right after the returned set.
        if common & ATTR_CMN_ERROR != 0 {
            let err: u32 = rd(b, p);
            p += 4;
            if err != 0 {
                return None;
            }
        }
        let mut name: Box<str> = "".into();
        if common & ATTR_CMN_NAME != 0 {
            let off: i32 = rd(b, p);
            let len: u32 = rd(b, p + 4);
            let start = (p as i64 + off as i64) as usize;
            let end = start + len.saturating_sub(1) as usize; // drop the NUL
            name = String::from_utf8_lossy(b.get(start..end)?).into();
            p += 8;
        }
        let mut kind = Kind::Other;
        if common & ATTR_CMN_OBJTYPE != 0 {
            let t: u32 = rd(b, p);
            kind = match t {
                VREG => Kind::File,
                VDIR => Kind::Dir,
                VLNK => Kind::Link,
                _ => Kind::Other,
            };
            p += 4;
        }
        let mut mtime = 0i64;
        if common & ATTR_CMN_MODTIME != 0 {
            mtime = rd::<i64>(b, p);
            p += 16;
        }
        let mut flags = 0u32;
        if common & ATTR_CMN_FLAGS != 0 {
            flags = rd(b, p);
            p += 4;
        }
        let mut fileid = 0u64;
        if common & ATTR_CMN_FILEID != 0 {
            fileid = rd(b, p);
            p += 8;
        }
        let mut nlink = 1u32;
        if file & ATTR_FILE_LINKCOUNT != 0 {
            nlink = rd(b, p);
            p += 4;
        }
        let mut alloc = 0u64;
        if file & ATTR_FILE_ALLOCSIZE != 0 {
            alloc = rd::<i64>(b, p).max(0) as u64;
        }
        if name.is_empty() {
            return None;
        }
        Some(Entry { name, kind, alloc, mtime, nlink, fileid, flags })
    }


}

#[cfg(target_os = "macos")]
pub use bulk::list;

#[cfg(not(target_os = "macos"))]
pub fn list(path: &Path) -> io::Result<Vec<Entry>> {
    list_slow(path)
}

/// SF_DATALESS: an iCloud placeholder that is not on this Mac.
#[cfg(unix)]
const CLOUD_ONLY: u32 = 0x4000_0000;
/// FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS | RECALL_ON_OPEN | OFFLINE: a
/// OneDrive (or other cloud) placeholder whose data is not on this PC.
#[cfg(windows)]
const CLOUD_ONLY: u32 = 0x0040_0000 | 0x0004_0000 | 0x0000_1000;

impl Entry {
    /// True for cloud placeholders. Opening such a folder can start a
    /// download, and such a file takes no space, so the scan skips them.
    pub fn cloud_only(&self) -> bool {
        self.flags & CLOUD_ONLY != 0
    }
}

#[cfg(unix)]
fn meta(md: &fs::Metadata) -> (u64, i64, u32, u64, u32) {
    use std::os::unix::fs::MetadataExt;
    #[cfg(target_os = "macos")]
    let flags = std::os::macos::fs::MetadataExt::st_flags(md);
    #[cfg(not(target_os = "macos"))]
    let flags = 0;
    (md.blocks() * 512, md.mtime(), md.nlink() as u32, md.ino(), flags)
}

#[cfg(windows)]
fn meta(md: &fs::Metadata) -> (u64, i64, u32, u64, u32) {
    use std::os::windows::fs::MetadataExt;
    let attrs = md.file_attributes();
    // Allocated size is not in the directory listing; whole 4 KiB clusters
    // is what NTFS uses for all but tiny (resident) files.
    let alloc = if attrs & CLOUD_ONLY != 0 { 0 } else { md.file_size().div_ceil(4096) * 4096 };
    // FILETIME: 100 ns ticks since 1601-01-01.
    let mtime = (md.last_write_time() / 10_000_000) as i64 - 11_644_473_600;
    (alloc, mtime, 1, 0, attrs)
}

fn list_slow(path: &Path) -> io::Result<Vec<Entry>> {
    let mut out = Vec::new();
    for e in fs::read_dir(path)?.flatten() {
        let Ok(md) = e.metadata() else { continue };
        let ft = md.file_type();
        let kind = if ft.is_dir() {
            Kind::Dir
        } else if ft.is_file() {
            Kind::File
        } else if ft.is_symlink() {
            Kind::Link
        } else {
            Kind::Other
        };
        let (alloc, mtime, nlink, fileid, flags) = meta(&md);
        out.push(Entry { name: e.file_name().to_string_lossy().into(), kind, alloc, mtime, nlink, fileid, flags });
    }
    Ok(out)
}
