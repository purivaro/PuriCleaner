//! Installed apps, the files each one leaves behind, and removing an app
//! together with them.
//!
//! macOS: an app is its .app bundle. Most apps name their files in ~/Library
//! after their bundle id (com.example.App), so those are matched exactly; a
//! folder that only shares the app's name is listed but left for the user to tick.
//! Windows: an app is its entry in Settings › Installed apps. Its own
//! uninstaller runs first, then the folders it left behind can go.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};
use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;
use serde_json::{json, Value};

use crate::clean::{self, Mode};

#[derive(Clone, Serialize)]
pub struct Leftover {
    pub path: String,
    pub size: u64,
    /// Named exactly after the app's id, so it is selected by default.
    /// Otherwise it only shares the app's name and the user decides.
    pub sure: bool,
    /// In a system folder: removed as administrator (permanently).
    pub admin: bool,
    /// support, cache, container, prefs, state, logs, web, launch, helper, folder,
    /// or install: the app's own folder (Windows), already counted in the app's size.
    pub kind: &'static str,
    pub newest: i64,
}

#[derive(Clone, Serialize)]
pub struct InstalledApp {
    pub key: String,
    pub name: String,
    pub version: Option<String>,
    /// Bundle id (macOS) or publisher (Windows).
    pub id: Option<String>,
    /// The .app bundle (macOS) or install folder (Windows).
    pub path: Option<String>,
    pub size: u64,
    pub last_used: Option<i64>,
    pub installed: Option<i64>,
    pub running: bool,
    pub can_remove: bool,
    /// Why it cannot be removed ("self").
    pub locked: Option<&'static str>,
    /// Ships a system extension, which macOS keeps until it is turned off in System Settings.
    pub extension: bool,
    pub leftovers: Vec<Leftover>,
    pub leftover_size: u64,
    /// The uninstall command line (Windows).
    #[serde(skip)]
    #[cfg_attr(not(windows), allow(dead_code))]
    uninstall: Option<String>,
}

/// Files of an app that is no longer installed, grouped by the id they are named after.
#[derive(Clone, Serialize)]
pub struct Orphan {
    pub id: String,
    pub size: u64,
    pub newest: i64,
    pub items: Vec<Leftover>,
}

#[derive(Clone, Default, Serialize)]
pub struct Listing {
    pub apps: Vec<InstalledApp>,
    pub orphans: Vec<Orphan>,
}

#[derive(Default)]
pub struct Progress {
    done: AtomicUsize,
    total: AtomicUsize,
}

#[derive(Clone, Default, Serialize)]
pub struct Job {
    pub running: bool,
    pub finished: bool,
    pub name: String,
    /// quit, uninstaller, waiting, remove
    pub step: &'static str,
    /// Bytes moved to the Trash / Recycle Bin.
    pub moved: u64,
    /// Bytes deleted outright.
    pub freed: u64,
    pub removed_app: bool,
    /// The app did not close (it may be asking to save); nothing was removed.
    pub still_open: bool,
    pub cancelled: bool,
    pub errors: Vec<String>,
}

#[derive(Default)]
struct Inner {
    listing: Option<Listing>,
    loading: bool,
    job: Job,
    /// Bundle size by (path → (mtime, bytes)), so a refresh only re-measures what changed.
    sizes: HashMap<PathBuf, (i64, u64)>,
}

pub struct Apps {
    home: PathBuf,
    inner: Arc<Mutex<Inner>>,
    prog: Arc<Progress>,
    stop: Arc<AtomicBool>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg_attr(windows, allow(dead_code))]
fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

impl Apps {
    pub fn new(home: PathBuf) -> Self {
        Apps { home, inner: Arc::default(), prog: Arc::default(), stop: Arc::default() }
    }

    pub fn call(&self, cmd: &str, args: &Value) -> Result<Value, String> {
        match cmd {
            "apps" => {
                let refresh = args.get("refresh").and_then(Value::as_bool) == Some(true);
                let first = { lock(&self.inner).listing.is_none() };
                if refresh || first {
                    self.start_list(refresh);
                }
                Ok(self.status(args.get("brief").and_then(Value::as_bool) == Some(true)))
            }
            "uninstall" => self.start_uninstall(args),
            "uninstall_stop" => {
                self.stop.store(true, Relaxed);
                Ok(json!({}))
            }
            _ => Err(format!("unknown command: {cmd}")),
        }
    }

    fn status(&self, brief: bool) -> Value {
        let i = lock(&self.inner);
        let mut v = json!({
            "loading": i.loading,
            "done": self.prog.done.load(Relaxed),
            "total": self.prog.total.load(Relaxed),
            "job": i.job,
        });
        if !brief {
            if let Some(l) = &i.listing {
                v["apps"] = json!(l.apps);
                v["orphans"] = json!(l.orphans);
            }
        }
        v
    }

    fn start_list(&self, refresh: bool) {
        let sizes = {
            let mut i = lock(&self.inner);
            if i.loading || i.job.running {
                return;
            }
            i.loading = true;
            if refresh {
                i.sizes.clear();
            }
            std::mem::take(&mut i.sizes)
        };
        spawn_list(self.home.clone(), self.inner.clone(), self.prog.clone(), sizes);
    }

