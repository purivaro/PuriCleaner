//! Read a whole directory's metadata in as few system calls as possible.
//!
//! `getattrlistbulk` returns names, types, sizes and times for many entries
//! per call, instead of one `lstat` per file. On a home folder with millions
//! of files that is the difference between minutes and seconds. Volumes that
//! do not support it fall back to `read_dir` + `lstat`.

use std::cell::RefCell;
use std::ffi::CString;
use std::fs;
use std::io;
use std::os::macos::fs::MetadataExt as _;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
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
        out.push(Entry {
            name: e.file_name().to_string_lossy().into(),
            kind,
            alloc: md.blocks() * 512,
            mtime: md.mtime(),
            nlink: md.nlink() as u32,
            fileid: md.ino(),
            flags: md.st_flags(),
        });
    }
    Ok(out)
}
