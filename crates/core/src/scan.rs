//! Parallel directory walk that measures real on-disk usage.
//!
//! Sizes are allocated blocks (`st_blocks * 512`), not the logical length,
//! so sparse files (e.g. Docker.raw) and iCloud placeholders that are not
//! downloaded are counted at what they really cost. Hard-linked files are
//! counted once. Symlinks are never followed and other volumes are skipped.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::Mutex;

use rayon::prelude::*;

use crate::listing::{self, Kind};
use crate::tree::{flag, marker_flag, Node, Tree, BIG_FILE, NCAT, NONE};

pub struct Progress {
    pub files: AtomicU64,
    pub dirs: AtomicU64,
    pub bytes: AtomicU64,
    pub errors: AtomicU64,
    pub current: Mutex<String>,
    pub cancel: AtomicBool,
    /// Folder each worker thread is reading right now. When the scan stalls
    /// (macOS waiting on a permission prompt), this says where.
    pub reading: Vec<Mutex<String>>,
}

impl Default for Progress {
    fn default() -> Self {
        Progress {
            files: AtomicU64::new(0),
            dirs: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            current: Mutex::new(String::new()),
            cancel: AtomicBool::new(false),
            reading: (0..64).map(|_| Mutex::new(String::new())).collect(),
        }
    }
}

impl Progress {
    pub fn waiting_on(&self) -> Vec<String> {
        self.reading.iter().filter_map(|m| m.try_lock().ok()).filter(|s| !s.is_empty()).map(|s| s.clone()).collect()
    }
}

impl Progress {
    pub fn reset(&self) {
        self.files.store(0, Relaxed);
        self.dirs.store(0, Relaxed);
        self.bytes.store(0, Relaxed);
        self.errors.store(0, Relaxed);
        self.cancel.store(false, Relaxed);
        self.current.lock().unwrap().clear();

    }
}

#[cfg(target_os = "macos")]
extern "C" {
    fn setiopolicy_np(iotype: i32, scope: i32, policy: i32) -> i32;
}

/// Ask macOS never to download cloud placeholders on our behalf, so just
/// looking at a folder cannot pull gigabytes from iCloud.
#[cfg(target_os = "macos")]
pub fn forbid_cloud_downloads() {
    const IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES: i32 = 3;
    const IOPOL_SCOPE_PROCESS: i32 = 0;
    const IOPOL_MATERIALIZE_DATALESS_FILES_OFF: i32 = 1;
    unsafe {
        setiopolicy_np(
            IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES,
            IOPOL_SCOPE_PROCESS,
            IOPOL_MATERIALIZE_DATALESS_FILES_OFF,
        );
    }
}

/// Elsewhere the scan simply skips placeholders (see `Entry::cloud_only`).
#[cfg(not(target_os = "macos"))]
pub fn forbid_cloud_downloads() {}

struct SFile {
    name: Box<str>,
    size: u64,
    mtime: i64,
}

#[derive(Default)]
struct SDir {
    name: Box<str>,
    size: u64,
    own: u64,
    own_count: u64,
    files: u64,
    newest: i64,
    own_newest: i64,
    flags: u16,
    dirs: Vec<SDir>,
    big: Vec<SFile>,
}

/// File IDs already counted, split across locks so 32 threads rarely wait.
struct Seen(Vec<Mutex<HashSet<u64>>>);

impl Seen {
    fn new() -> Self {
        Seen((0..64).map(|_| Mutex::new(HashSet::new())).collect())
    }
    /// True the first time an ID is seen.
    fn first(&self, id: u64) -> bool {
        let shard = (id.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 58) as usize;
        self.0[shard].lock().unwrap().insert(id)
    }
}

struct Ctx<'a> {
    prog: &'a Progress,
    seen_links: Seen,
    /// Other volumes mounted inside the scanned folders (simulator runtimes,
    /// disk images). Their bytes are not on this disk, so they are skipped.
    mounts: HashSet<PathBuf>,
}

/// Every mount point except the system and data volumes themselves.
#[cfg(target_os = "macos")]
fn foreign_mounts() -> HashSet<PathBuf> {
    let mut out = HashSet::new();
    unsafe {
        let n = libc::getfsstat(std::ptr::null_mut(), 0, libc::MNT_NOWAIT);
        if n <= 0 {
            return out;
        }
        let mut buf: Vec<libc::statfs> = vec![std::mem::zeroed(); n as usize + 8];
        let size = (buf.len() * std::mem::size_of::<libc::statfs>()) as libc::c_int;
        let n = libc::getfsstat(buf.as_mut_ptr(), size, libc::MNT_NOWAIT);
        for st in buf.iter().take(n.max(0) as usize) {
            let p = std::ffi::CStr::from_ptr(st.f_mntonname.as_ptr()).to_string_lossy().into_owned();
            if p != "/" && p != "/System/Volumes/Data" {
                out.insert(PathBuf::from(p));
            }
        }
    }
    out
}