    fn start_uninstall(&self, args: &Value) -> Result<Value, String> {
        let key = args.get("key").and_then(Value::as_str);
        let want: HashSet<&str> =
            args.get("paths").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        let thai = args.get("lang").and_then(Value::as_str) != Some("en");
        let mut inner = lock(&self.inner);
        if inner.job.running || inner.loading {
            return Err("busy".into());
        }
        let listing = inner.listing.as_ref().ok_or("the app list is not ready yet")?;
        // Only paths this module found itself can be removed.
        let (app, items): (Option<InstalledApp>, Vec<Leftover>) = match key {
            Some(k) => {
                let a = listing.apps.iter().find(|a| a.key == k).ok_or("this app is no longer in the list")?;
                if !a.can_remove {
                    return Err("this app cannot be removed here".into());
                }
                (Some(a.clone()), a.leftovers.iter().filter(|l| want.contains(l.path.as_str())).cloned().collect())
            }
            None => (None, listing.orphans.iter().flat_map(|o| &o.items).filter(|l| want.contains(l.path.as_str())).cloned().collect()),
        };
        if app.is_none() && items.is_empty() {
            return Err("nothing selected".into());
        }
        inner.job = Job {
            running: true,
            name: app.as_ref().map(|a| a.name.clone()).unwrap_or_default(),
            step: "start",
            ..Default::default()
        };
        drop(inner);
        self.stop.store(false, Relaxed);
        let (inner, stop) = (self.inner.clone(), self.stop.clone());
        let key = key.map(String::from);
        std::thread::spawn(move || {
            run_job(app, items, thai, &inner, &stop);
            let mut i = lock(&inner);
            // Update the list in place: measuring every app again takes a while.
            let removed_app = i.job.removed_app;
            if let Some(l) = i.listing.as_mut() {
                if removed_app {
                    l.apps.retain(|a| Some(&a.key) != key.as_ref());
                }
                let there = |x: &Leftover| Path::new(&x.path).symlink_metadata().is_ok();
                for a in &mut l.apps {
                    a.leftovers.retain(there);
                    a.leftover_size = a.leftovers.iter().filter(|x| x.kind != "install").map(|x| x.size).sum();
                }
                for o in &mut l.orphans {
                    o.items.retain(there);
                    o.size = o.items.iter().map(|x| x.size).sum();
                }
                l.orphans.retain(|o| !o.items.is_empty());
            }
            i.job.running = false;
            i.job.finished = true;
        });
        Ok(json!({ "started": true }))
    }
}

fn spawn_list(home: PathBuf, inner: Arc<Mutex<Inner>>, prog: Arc<Progress>, mut sizes: HashMap<PathBuf, (i64, u64)>) {
    lock(&inner).loading = true;
    prog.done.store(0, Relaxed);
    prog.total.store(0, Relaxed);
    std::thread::spawn(move || {
        let listing = platform::list(&home, &prog, &mut sizes);
        let mut i = lock(&inner);
        i.listing = Some(listing);
        i.sizes = sizes;
        i.loading = false;
    });
}

fn run_job(app: Option<InstalledApp>, items: Vec<Leftover>, thai: bool, inner: &Mutex<Inner>, stop: &AtomicBool) {
    let set = |f: &dyn Fn(&mut Job)| f(&mut lock(inner).job);
    if let Some(a) = &app {
        // The app itself goes first: if that fails, its settings are left alone.
        match platform::remove_app(a, &|step| set(&|j| j.step = step), stop) {
            Ok(()) => set(&|j| {
                j.removed_app = true;
                if cfg!(windows) {
                    j.freed += a.size;
                } else {
                    j.moved += a.size;
                }
            }),
            Err(e) => {
                set(&|j| match e.as_str() {
                    "still_open" => j.still_open = true,
                    "cancelled" | "stopped" => j.cancelled = true,
                    _ => j.errors.push(e.clone()),
                });
                return;
            }
        }
    }
    set(&|j| j.step = "remove");
    let (admin, user): (Vec<&Leftover>, Vec<&Leftover>) = items.iter().partition(|l| l.admin);
    for l in user {
        let p = Path::new(&l.path);
        if p.symlink_metadata().is_err() {
            continue; // the uninstaller already took it
        }
        platform::before_trash(l);
        match clean::remove(p, Mode::Trash) {
            Ok(()) => set(&|j| j.moved += l.size),
            Err(e) => set(&|j| j.errors.push(format!("{}: {e}", l.path))),
        }
    }
    let admin: Vec<&Leftover> = admin.into_iter().filter(|l| Path::new(&l.path).symlink_metadata().is_ok()).collect();
    if !admin.is_empty() {
        let paths: Vec<PathBuf> = admin.iter().map(|l| PathBuf::from(&l.path)).collect();
        let result = platform::remove_admin(&paths, thai);
        for l in admin {
            if Path::new(&l.path).symlink_metadata().is_err() {
                set(&|j| j.freed += l.size);
            } else if let Err(e) = &result {
                set(&|j| j.errors.push(format!("{}: {e}", l.path)));
            }
        }
    }
}

/* ─── Matching (pure, tested on every system) ─────────────────── */

/// The id a ~/Library entry is named after: "com.foo.App.plist" → "com.foo.App",
/// "group.com.foo.App" → "com.foo.App", "ABCDE12345.com.foo.App" → "com.foo.App".
#[cfg_attr(windows, allow(dead_code))]
fn entry_id(name: &str) -> &str {
    let mut s = name;
    for suffix in [".plist", ".savedState", ".binarycookies"] {
        if let Some(x) = s.strip_suffix(suffix) {
            s = x;
            break;
        }
    }
    if let Some(x) = s.strip_prefix("group.") {
        return x;
    }
    // Team-id prefix: ten upper-case letters or digits.
    let b = s.as_bytes();
    if b.len() > 11 && b[10] == b'.' && b[..10].iter().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
        return &s[11..];
    }
    s
}

/// Reverse-DNS ids ("com.example.App"): three or more parts, the first a short lower-case domain.
#[cfg_attr(windows, allow(dead_code))]
fn looks_like_id(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() >= 3
        && (2..=4).contains(&parts[0].len())
        && parts[0].bytes().all(|b| b.is_ascii_lowercase())
        && parts.iter().all(|p| !p.is_empty() && !p.contains(' '))
}

/// "com.foo.App" → "com.foo".
#[cfg_attr(windows, allow(dead_code))]
fn vendor(id: &str) -> String {
    id.to_ascii_lowercase().splitn(3, '.').take(2).collect::<Vec<_>>().join(".")
}

/// The installed app an id belongs to: the app's own id, or something under it
/// ("com.foo.App.helper"). The longest id wins, and matching never stops at a
/// vendor ("com.foo"), which would claim every app that vendor makes.
#[cfg_attr(windows, allow(dead_code))]
fn owner(id: &str, ids: &HashMap<String, usize>) -> Option<usize> {
    let lower = id.to_ascii_lowercase();
    let mut key = lower.as_str();
    loop {
        if let Some(&i) = ids.get(key) {
            return Some(i);
        }
        key = &key[..key.rfind('.')?];
        if key.matches('.').count() < 2 {
            return None;
        }
    }
}

/// Split a Windows command line into program and arguments.
#[cfg_attr(not(windows), allow(dead_code))]
fn split_command(cmd: &str) -> (String, String) {
    let cmd = cmd.trim();
    if let Some(rest) = cmd.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            return (rest[..end].to_string(), rest[end + 1..].trim().to_string());
        }
    }
    // Unquoted: the program ends at ".exe" (its path may contain spaces).
    if let Some(i) = cmd.to_ascii_lowercase().find(".exe") {
        return (cmd[..i + 4].to_string(), cmd[i + 4..].trim().to_string());
    }
    match cmd.split_once(' ') {
        Some((a, b)) => (a.to_string(), b.trim().to_string()),
        None => (cmd.to_string(), String::new()),
    }
}