/// Windows reports mounted folders as reparse points, which the scan already
/// skips, so there is nothing extra to collect.
#[cfg(not(target_os = "macos"))]
fn foreign_mounts() -> HashSet<PathBuf> {
    HashSet::new()
}

fn scan_dir(path: &Path, name: Box<str>, ctx: &Ctx) -> SDir {
    let mut d = SDir { name, ..Default::default() };
    if ctx.prog.cancel.load(Relaxed) {
        return d;
    }
    let n = ctx.prog.dirs.fetch_add(1, Relaxed);
    if n % 512 == 0 {
        if let Ok(mut cur) = ctx.prog.current.try_lock() {
            *cur = path.display().to_string();
        }
    }
    let slot = rayon::current_thread_index().and_then(|i| ctx.prog.reading.get(i));
    if let Some(m) = slot {
        *m.lock().unwrap() = path.display().to_string();
    }
    let listed = listing::list(path);
    if let Some(m) = slot {
        m.lock().unwrap().clear();
    }
    let entries = match listed {
        Ok(v) => v,
        Err(_) => {
            d.flags |= flag::UNREADABLE;
            ctx.prog.errors.fetch_add(1, Relaxed);
            return d;
        }
    };

    let mut subdirs: Vec<(PathBuf, Box<str>)> = Vec::new();
    let mut bytes_here = 0u64;
    let mut files_here = 0u64;
    for e in entries {
        d.flags |= marker_flag(&e.name);
        if e.kind == Kind::Dir {
            // An iCloud folder that is not on this Mac: opening it would
            // start a download, so leave it alone (it takes no space).
            if !e.cloud_only() {
                let p = path.join(&*e.name);
                if ctx.mounts.is_empty() || !ctx.mounts.contains(&p) {
                    subdirs.push((p, e.name));
                }
            }
            continue;
        }
        let mut alloc = e.alloc;
        if e.kind == Kind::File && e.nlink > 1 {
            let first = ctx.seen_links.first(e.fileid);
            if !first {
                alloc = 0;
            }
        }
        files_here += 1;
        bytes_here += alloc;
        if e.kind == Kind::File && alloc >= BIG_FILE {
            d.big.push(SFile { name: e.name, size: alloc, mtime: e.mtime });
        } else {
            d.own += alloc;
            d.own_count += 1;
            d.own_newest = d.own_newest.max(e.mtime);
        }
        d.newest = d.newest.max(e.mtime);
    }
    ctx.prog.files.fetch_add(files_here, Relaxed);
    ctx.prog.bytes.fetch_add(bytes_here, Relaxed);

    d.dirs = subdirs
        .into_par_iter()
        .map(|(p, n)| scan_dir(&p, n, ctx))
        .collect();

    d.size = bytes_here;
    d.files = files_here;
    for s in &d.dirs {
        d.size += s.size;
        d.files += s.files;
        d.newest = d.newest.max(s.newest);
    }
    d
}

fn flatten(sd: SDir, parent: u32, nodes: &mut Vec<Node>) -> u32 {
    let id = nodes.len() as u32;
    nodes.push(Node {
        name: sd.name,
        parent,
        children: Vec::new(),
        size: sd.size,
        own: sd.own,
        own_count: sd.own_count,
        files: sd.files,
        newest: sd.newest,
        own_newest: sd.own_newest,
        flags: sd.flags,
        cat: 0,
        cats: [0; NCAT],
    });
    let mut kids = Vec::with_capacity(sd.big.len() + sd.dirs.len());
    for f in sd.big {
        kids.push(nodes.len() as u32);
        nodes.push(Node {
            name: f.name,
            parent: id,
            children: Vec::new(),
            size: f.size,
            own: f.size,
            own_count: 1,
            files: 1,
            newest: f.mtime,
            own_newest: f.mtime,
            flags: flag::FILE,
            cat: 0,
            cats: [0; NCAT],
        });
    }
    for d in sd.dirs {
        kids.push(flatten(d, id, nodes));
    }
    kids.sort_by(|&a, &b| nodes[b as usize].size.cmp(&nodes[a as usize].size));
    nodes[id as usize].children = kids;
    id
}

/// Measure one path without keeping its structure: (bytes, files).
pub fn measure(path: &Path) -> (u64, u64) {
    let (bytes, files, _) = measure_newest(path);
    (bytes, files)
}

/// Like `measure`, plus the newest modification time inside (unix seconds).
pub fn measure_newest(path: &Path) -> (u64, u64, i64) {
    forbid_cloud_downloads();
    let Ok(md) = fs::symlink_metadata(path) else { return (0, 0, 0) };
    if !md.is_dir() {
        #[cfg(unix)]
        let bytes = std::os::unix::fs::MetadataExt::blocks(&md) * 512;
        #[cfg(windows)]
        let bytes = md.len().div_ceil(4096) * 4096;
        let mtime = md.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64);
        return (bytes, 1, mtime);
    }
    let prog = Progress::default();
    let ctx = Ctx { prog: &prog, seen_links: Seen::new(), mounts: foreign_mounts() };
    let d = scan_dir(path, "".into(), &ctx);
    (d.size, d.files, d.newest)
}