/// MsiExec "/I{GUID}" opens the repair screen; "/X{GUID}" uninstalls.
#[cfg_attr(not(windows), allow(dead_code))]
fn msi_uninstall_args(args: &str) -> String {
    args.split(' ')
        .map(|t| if (t.starts_with("/I") || t.starts_with("/i")) && t[2..].starts_with('{') { format!("/X{}", &t[2..]) } else { t.to_string() })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Folder names an app may use: "Mozilla Firefox (x64 en-US)" → "Mozilla Firefox",
/// and "Firefox" when the publisher is Mozilla.
#[cfg_attr(not(windows), allow(dead_code))]
fn name_variants(name: &str, publisher: Option<&str>) -> Vec<String> {
    let mut out = vec![name.trim().to_string()];
    let base = name.split('(').next().unwrap_or(name);
    let words: Vec<&str> = base.split_whitespace().collect();
    let mut keep = words.len();
    while keep > 1 {
        let w = words[keep - 1].to_ascii_lowercase();
        if w.starts_with(|c: char| c.is_ascii_digit()) || ["x64", "x86", "64-bit", "32-bit", "-"].contains(&w.as_str()) {
            keep -= 1;
        } else {
            break;
        }
    }
    let base = words[..keep].join(" ");
    if let Some(first) = publisher.and_then(|p| p.split_whitespace().next()) {
        if let Some(rest) = base.strip_prefix(first) {
            out.push(rest.trim().to_string());
        }
    }
    out.push(base);
    let mut seen = HashSet::new();
    out.retain(|s| s.chars().count() >= 3 && seen.insert(s.to_lowercase()));
    out
}

/// Lower-case, no trailing separator, for comparing Windows paths.
#[cfg_attr(not(windows), allow(dead_code))]
fn norm(p: &str) -> String {
    p.trim().trim_matches('"').trim_end_matches(['\\', '/']).to_lowercase()
}

/// An install folder that is the app's alone: deep enough, not a shared root
/// ("C:\Program Files") or above one, and not shared with another app.
#[cfg_attr(not(windows), allow(dead_code))]
fn own_folder(loc: &str, shared: &[String], others: &[String]) -> bool {
    let loc = norm(loc);
    let under = |a: &str, b: &str| a == b || a.starts_with(&format!("{b}\\"));
    loc.matches('\\').count() >= 2
        && !shared.iter().any(|s| under(s, &loc))
        && !others.iter().any(|o| under(o, &loc) || under(&loc, o))
}

/* ─── macOS ───────────────────────────────────────────────────── */

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use rayon::prelude::*;
    use std::fs;
    use std::os::macos::fs::MetadataExt;
    use std::process::Command;
    use std::time::Duration;

    /// Set on apps that are part of macOS (Safari): protected by the system.
    const SF_RESTRICTED: u32 = 0x0008_0000;
    /// Leftovers of removed apps: listed when untouched this long, and at least this big.
    const ORPHAN_DAYS: i64 = 30;
    const ORPHAN_MIN: u64 = 1_000_000;

    /// Folders in ~/Library where apps keep their files.
    const USER_PLACES: &[(&str, &str)] = &[
        ("Application Support", "support"),
        ("Application Scripts", "support"),
        ("Caches", "cache"),
        ("HTTPStorages", "web"),
        ("WebKit", "web"),
        ("Cookies", "web"),
        ("Containers", "container"),
        ("Group Containers", "container"),
        ("Preferences", "prefs"),
        ("Preferences/ByHost", "prefs"),
        ("Saved Application State", "state"),
        ("Logs", "logs"),
        ("LaunchAgents", "launch"),
    ];
    const SYSTEM_PLACES: &[(&str, &str)] = &[
        ("/Library/Application Support", "support"),
        ("/Library/Caches", "cache"),
        ("/Library/Preferences", "prefs"),
        ("/Library/Logs", "logs"),
        ("/Library/LaunchAgents", "launch"),
        ("/Library/LaunchDaemons", "launch"),
        ("/Library/PrivilegedHelperTools", "helper"),
    ];
    /// Where a folder named after the app (not its id) is worth showing.
    const NAME_KINDS: &[&str] = &["support", "cache", "logs"];
    /// Owner index for an id that belongs to an app installed somewhere else.
    const ELSEWHERE: usize = usize::MAX;

    struct Info {
        id: Option<String>,
        version: Option<String>,
        short_name: Option<String>,
    }

    fn read_info(bundle: &Path) -> Info {
        let v: Value = Command::new("/usr/bin/plutil")
            .args(["-convert", "json", "-o", "-"])
            .arg(bundle.join("Contents/Info.plist"))
            .output()
            .ok()
            .and_then(|o| serde_json::from_slice(&o.stdout).ok())
            .unwrap_or_default();
        let s = |k: &str| v[k].as_str().map(str::trim).filter(|s| !s.is_empty()).map(String::from);
        Info { id: s("CFBundleIdentifier"), version: s("CFBundleShortVersionString"), short_name: s("CFBundleName") }
    }

    /// .app bundles in /Applications and ~/Applications, and one folder deeper.
    fn bundles(home: &Path) -> Vec<PathBuf> {
        let is_app = |p: &Path| p.extension().is_some_and(|x| x == "app") && fs::symlink_metadata(p).is_ok_and(|m| m.is_dir());
        let mut out = Vec::new();
        for root in [PathBuf::from("/Applications"), home.join("Applications")] {
            let Ok(rd) = fs::read_dir(&root) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                if is_app(&p) {
                    out.push(p);
                } else if fs::symlink_metadata(&p).is_ok_and(|m| m.is_dir()) {
                    if let Ok(inner) = fs::read_dir(&p) {
                        out.extend(inner.flatten().map(|e| e.path()).filter(|p| is_app(p)));
                    }
                }
            }
        }
        out.retain(|p| fs::symlink_metadata(p).is_ok_and(|m| m.st_flags() & SF_RESTRICTED == 0));
        out
    }

    /// Bundle ids of every app Spotlight knows, wherever it is installed,
    /// and whether it is a standalone app (not a helper inside another app).
    fn known_ids() -> Vec<(String, bool)> {
        let Ok(out) = Command::new("/usr/bin/mdfind")
            .args(["-attr", "kMDItemCFBundleIdentifier", "kMDItemContentType == \"com.apple.application-bundle\""])
            .output()
        else {
            return Vec::new();
        };
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| l.rsplit_once("kMDItemCFBundleIdentifier = "))
            .map(|(path, id)| (id.trim().to_ascii_lowercase(), !path.trim_end().contains(".app/")))
            .filter(|(id, _)| id != "(null)" && !id.is_empty())
            .collect()
    }

    fn running_exes() -> Vec<(u32, PathBuf)> {
        use ::sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
        let mut sys = System::new();
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_exe(UpdateKind::Always));
        sys.processes().iter().filter_map(|(pid, p)| p.exe().map(|e| (pid.as_u32(), e.to_path_buf()))).collect()
    }

    struct Cand {
        path: PathBuf,
        name: String,
        kind: &'static str,
        admin: bool,
    }

    fn candidates(lib: &Path) -> Vec<Cand> {
        let places = USER_PLACES
            .iter()
            .map(|&(d, k)| (lib.join(d), k, false))
            .chain(SYSTEM_PLACES.iter().map(|&(d, k)| (PathBuf::from(d), k, true)));
        let mut out = Vec::new();
        for (dir, kind, admin) in places {
            let Ok(rd) = fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') || name == "ByHost" {
                    continue;
                }
                out.push(Cand { path: e.path(), name, kind, admin });
            }
        }
        out
    }

    pub fn list(home: &Path, prog: &Progress, sizes: &mut HashMap<PathBuf, (i64, u64)>) -> Listing {
        let paths = bundles(home);
        prog.total.store(paths.len(), Relaxed);
        let me = std::env::current_exe().ok().and_then(|e| crate::memory::outer_bundle(&e));
        let running: HashSet<PathBuf> = running_exes().into_iter().filter_map(|(_, e)| crate::memory::outer_bundle(&e)).collect();
        let last_used = crate::sysinfo::app_last_used(&paths);

        let cached = &*sizes;
        let measured: Vec<(InstalledApp, Option<String>, i64)> = paths
            .par_iter()
            .map(|p| {
                let info = read_info(p);
                let mtime = fs::symlink_metadata(p).map_or(0, |m| m.st_mtime());
                let size = match cached.get(p) {
                    Some(&(t, s)) if t == mtime => s,
                    _ => crate::scan::measure(p).0,
                };
                prog.done.fetch_add(1, Relaxed);
                let is_me = me.as_deref() == Some(p.as_path());
                let ext = fs::read_dir(p.join("Contents/Library/SystemExtensions")).is_ok_and(|mut d| d.next().is_some());
                let app = InstalledApp {
                    key: p.to_string_lossy().into_owned(),
                    name: p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                    version: info.version,
                    id: info.id,
                    path: Some(p.to_string_lossy().into_owned()),
                    size,
                    last_used: last_used.get(p).copied().filter(|&t| t > 0),
                    installed: None,
                    running: running.contains(p),
                    can_remove: !is_me,
                    locked: is_me.then_some("self"),
                    extension: ext,
                    leftovers: Vec::new(),
                    leftover_size: 0,
                    uninstall: None,
                };
                (app, info.short_name, mtime)
            })
            .collect();
        let mut apps = Vec::with_capacity(measured.len());
        let mut names: HashMap<String, Option<usize>> = HashMap::new();
        for (i, (app, short, mtime)) in measured.into_iter().enumerate() {
            sizes.insert(PathBuf::from(&app.key), (mtime, app.size));
            for n in [Some(&app.name), short.as_ref()].into_iter().flatten() {
                let n = n.to_lowercase();
                if n.chars().count() >= 3 {
                    // A name two apps share is no clue at all.
                    let e = names.entry(n).or_insert(Some(i));
                    if *e != Some(i) {
                        *e = None;
                    }
                }
            }
            apps.push(app);
        }

        // Apps installed elsewhere count too, so "com.google.chrome.for.testing"
        // (its own app) is not taken for part of "com.google.Chrome".
        let known = known_ids();
        let mut ids: HashMap<String, usize> = known.iter().filter(|(_, alone)| *alone).map(|(id, _)| (id.clone(), ELSEWHERE)).collect();
        ids.extend(apps.iter().enumerate().filter_map(|(i, a)| a.id.as_ref().map(|id| (id.to_ascii_lowercase(), i))));
        let mut vendors: HashSet<String> = known.iter().map(|(id, _)| id).chain(ids.keys()).map(|id| vendor(id)).collect();
        vendors.insert("com.apple".into());

        let mut matched: Vec<(usize, bool, Cand)> = Vec::new();
        let mut orphans: Vec<(String, Cand)> = Vec::new();
        for c in candidates(&home.join("Library")) {
            let id = entry_id(&c.name);
            if let Some(i) = owner(id, &ids) {
                if i != ELSEWHERE {
                    // An app's own parts keep its exact spelling ("com.google.Chrome.helper");
                    // "com.google.chrome.for.testing" is someone else's, so only shown.
                    let exact = apps[i].id.as_deref().is_some_and(|o| id == o || id.starts_with(&format!("{o}.")));
                    matched.push((i, exact, c));
                }
            } else if let Some(Some(i)) = NAME_KINDS.contains(&c.kind).then(|| names.get(&c.name.to_lowercase()).copied()).flatten() {
                matched.push((i, false, c));
            } else if !c.admin && looks_like_id(id) && !vendors.contains(&vendor(id)) {
                orphans.push((id.to_string(), c));
            }
        }

        // "Application Support/Google/Chrome" for "Google Chrome".
        for (dir, kind) in [("Application Support", "support"), ("Caches", "cache")] {
            for (i, a) in apps.iter().enumerate() {
                if let Some((maker, rest)) = a.name.split_once(' ') {
                    let path = home.join("Library").join(dir).join(maker).join(rest);
                    if fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir()) {
                        matched.push((i, false, Cand { path, name: rest.into(), kind, admin: false }));
                    }
                }
            }
        }

        prog.total.fetch_add(matched.len() + orphans.len(), Relaxed);
        let measure = |c: &Cand| {
            let (size, _, newest) = crate::scan::measure_newest(&c.path);
            prog.done.fetch_add(1, Relaxed);
            Leftover { path: c.path.to_string_lossy().into_owned(), size, sure: false, admin: c.admin, kind: c.kind, newest }
        };
        let left: Vec<Leftover> = matched.par_iter().map(|(_, _, c)| measure(c)).collect();
        for ((i, sure, _), mut l) in matched.iter().zip(left) {
            l.sure = *sure;
            apps[*i].leftover_size += l.size;
            apps[*i].leftovers.push(l);
        }
        for a in &mut apps {
            a.leftovers.sort_by(|x, y| y.sure.cmp(&x.sure).then(y.size.cmp(&x.size)));
        }

        let found: Vec<Leftover> = orphans.par_iter().map(|(_, c)| measure(c)).collect();
        let mut groups: HashMap<String, Orphan> = HashMap::new();
        for ((id, _), mut l) in orphans.into_iter().zip(found) {
            l.sure = true;
            let g = groups.entry(id.to_ascii_lowercase()).or_insert_with(|| Orphan { id, size: 0, newest: 0, items: Vec::new() });
            g.size += l.size;
            g.newest = g.newest.max(l.newest);
            g.items.push(l);
        }
        let stale = now() - ORPHAN_DAYS * 86400;
        let mut orphans: Vec<Orphan> = groups.into_values().filter(|g| g.size >= ORPHAN_MIN && g.newest < stale).collect();
        orphans.sort_by(|a, b| b.size.cmp(&a.size));
        Listing { apps, orphans }
    }

    /// Ask the app to quit (as if from the Dock) and wait. An app with unsaved
    /// work asks first and stays open; then nothing is removed.
    fn quit(bundle: &Path) -> Result<(), String> {
        let inside = |e: &Path| e.starts_with(bundle);
        if !running_exes().iter().any(|(_, e)| inside(e)) {
            return Ok(());
        }
        let p = serde_json::to_string(&bundle.to_string_lossy()).map_err(|e| e.to_string())?;
        let js = format!(
            "ObjC.import('AppKit'); var P = {p}, a = $.NSWorkspace.sharedWorkspace.runningApplications; \
             for (var i = 0; i < a.count; i++) {{ var x = a.objectAtIndex(i), u = x.bundleURL; \
               if (u && !u.isNil()) {{ var s = u.path.js; if (s === P || s.indexOf(P + '/') === 0) x.terminate; }} }}"
        );
        let _ = Command::new("/usr/bin/osascript").args(["-l", "JavaScript", "-e", &js]).output();
        for _ in 0..40 {
            std::thread::sleep(Duration::from_millis(250));
            if !running_exes().iter().any(|(_, e)| inside(e)) {
                return Ok(());
            }
        }
        let left: Vec<(u32, PathBuf)> = running_exes().into_iter().filter(|(_, e)| inside(e)).collect();
        if left.iter().any(|(_, e)| e.starts_with(bundle.join("Contents/MacOS"))) {
            return Err("still_open".into());
        }
        // Only helpers with no window are left (updaters, crash reporters): stop them.
        for (pid, _) in left {
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGTERM);
            }
        }
        std::thread::sleep(Duration::from_secs(1));
        Ok(())
    }

    pub fn remove_app(a: &InstalledApp, step: &dyn Fn(&'static str), _stop: &AtomicBool) -> Result<(), String> {
        let bundle = PathBuf::from(a.path.as_deref().ok_or("no path")?);
        step("quit");
        quit(&bundle)?;
        step("remove");
        // Finder asks for the admin password when the app belongs to the system (App Store apps).
        clean::remove(&bundle, Mode::Trash).or_else(|_| clean::trash_with_finder(&bundle)).map_err(|e| {
            if e.contains("-128") || e.to_lowercase().contains("cancel") {
                "cancelled".into()
            } else {
                e
            }
        })
    }

    /// Unload a login agent before its file goes, so it stops now rather than at the next restart.
    pub fn before_trash(l: &Leftover) {
        if l.kind == "launch" && l.path.ends_with(".plist") {
            let uid = unsafe { libc::getuid() };
            let _ = Command::new("/bin/launchctl").args(["bootout", &format!("gui/{uid}"), &l.path]).output();
        }
    }

    pub fn remove_admin(paths: &[PathBuf], thai: bool) -> Result<(), String> {
        use crate::api::sh_quote;
        let mut cmd = String::new();
        for p in paths.iter().filter(|p| p.starts_with("/Library/LaunchDaemons")) {
            cmd += &format!("/bin/launchctl bootout system {} 2>/dev/null; ", sh_quote(&p.to_string_lossy()));
        }
        cmd += "/bin/rm -rf --";
        for p in paths {
            cmd += " ";
            cmd += &sh_quote(&p.to_string_lossy());
        }
        crate::api::as_root(&cmd, thai)
    }
}

/* ─── Windows ─────────────────────────────────────────────────── */

#[cfg(windows)]
mod platform {
    use super::*;
    use rayon::prelude::*;
    use std::fs;
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    use std::time::Duration;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// Every entry of Settings › Installed apps, as JSON.
    const LIST_PS: &str = r#"
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$roots = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall',
         'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall',
         'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall'
$out = foreach ($r in $roots) {
  Get-ChildItem $r -ErrorAction SilentlyContinue | ForEach-Object {
    $p = Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue
    if ($p.DisplayName -and $p.UninstallString) {
      [pscustomobject]@{ key = $_.Name; name = $p.DisplayName; version = $p.DisplayVersion; publisher = $p.Publisher;
        location = $p.InstallLocation; uninstall = $p.UninstallString; size = $p.EstimatedSize; date = $p.InstallDate;
        system = $p.SystemComponent; parent = $p.ParentKeyName; release = $p.ReleaseType }
    }
  }
}
ConvertTo-Json -InputObject @($out) -Compress
"#;

    fn ps(script: &str) -> Result<String, String> {
        let out = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(if msg.contains("canceled") || msg.contains("cancelled") { "cancelled".into() } else { msg })
        }
    }

    fn env(k: &str) -> Option<String> {
        std::env::var(k).ok().filter(|v| !v.is_empty())
    }

    fn running_exes() -> Vec<PathBuf> {
        use ::sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
        let mut sys = System::new();
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_exe(UpdateKind::Always));
        sys.processes().values().filter_map(|p| p.exe().map(Path::to_path_buf)).collect()
    }

    /// "20240115" → unix seconds.
    fn install_date(s: &str) -> Option<i64> {
        let s = s.trim();
        if s.len() != 8 || !s.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let n = |a: usize, b: usize| s[a..b].parse::<i64>().ok();
        Some(crate::sysinfo::days_from_civil(n(0, 4)?, n(4, 6)?, n(6, 8)?) * 86400)
    }

    fn text(v: &Value, k: &str) -> Option<String> {
        match &v[k] {
            Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        }
    }

    pub fn list(home: &Path, prog: &Progress, _sizes: &mut HashMap<PathBuf, (i64, u64)>) -> Listing {
        let rows: Vec<Value> = ps(LIST_PS).ok().and_then(|s| serde_json::from_str(s.trim()).ok()).unwrap_or_default();
        let me = std::env::current_exe().ok().map(|p| norm(&p.to_string_lossy()));
        let running: Vec<String> = running_exes().iter().map(|p| norm(&p.to_string_lossy())).collect();

        let mut seen = HashSet::new();
        let mut apps: Vec<InstalledApp> = Vec::new();
        for r in &rows {
            // Hidden parts of other products, and Windows updates.
            if r["system"].as_i64() == Some(1) || text(r, "parent").is_some() {
                continue;
            }
            if text(r, "release").is_some_and(|t| t.contains("Update") || t.contains("Hotfix")) {
                continue;
            }
            let (Some(name), Some(key)) = (text(r, "name"), text(r, "key")) else { continue };
            let version = text(r, "version");
            if !seen.insert((name.to_lowercase(), version.clone())) {
                continue;
            }
            let location = text(r, "location").map(|l| l.trim_matches('"').trim_end_matches('\\').to_string());
            let loc_n = location.as_deref().map(norm);
            let inside = |e: &String| loc_n.as_ref().is_some_and(|l| l.matches('\\').count() >= 2 && e.starts_with(&format!("{l}\\")));
            let is_me = me.as_ref().is_some_and(inside);
            apps.push(InstalledApp {
                key,
                name,
                version,
                id: text(r, "publisher"),
                path: location,
                size: r["size"].as_u64().unwrap_or(0) * 1024,
                last_used: None,
                installed: text(r, "date").and_then(|d| install_date(&d)),
                running: running.iter().any(inside),
                can_remove: !is_me,
                locked: is_me.then_some("self"),
                extension: false,
                leftovers: Vec::new(),
                leftover_size: 0,
                uninstall: text(r, "uninstall"),
            });
        }
        prog.total.store(apps.len(), Relaxed);

        let shared: Vec<String> = ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432", "ProgramData", "APPDATA", "LOCALAPPDATA", "SystemRoot", "SystemDrive", "CommonProgramFiles", "CommonProgramFiles(x86)"]
            .iter()
            .filter_map(|k| env(k))
            .chain(env("LOCALAPPDATA").map(|l| format!("{l}\\Programs")))
            .chain([home.to_string_lossy().into_owned()])
            .map(|s| norm(&s))
            .collect();
        let admin_roots: Vec<String> =
            ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432", "ProgramData"].iter().filter_map(|k| env(k)).map(|s| norm(&s)).collect();
        let is_admin = |p: &str| {
            let p = norm(p);
            admin_roots.iter().any(|r| p.starts_with(&format!("{r}\\")))
        };
        let locations: Vec<String> = apps.iter().filter_map(|a| a.path.as_deref()).map(norm).collect();

        // Folders under AppData and ProgramData, by lower-case name.
        let places: Vec<(PathBuf, &'static str)> = [
            (env("APPDATA"), "support"),
            (env("LOCALAPPDATA"), "support"),
            (env("LOCALAPPDATA").map(|l| format!("{l}\\Programs")), "folder"),
            (env("ProgramData"), "support"),
        ]
        .into_iter()
        .filter_map(|(d, k)| d.map(|d| (PathBuf::from(d), k)))
        .collect();
        let dirs = |d: &Path| -> HashMap<String, PathBuf> {
            fs::read_dir(d)
                .map(|rd| {
                    rd.flatten()
                        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                        .map(|e| (e.file_name().to_string_lossy().to_lowercase(), e.path()))
                        .collect()
                })
                .unwrap_or_default()
        };
        let place_dirs: Vec<(HashMap<String, PathBuf>, &'static str)> = places.iter().map(|(d, k)| (dirs(d), *k)).collect();

        let mut found: Vec<(usize, PathBuf, &'static str, bool)> = Vec::new();
        for (i, a) in apps.iter().enumerate() {
            if let Some(loc) = &a.path {
                let others: Vec<String> = locations.iter().filter(|l| **l != norm(loc)).cloned().collect();
                if Path::new(loc).is_dir() && own_folder(loc, &shared, &others) {
                    // Sure only when the folder is plainly named after the app or its maker.
                    let last = norm(loc).rsplit('\\').next().unwrap_or("").to_string();
                    let first_word = a.name.split_whitespace().next().unwrap_or("").to_lowercase();
                    let publisher = a.id.as_deref().and_then(|p| p.split_whitespace().next()).unwrap_or("").to_lowercase();
                    let sure = (!first_word.is_empty() && last.contains(&first_word)) || (publisher.len() >= 3 && last.contains(&publisher));
                    found.push((i, PathBuf::from(loc), "install", sure));
                }
            }
            let variants = name_variants(&a.name, a.id.as_deref());
            let publisher = a.id.as_deref().and_then(|p| p.split_whitespace().next()).map(str::to_lowercase);
            for (d, kind) in &place_dirs {
                for v in &variants {
                    if let Some(p) = d.get(&v.to_lowercase()) {
                        found.push((i, p.clone(), *kind, false));
                    }
                }
                // "<Publisher>\<App>" — the publisher folder itself is shared and never offered.
                if let Some(pd) = publisher.as_ref().filter(|p| p.len() >= 3).and_then(|p| d.get(p)) {
                    let inner = dirs(pd);
                    for v in &variants {
                        if let Some(p) = inner.get(&v.to_lowercase()) {
                            found.push((i, p.clone(), *kind, false));
                        }
                    }
                }
            }
        }
        // A folder two apps both match is no clue at all.
        let mut count: HashMap<String, HashSet<usize>> = HashMap::new();
        for (i, p, _, _) in &found {
            count.entry(norm(&p.to_string_lossy())).or_default().insert(*i);
        }
        found.retain(|(_, p, _, _)| count[&norm(&p.to_string_lossy())].len() == 1);
        found.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
        found.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);

        let left: Vec<Leftover> = found
            .par_iter()
            .map(|(_, p, kind, sure)| {
                let (size, _, newest) = crate::scan::measure_newest(p);
                let path = p.to_string_lossy().into_owned();
                Leftover { admin: is_admin(&path), path, size, sure: *sure, kind, newest }
            })
            .collect();
        for ((i, ..), l) in found.iter().zip(left) {
            let a = &mut apps[*i];
            // The registry's estimate is missing for some apps: measure the folder instead.
            if l.kind == "install" {
                if a.size == 0 {
                    a.size = l.size;
                }
            } else {
                a.leftover_size += l.size;
            }
            a.leftovers.push(l);
        }
        for a in &mut apps {
            a.leftovers.sort_by(|x, y| y.sure.cmp(&x.sure).then(y.size.cmp(&x.size)));
            prog.done.fetch_add(1, Relaxed);
        }
        Listing { apps, orphans: Vec::new() }
    }

    fn still_installed(key: &str) -> bool {
        ps(&format!("Test-Path -LiteralPath {}", crate::api::ps_quote(&format!("Registry::{key}")))).map_or(true, |s| s.trim() == "True")
    }

    /// Run the app's own uninstaller (it asks for permission itself when it needs it),
    /// then wait until the app has left the installed list.
    pub fn remove_app(a: &InstalledApp, step: &dyn Fn(&'static str), stop: &AtomicBool) -> Result<(), String> {
        use crate::api::ps_quote;
        let cmd = a.uninstall.as_deref().ok_or("this app has no uninstaller")?;
        let (exe, args) = split_command(cmd);
        let is_msi = Path::new(&exe).file_stem().is_some_and(|s| s.eq_ignore_ascii_case("msiexec"));
        let args = if is_msi { msi_uninstall_args(&args) } else { args };
        let list = if args.is_empty() { String::new() } else { format!(" -ArgumentList {}", ps_quote(&args)) };
        step("uninstaller");
        ps(&format!("$p = Start-Process -FilePath {}{list} -PassThru -Wait; exit 0", ps_quote(&exe)))?;
        // Some uninstallers hand over to a copy of themselves and return at once.
        step("waiting");
        while still_installed(&a.key) {
            if stop.load(Relaxed) {
                return Err("stopped".into());
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        Ok(())
    }

    pub fn before_trash(_l: &Leftover) {}

    pub fn remove_admin(paths: &[PathBuf], thai: bool) -> Result<(), String> {
        crate::api::admin_remove(paths, thai)
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod platform {
    use super::*;

    pub fn list(_home: &Path, _prog: &Progress, _sizes: &mut HashMap<PathBuf, (i64, u64)>) -> Listing {
        Listing::default()
    }
    pub fn remove_app(_a: &InstalledApp, _step: &dyn Fn(&'static str), _stop: &AtomicBool) -> Result<(), String> {
        Err("not supported on this system".into())
    }
    pub fn before_trash(_l: &Leftover) {}
    pub fn remove_admin(_paths: &[PathBuf], _thai: bool) -> Result<(), String> {
        Err("not supported on this system".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_id_a_library_entry_is_named_after() {
        assert_eq!(entry_id("com.foo.App.plist"), "com.foo.App");
        assert_eq!(entry_id("com.foo.App.savedState"), "com.foo.App");
        assert_eq!(entry_id("group.com.foo.App"), "com.foo.App");
        assert_eq!(entry_id("ABCDE12345.com.foo.App"), "com.foo.App");
        assert_eq!(entry_id("Visual Studio Code"), "Visual Studio Code");
        assert_eq!(entry_id("ไทย.plist"), "ไทย");
    }

    #[test]
    fn matches_an_app_and_its_parts_but_never_a_whole_vendor() {
        let ids: HashMap<String, usize> = [("com.foo.app".to_string(), 0), ("com.foo.app.pro".to_string(), 1), ("com.bar".to_string(), 2)].into();
        assert_eq!(owner("com.foo.App", &ids), Some(0));
        assert_eq!(owner("com.foo.App.helper", &ids), Some(0));
        // The more specific app wins.
        assert_eq!(owner("com.foo.App.Pro.cache", &ids), Some(1));
        // Another app of the same vendor is not this one.
        assert_eq!(owner("com.foo.Other", &ids), None);
        // A two-part id only matches itself.
        assert_eq!(owner("com.bar", &ids), Some(2));
        assert_eq!(owner("com.bar.thing", &ids), None);
        assert_eq!(owner("Visual Studio Code", &ids), None);
    }

    #[test]
    fn recognises_ids_and_vendors() {
        assert!(looks_like_id("com.example.App"));
        assert!(looks_like_id("org.videolan.vlc"));
        assert!(!looks_like_id("com.example"));
        assert!(!looks_like_id("homebrew.mxcl.postgresql"));
        assert!(!looks_like_id("Google Chrome"));
        assert!(!looks_like_id("CloudKit"));
        assert_eq!(vendor("com.Google.Chrome"), "com.google");
    }

    #[test]
    fn splits_windows_uninstall_commands() {
        assert_eq!(split_command(r#""C:\Program Files\Foo\unins000.exe" /SILENT"#), (r"C:\Program Files\Foo\unins000.exe".into(), "/SILENT".into()));
        assert_eq!(split_command(r"C:\Program Files\Foo\uninstall.exe --x"), (r"C:\Program Files\Foo\uninstall.exe".into(), "--x".into()));
        assert_eq!(split_command("MsiExec.exe /I{1234-ABCD}"), ("MsiExec.exe".into(), "/I{1234-ABCD}".into()));
        assert_eq!(msi_uninstall_args("/I{1234-ABCD} /quiet"), "/X{1234-ABCD} /quiet");
        assert_eq!(msi_uninstall_args("/X{1234}"), "/X{1234}");
        assert_eq!(msi_uninstall_args("/i"), "/i");
    }

    #[test]
    fn derives_folder_names_from_an_app_name() {
        assert_eq!(name_variants("Mozilla Firefox (x64 en-US)", Some("Mozilla")), vec!["Mozilla Firefox (x64 en-US)", "Firefox", "Mozilla Firefox"]);
        assert_eq!(name_variants("7-Zip 23.01 (x64)", Some("Igor Pavlov")), vec!["7-Zip 23.01 (x64)", "7-Zip"]);
        assert_eq!(name_variants("Go", None), Vec::<String>::new());
    }

    #[test]
    fn refuses_shared_or_shallow_install_folders() {
        let shared = vec![norm(r"C:\Program Files"), norm(r"C:\ProgramData"), norm(r"C:\Users\me\AppData\Local")];
        let others = vec![norm(r"C:\Program Files\Acme\Tool")];
        assert!(own_folder(r"C:\Program Files\Foo\", &shared, &others));
        assert!(!own_folder(r"C:\Program Files", &shared, &others));
        assert!(!own_folder(r"C:\", &shared, &others));
        assert!(!own_folder(r"C:\Users\me", &shared, &others), "above a shared folder");
        assert!(!own_folder(r"C:\Program Files\Acme", &shared, &others), "holds another app");
        assert!(!own_folder(r"C:\Program Files\Acme\Tool\Sub", &shared, &others), "inside another app");
    }

    #[test]
    fn only_removes_what_it_found() {
        let apps = Apps::new(PathBuf::from("/nonexistent"));
        let leftover = |p: &str| Leftover { path: p.into(), size: 1, sure: true, admin: false, kind: "cache", newest: 0 };
        {
            let mut i = lock(&apps.inner);
            i.listing = Some(Listing {
                apps: vec![InstalledApp {
                    key: "/Applications/Me.app".into(),
                    name: "Me".into(),
                    version: None,
                    id: None,
                    path: None,
                    size: 0,
                    last_used: None,
                    installed: None,
                    running: false,
                    can_remove: false,
                    locked: Some("self"),
                    extension: false,
                    leftovers: vec![leftover("/tmp/a")],
                    leftover_size: 1,
                    uninstall: None,
                }],
                orphans: vec![Orphan { id: "com.x.y".into(), size: 1, newest: 0, items: vec![leftover("/tmp/b")] }],
            });
        }
        let err = |args: Value| apps.call("uninstall", &args).unwrap_err();
        assert!(err(json!({ "key": "/Applications/Me.app", "paths": ["/tmp/a"] })).contains("cannot be removed"));
        assert!(err(json!({ "key": "/Applications/Other.app" })).contains("no longer"));
        // A path it never listed is ignored, leaving nothing to do.
        assert!(err(json!({ "paths": ["/Users/me/Documents"] })).contains("nothing selected"));
        assert!(!lock(&apps.inner).job.running);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn lists_this_macs_apps_without_system_ones() {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        let prog = Progress::default();
        let l = platform::list(&home, &prog, &mut HashMap::new());
        assert!(!l.apps.is_empty());
        assert!(l.apps.iter().all(|a| a.id.as_deref() != Some("com.apple.Safari")), "Safari is part of macOS");
        let lib = home.join("Library");
        for a in &l.apps {
            for x in &a.leftovers {
                let p = Path::new(&x.path);
                assert!(p.starts_with(&lib) || p.starts_with("/Library"), "{}", x.path);
                assert!(p.parent().is_some_and(|d| d != lib && d != Path::new("/Library")), "never a Library folder itself: {}", x.path);
            }
        }
        for o in &l.orphans {
            assert!(!o.id.to_lowercase().starts_with("com.apple."));
            assert!(o.items.iter().all(|x| !x.admin));
        }
    }
}