/// The per-user folder macOS keeps under /private/var/folders (holds T = temp, C = cache).
#[cfg(not(target_os = "macos"))]
pub fn user_var_dir() -> Option<PathBuf> {
    None
}

#[cfg(target_os = "macos")]
pub fn user_var_dir() -> Option<PathBuf> {
    const CS_DARWIN_USER_TEMP_DIR: libc::c_int = 65537;
    let mut buf = vec![0u8; 1024];
    let n = unsafe { libc::confstr(CS_DARWIN_USER_TEMP_DIR, buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    if n == 0 || n > buf.len() {
        return None;
    }
    let s = String::from_utf8_lossy(&buf[..n - 1]).into_owned();
    let t = fs::canonicalize(s.trim_end_matches('/')).ok()?;
    t.parent().map(Path::to_path_buf)
}

/// Top of the whole-computer view: `/` on macOS, the system drive on Windows.
pub fn system_root() -> PathBuf {
    #[cfg(windows)]
    {
        let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        PathBuf::from(format!("{drive}\\"))
    }
    #[cfg(not(windows))]
    PathBuf::from("/")
}

/// Folders scanned in whole-computer mode, besides home.
fn system_roots(home: &Path) -> Vec<PathBuf> {
    #[cfg(windows)]
    let mut roots: Vec<PathBuf> = ["ProgramFiles", "ProgramFiles(x86)", "ProgramData", "SystemRoot"]
        .iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .collect();
    #[cfg(not(windows))]
    let mut roots: Vec<PathBuf> = ["/Applications", "/Library", "/opt/homebrew", "/usr/local"]
        .iter()
        .map(PathBuf::from)
        .collect();
    roots.extend(user_var_dir());
    // Leftovers from Windows upgrades sit at the drive root.
    #[cfg(windows)]
    roots.extend(
        ["Windows.old", "$Windows.~BT", "$Windows.~WS", "$WinREAgent"].iter().map(|n| system_root().join(n)),
    );
    roots.sort();
    roots.dedup();
    roots.retain(|r| r.is_dir() && !r.starts_with(home) && !home.starts_with(r));
    roots
}

/// Name of a scanned root below the top: its path components joined by `/`
/// (e.g. "Users/me", "Program Files (x86)").
pub fn root_name(top: &Path, r: &Path) -> String {
    let rel = r.strip_prefix(top).unwrap_or(r);
    rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}

fn sum(name: Box<str>, dirs: Vec<SDir>) -> SDir {
    let mut d = SDir { name, ..Default::default() };
    for s in &dirs {
        d.size += s.size;
        d.files += s.files;
        d.newest = d.newest.max(s.newest);
    }
    d.dirs = dirs;
    d
}

/// Scan `single` if given, otherwise the whole Mac: home plus the system
/// folders that hold apps, developer tools and Homebrew.
pub fn scan(single: Option<&Path>, home: &Path, prog: &Progress) -> std::io::Result<Tree> {
    forbid_cloud_downloads();
    let ctx = Ctx { prog, seen_links: Seen::new(), mounts: foreign_mounts() };
    // Deep trees (nested node_modules) recurse a lot; give workers room.
    // The walk waits on the file system more than on the CPU, so use more
    // threads than cores to keep the SSD busy.
    let threads = std::env::var("PURI_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or(32);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .stack_size(32 * 1024 * 1024)
        .build()
        .expect("thread pool");

    let (root, sd) = match single {
        Some(root) => {
            fs::symlink_metadata(root)?;
            let name: Box<str> = if root == home { "~".into() } else { root.display().to_string().into() };
            (root.to_path_buf(), pool.install(|| scan_dir(root, name, &ctx)))
        }
        None => {
            fs::symlink_metadata(home)?;
            let top = system_root();
            let mut roots = vec![home.to_path_buf()];
            roots.extend(system_roots(home));
            let dirs: Vec<SDir> = pool.install(|| {
                roots
                    .par_iter()
                    .map(|r| {
                        scan_dir(r, root_name(&top, r).into(), &ctx)
                    })
                    .collect()
            });
            let name: Box<str> = top.display().to_string().into();
            (top, sum(name, dirs))
        }
    };

    let mut nodes = Vec::with_capacity(prog.dirs.load(Relaxed) as usize + 1024);
    flatten(sd, NONE, &mut nodes);
    let mut tree = Tree { root, home: home.to_path_buf(), home_id: 0, nodes, unreadable: 0 };
    if single.is_none() {
        let home_name = root_name(&tree.root, home);
        tree.home_id = tree.child(0, &home_name).unwrap_or(0);
    }
    tree.unreadable = (0..tree.nodes.len() as u32)
        .filter(|&i| tree.node(i).flags & flag::UNREADABLE != 0 && tree.is_under(i, tree.home_id))
        .count() as u64;
    crate::category::classify(&mut tree);
    Ok(tree)
}
