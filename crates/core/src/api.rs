//! One JSON entry point (`App::call`) shared by the desktop app and the
//! browser server, so both front-ends speak exactly the same protocol.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use serde::Serialize;
use serde_json::{json, Value};

use crate::category::KEYS;
use crate::clean::{self, Mode, ScriptGroup};
use crate::rules::{self, Action, Item, Level, Recommendation, VIRTUAL};
use crate::scan::{self, Progress};
use crate::sysinfo::{self, SysInfo};
use crate::tree::Tree;

const TOP_TILES: usize = 48;
const INNER_TILES: usize = 18;

#[derive(Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
enum Phase {
    #[default]
    Idle,
    Scanning,
    Ready,
    Cleaning,
}

#[derive(Default)]
struct State {
    phase: Phase,
    tree: Option<Tree>,
    sys: SysInfo,
    recs: Vec<Recommendation>,
    items: HashMap<u32, (usize, Level)>,
    contains: HashMap<u32, [u64; 2]>,
    started: Option<Instant>,
    scan_ms: u64,
    error: Option<String>,
}

impl State {
    fn refresh(&mut self) {
        if let Some(tree) = &self.tree {
            self.recs = rules::recommend(tree, &self.sys);
            self.items = rules::item_index(&self.recs);
            self.contains = rules::containers(tree, &self.recs);
        }
    }

    fn item(&self, id: u32) -> Option<&Item> {
        let &(r, _) = self.items.get(&id)?;
        self.recs[r].items.iter().find(|it| it.id == id)
    }
}

#[derive(Default, Clone, Serialize)]
struct CleanStatus {
    running: bool,
    finished: bool,
    mode: &'static str,
    total: usize,
    done: usize,
    /// Bytes given back to the disk.
    freed: u64,
    /// Bytes moved to the Trash (freed only once it is emptied).
    moved: u64,
    current: String,
    errors: Vec<String>,
}

pub struct App {
    /// A single folder to scan, or None for the whole Mac.
    single: Option<PathBuf>,
    home: PathBuf,
    state: Arc<Mutex<State>>,
    prog: Arc<Progress>,
    clean: Arc<Mutex<CleanStatus>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn arg_u32(args: &Value, key: &str) -> Option<u32> {
    args.get(key).and_then(Value::as_u64).map(|v| v as u32)
}

fn arg_ids(args: &Value) -> Vec<u32> {
    args.get("items")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_u64).map(|v| v as u32).collect())
        .unwrap_or_default()
}

#[cfg(not(windows))]
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Start this same program again with administrator rights (UAC), then quit.
#[cfg(windows)]
fn relaunch_admin() -> Result<Value, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    let wide = |s: &std::ffi::OsStr| s.encode_wide().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let params = std::env::args()
        .skip(1)
        .map(|a| if a.contains(' ') { format!("\"{a}\"") } else { a })
        .collect::<Vec<_>>()
        .join(" ");
    let verb = wide("runas".as_ref());
    let (exe_w, params_w) = (wide(exe.as_os_str()), wide(params.as_ref()));
    const SW_SHOWNORMAL: i32 = 1;
    let r = unsafe {
        ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), exe_w.as_ptr(), params_w.as_ptr(), std::ptr::null(), SW_SHOWNORMAL)
    };
    // Values above 32 mean the new process started; otherwise UAC was declined or it failed.
    if r as isize <= 32 {
        return Err("cancelled".into());
    }
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(400));
        std::process::exit(0);
    });
    Ok(json!({ "relaunched": true }))
}

#[cfg(not(windows))]
fn relaunch_admin() -> Result<Value, String> {
    Err("only on Windows".into())
}

/// PowerShell single-quoted string.
#[cfg(windows)]
fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Delete paths as administrator, after Windows asks (UAC) once.
#[cfg(windows)]
fn admin_remove(paths: &[PathBuf], _thai: bool) -> Result<(), String> {
    let list = paths.iter().map(|p| ps_quote(&p.to_string_lossy())).collect::<Vec<_>>().join(",");
    elevated(&format!("Remove-Item -LiteralPath {list} -Recurse -Force -ErrorAction SilentlyContinue"))
}

/// Run a program as administrator (UAC prompt).
#[cfg(windows)]
fn admin_command(argv: &[String], _thai: bool) -> Result<(), String> {
    let args = argv.iter().map(|a| ps_quote(a)).collect::<Vec<_>>().join(" ");
    elevated(&format!("& {args}; exit $LASTEXITCODE"))
}

/// Run PowerShell code in an elevated, hidden PowerShell and wait for it.
#[cfg(windows)]
fn elevated(inner: &str) -> Result<(), String> {
    // Encode the inner command so no quoting survives two shells.
    let encoded = {
        let utf16: Vec<u8> = inner.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        base64(&utf16)
    };
    // -PassThru + exit: report the elevated command's own exit code.
    let outer = format!(
        "$p = Start-Process powershell -Verb RunAs -Wait -PassThru -WindowStyle Hidden -ArgumentList '-NoProfile','-EncodedCommand','{encoded}'; exit $p.ExitCode"
    );
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &outer])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if msg.contains("canceled") || msg.contains("cancelled") { "cancelled".into() } else { msg })
    }
}

#[cfg(windows)]
fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Delete paths as root, after macOS asks for the admin password once.
#[cfg(not(windows))]
fn admin_remove(paths: &[PathBuf], thai: bool) -> Result<(), String> {
    let list = paths.iter().map(|p| sh_quote(&p.to_string_lossy())).collect::<Vec<_>>().join(" ");
    as_root(&format!("/bin/rm -rf -- {list}"), thai)
}

#[cfg(not(windows))]
fn admin_command(argv: &[String], thai: bool) -> Result<(), String> {
    as_root(&argv.iter().map(|a| sh_quote(a)).collect::<Vec<_>>().join(" "), thai)
}

/// Run a shell command as root through the macOS password dialog.
#[cfg(not(windows))]
fn as_root(cmd: &str, thai: bool) -> Result<(), String> {
    let prompt = if thai {
        "Puri Cleaner ต้องการลบไฟล์ของระบบที่คุณเลือก"
    } else {
        "Puri Cleaner wants to delete the system files you selected."
    };
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        "do shell script \"{}\" with administrator privileges with prompt \"{}\"",
        esc(&cmd),
        esc(prompt)
    );
    let out = Command::new("/usr/bin/osascript").arg("-e").arg(script).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if msg.contains("-128") { "cancelled".into() } else { msg })
    }
}

impl App {
    pub fn new(single: Option<PathBuf>) -> Self {
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
            .map(PathBuf::from)
            .unwrap_or_else(scan::system_root);
        App {
            single,
            home,
            state: Arc::new(Mutex::new(State::default())),
            prog: Arc::new(Progress::default()),
            clean: Arc::new(Mutex::new(CleanStatus::default())),
        }
    }

    pub fn call(&self, cmd: &str, args: &Value) -> Result<Value, String> {
        match cmd {
            "scan" => self.start_scan(),
            "cancel" => {
                self.prog.cancel.store(true, Relaxed);
                Ok(json!({}))
            }
            "status" => Ok(self.status()),
            "overview" => self.overview(),
            "node" => self.node(arg_u32(args, "id").unwrap_or(0)),
            "recommendations" => Ok(json!(lock(&self.state).recs)),
            "script" => self.script(&arg_ids(args), args.get("lang").and_then(Value::as_str) != Some("en")),
            "clean" => {
                let mode = match args.get("mode").and_then(Value::as_str) {
                    Some("delete") => Mode::Delete,
                    _ => Mode::Trash,
                };
                let thai = args.get("lang").and_then(Value::as_str) != Some("en");
                self.start_clean(arg_ids(args), mode, thai)
            }
            "reveal" => self.reveal(arg_u32(args, "id").ok_or("missing id")?),
            "relaunch_admin" => relaunch_admin(),
            "open_fda" => {
                #[cfg(target_os = "macos")]
                let _ = Command::new("open")
                    .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
                    .spawn();
                Ok(json!({}))
            }
            _ => Err(format!("unknown command: {cmd}")),
        }
    }

    fn start_scan(&self) -> Result<Value, String> {
        {
            let mut st = lock(&self.state);
            if matches!(st.phase, Phase::Scanning | Phase::Cleaning) {
                return Ok(json!({ "started": false }));
            }
            st.phase = Phase::Scanning;
            st.started = Some(Instant::now());
            st.error = None;
        }
        self.prog.reset();
        let (state, prog, single, home) = (self.state.clone(), self.prog.clone(), self.single.clone(), self.home.clone());
        std::thread::spawn(move || {
            let t0 = Instant::now();
            // Simulator and app details come from other tools; fetch them
            // while the disk walk runs.
            let whole_mac = single.is_none();
            let sys_job = std::thread::spawn(move || sysinfo::gather(whole_mac));
            let result = scan::scan(single.as_deref(), &home, &prog);
            // Time the disk walk only, not the wait for DISM / simctl.
            let ms = t0.elapsed().as_millis() as u64;
            *lock(&prog.current) = "@sysinfo".into();
            let sys = sys_job.join().unwrap_or_default();
            let mut st = lock(&state);
            match result {
                Ok(tree) => {
                    st.tree = Some(tree);
                    st.sys = sys;
                    st.refresh();
                    st.scan_ms = ms;
                }
                Err(e) => st.error = Some(e.to_string()),
            }
            st.phase = if st.tree.is_some() { Phase::Ready } else { Phase::Idle };
        });
        Ok(json!({ "started": true }))
    }

    fn status(&self) -> Value {
        let st = lock(&self.state);
        json!({
            "phase": st.phase,
            "error": st.error,
            "files": self.prog.files.load(Relaxed),
            "dirs": self.prog.dirs.load(Relaxed),
            "bytes": self.prog.bytes.load(Relaxed),
            "errors": self.prog.errors.load(Relaxed),
            "current": self.short(&lock(&self.prog.current)),
            "waiting": self.prog.waiting_on().iter().map(|p| self.short(p)).collect::<Vec<_>>(),
            "elapsed_ms": st.started.map(|t| t.elapsed().as_millis() as u64).unwrap_or(0),
            "clean": *lock(&self.clean),
        })
    }

    fn short(&self, p: &str) -> String {
        match Path::new(p).strip_prefix(&self.home) {
            Ok(r) => format!("~/{}", r.display()),
            Err(_) => p.to_string(),
        }
    }

    fn overview(&self) -> Result<Value, String> {
        let st = lock(&self.state);
        let tree = st.tree.as_ref().ok_or("not scanned yet")?;
        let (total, free) = crate::disk::usage(&self.home);
        let root = tree.node(0);
        let mut cats = serde_json::Map::new();
        for (i, k) in KEYS.iter().enumerate() {
            cats.insert(k.to_string(), json!(root.cats[i]));
        }
        Ok(json!({
            "root": tree.display(0),
            "whole_mac": self.single.is_none(),
            "os": std::env::consts::OS,
            "disk_total": total,
            "disk_free": free,
            "scanned": root.size,
            "home_size": tree.node(tree.home_id).size,
            "files": root.files,
            "unreadable": tree.unreadable,
            // Folders anywhere (system ones too) that could not be read.
            "unreadable_all": tree.nodes.iter().filter(|n| n.flags & crate::tree::flag::UNREADABLE != 0).count(),
            "elevated": crate::sysinfo::is_elevated(),
            "scan_ms": st.scan_ms,
            "cats": cats,
        }))
    }

    /// Name shown on a tile: scanned roots in whole-Mac mode get their path.
    fn tile_name(&self, tree: &Tree, id: u32) -> String {
        let n = tree.node(id);
        if self.single.is_none() && n.parent == 0 {
            tree.display(id)
        } else {
            n.name.to_string()
        }
    }

    fn tile(&self, st: &State, tree: &Tree, id: u32, depth: u8) -> Value {
        let n = tree.node(id);
        let rec = st.items.get(&id);
        json!({
            "id": id,
            "name": self.tile_name(tree, id),
            "kind": if n.is_file() { "file" } else { "dir" },
            "size": n.size,
            "files": n.files,
            "cat": KEYS[tree.dominant(id) as usize],
            "status": rec.map(|r| r.1),
            "rec": rec.map(|r| st.recs[r.0].id),
            "contains": st.contains.get(&id),
            "children": if depth > 0 && !n.is_file() { self.tiles(st, tree, id, depth - 1, INNER_TILES) } else { vec![] },
        })
    }

    fn tiles(&self, st: &State, tree: &Tree, id: u32, depth: u8, limit: usize) -> Vec<Value> {
        let n = tree.node(id);
        let mut kids: Vec<u32> = n
            .children
            .iter()
            .copied()
            .filter(|&c| !tree.node(c).is_removed() && tree.node(c).size > 0)
            .collect();
        kids.sort_by_key(|&c| std::cmp::Reverse(tree.node(c).size));
        let mut out: Vec<Value> = kids.iter().take(limit).map(|&c| self.tile(st, tree, c, depth)).collect();
        if n.own > 0 {
            out.push(json!({ "kind": "files", "size": n.own, "count": n.own_count, "cat": KEYS[n.cat as usize] }));
        }
        if kids.len() > limit {
            let rest = &kids[limit..];
            let size: u64 = rest.iter().map(|&c| tree.node(c).size).sum();
            out.push(json!({ "kind": "more", "size": size, "count": rest.len(), "cat": KEYS[n.cat as usize] }));
        }
        out.sort_by_key(|v| std::cmp::Reverse(v["size"].as_u64()));
        out
    }

    fn node(&self, id: u32) -> Result<Value, String> {
        let st = lock(&self.state);
        let tree = st.tree.as_ref().ok_or("not scanned yet")?;
        if id as usize >= tree.nodes.len() || tree.node(id).is_removed() {
            return Err("this folder is gone".into());
        }
        let mut crumbs: Vec<Value> = tree
            .ancestors(id)
            .map(|a| json!({ "id": a, "name": self.tile_name(tree, a) }))
            .collect();
        crumbs.reverse();
        crumbs.push(json!({ "id": id, "name": self.tile_name(tree, id) }));
        let n = tree.node(id);
        let rec = st.items.get(&id);
        let mut children = self.tiles(&st, tree, id, 1, TOP_TILES);
        if id == 0 && self.single.is_none() {
            // Whatever the disk uses beyond what we could scan: macOS itself,
            // other volumes in the container, snapshots, unreadable folders.
            let (total, free) = crate::disk::usage(&self.home);
            let rest = total.saturating_sub(free).saturating_sub(n.size);
            if rest > 0 {
                children.push(json!({ "kind": "system", "size": rest, "cat": "sys" }));
                children.sort_by_key(|v| std::cmp::Reverse(v["size"].as_u64()));
            }
        }
        Ok(json!({
            "id": id,
            "name": self.tile_name(tree, id),
            "path": tree.display(id),
            "kind": if n.is_file() { "file" } else { "dir" },
            "size": n.size,
            "files": n.files,
            "newest": n.newest,
            "cat": KEYS[tree.dominant(id) as usize],
            "status": rec.map(|r| r.1),
            "rec": rec.map(|r| st.recs[r.0].id),
            "whole_mac": self.single.is_none(),
            "crumbs": crumbs,
            "children": children,
        }))
    }

    fn reveal(&self, id: u32) -> Result<Value, String> {
        let path = {
            let st = lock(&self.state);
            let tree = st.tree.as_ref().ok_or("not scanned yet")?;
            if id >= VIRTUAL {
                PathBuf::from(&st.item(id).ok_or("unknown item")?.path)
            } else if (id as usize) < tree.nodes.len() {
                tree.path(id)
            } else {
                return Err("unknown item".into());
            }
        };
        let target = if path.exists() { path } else { path.parent().map(Path::to_path_buf).unwrap_or(path) };
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // explorer wants exactly: /select,"C:\path"
            Command::new("explorer")
                .raw_arg(format!("/select,\"{}\"", target.display()))
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        #[cfg(not(windows))]
        Command::new("open").arg("-R").arg(&target).spawn().map_err(|e| e.to_string())?;
        Ok(json!({}))
    }

    /// The PowerShell line that removes one item.
    #[cfg(windows)]
    fn shell_line(&self, tree: &Tree, it: &Item) -> String {
        let path = |id| {
            let p = tree.path(id).to_string_lossy().into_owned();
            let home = self.home.to_string_lossy().into_owned();
            match p.strip_prefix(&home) {
                Some(rest) => format!("\"$env:USERPROFILE{}\"", rest.replace('`', "``").replace('$', "`$").replace('"', "`\"")),
                None => ps_quote(&p),
            }
        };
        match &it.action {
            Action::Command { argv } => argv.iter().map(|a| if a.contains(' ') { ps_quote(a) } else { a.clone() }).collect::<Vec<_>>().join(" "),
            Action::EmptyRecycleBin => "Clear-RecycleBin -Force".into(),
            Action::AdminCommand { argv } => format!("{}   # Run as administrator", argv.join(" ")),
            Action::Admin => format!("Remove-Item -LiteralPath {} -Recurse -Force   # Run as administrator", path(it.id)),
            Action::Remove | Action::Finder => format!("Remove-Item -LiteralPath {} -Recurse -Force", path(it.id)),
        }
    }

    /// The shell line that removes one item.
    #[cfg(not(windows))]
    fn shell_line(&self, tree: &Tree, it: &Item) -> String {
        let quote = |p: &Path| {
            let s = p.to_string_lossy();
            match s.strip_prefix(&*self.home.to_string_lossy()) {
                Some(rest) => format!("\"$HOME\"{}", sh_quote(rest)),
                None => sh_quote(&s),
            }
        };
        match &it.action {
            Action::Command { argv } => argv.iter().map(|a| if a.contains(' ') { sh_quote(a) } else { a.clone() }).collect::<Vec<_>>().join(" "),
            Action::Admin => format!("sudo rm -rf -- {}", quote(&tree.path(it.id))),
            Action::EmptyRecycleBin => "# (Windows only)".into(),
            Action::AdminCommand { argv } => format!("sudo {}", argv.iter().map(|a| sh_quote(a)).collect::<Vec<_>>().join(" ")),
            Action::Remove | Action::Finder => format!("rm -rf -- {}", quote(&tree.path(it.id))),
        }
    }

    fn script(&self, ids: &[u32], thai: bool) -> Result<Value, String> {
        let st = lock(&self.state);
        let tree = st.tree.as_ref().ok_or("not scanned yet")?;
        let mut groups: Vec<ScriptGroup> = Vec::new();
        for r in &st.recs {
            let items: Vec<(String, u64)> = r
                .items
                .iter()
                .filter(|it| ids.contains(&it.id))
                .map(|it| (self.shell_line(tree, it), it.size))
                .collect();
            if items.is_empty() {
                continue;
            }
            groups.push(ScriptGroup {
                title: if thai { r.title_th } else { r.title_en },
                command: r.command,
                size: items.iter().map(|i| i.1).sum(),
                items,
            });
        }
        Ok(json!({ "script": clean::script(&groups, thai) }))
    }

    fn start_clean(&self, ids: Vec<u32>, mode: Mode, thai: bool) -> Result<Value, String> {
        struct Job {
            id: u32,
            path: PathBuf,
            size: u64,
            files: u64,
            action: Action,
        }
        let jobs: Vec<Job> = {
            let mut st = lock(&self.state);
            if st.phase != Phase::Ready {
                return Err("busy".into());
            }
            let tree = st.tree.as_ref().ok_or("not scanned yet")?;
            let mut jobs = Vec::new();
            for id in ids {
                // Only things this app itself recommended can be removed.
                let Some(it) = st.item(id) else { continue };
                let path = if id >= VIRTUAL { PathBuf::from(&it.path) } else { tree.path(id) };
                if id < VIRTUAL {
                    // Never a scanned root (home, /Library, ...) or anything above one.
                    let n = tree.node(id);
                    if id == 0 || n.parent == 0 || path == self.home || !path.starts_with(&tree.root) {
                        continue;
                    }
                }
                let files = if id < VIRTUAL { tree.node(id).files } else { 0 };
                jobs.push(Job { id, path, size: it.size, files, action: it.action.clone() });
            }
            if jobs.is_empty() {
                return Err("nothing to clean".into());
            }
            st.phase = Phase::Cleaning;
            jobs
        };
        *lock(&self.clean) = CleanStatus {
            running: true,
            mode: if mode == Mode::Trash { "trash" } else { "delete" },
            total: jobs.len(),
            ..Default::default()
        };
        let (state, status, home, whole_mac) = (self.state.clone(), self.clean.clone(), self.home.clone(), self.single.is_none());
        std::thread::spawn(move || {
            let mut removed = Vec::new();
            let mut partial = Vec::new();
            let mut moved = (0u64, 0u64);

            // System-owned items: one password prompt for all of them.
            let admin: Vec<&Job> = jobs.iter().filter(|j| j.action == Action::Admin).collect();
            if !admin.is_empty() {
                lock(&status).current = admin[0].path.display().to_string();
                let paths: Vec<PathBuf> = admin.iter().map(|j| j.path.clone()).collect();
                let result = admin_remove(&paths, thai);
                for j in &admin {
                    if j.path.symlink_metadata().is_err() {
                        removed.push(j.id);
                        lock(&status).freed += j.size;
                    } else if let Err(e) = &result {
                        lock(&status).errors.push(format!("{}: {e}", j.path.display()));
                    }
                    lock(&status).done += 1;
                }
            }

            for j in jobs.iter().filter(|j| j.action != Action::Admin) {
                lock(&status).current = j.path.display().to_string();
                let result = match &j.action {
                    Action::Command { argv } => Command::new(&argv[0])
                        .args(&argv[1..])
                        .output()
                        .map_err(|e| e.to_string())
                        .and_then(|o| {
                            if o.status.success() {
                                Ok(())
                            } else {
                                Err(String::from_utf8_lossy(&o.stderr).trim().to_string())
                            }
                        }),
                    Action::Finder => clean::trash_with_finder(&j.path),
                    Action::EmptyRecycleBin => crate::disk::empty_recycle_bin(),
                    // A tool can report success and still leave files behind; check.
                    Action::AdminCommand { argv } => admin_command(argv, thai).and_then(|()| {
                        if j.id < VIRTUAL && j.path.symlink_metadata().is_ok() {
                            Err("still there after the command".into())
                        } else {
                            Ok(())
                        }
                    }),
                    _ => clean::remove(&j.path, mode),
                };
                let to_trash = matches!(j.action, Action::Finder) || (j.action == Action::Remove && mode == Mode::Trash);
                match result {
                    Ok(()) => {
                        removed.push(j.id);
                        if to_trash {
                            moved.0 += j.size;
                            moved.1 += j.files;
                            lock(&status).moved += j.size;
                        } else {
                            lock(&status).freed += j.size;
                        }
                    }
                    Err(e) => {
                        if j.id < VIRTUAL {
                            // Something may still have been deleted; measure what is left.
                            let (left, left_files) = scan::measure(&j.path);
                            if !to_trash {
                                lock(&status).freed += j.size.saturating_sub(left);
                            }
                            if j.path.symlink_metadata().is_err() {
                                removed.push(j.id);
                            } else {
                                partial.push((j.id, left, left_files));
                            }
                        }
                        lock(&status).errors.push(format!("{}: {e}", j.path.display()));
                    }
                }
                lock(&status).done += 1;
            }

            let sys = sysinfo::gather(whole_mac);
            let mut st = lock(&state);
            st.sys = sys;
            if let Some(tree) = st.tree.as_mut() {
                for id in removed.into_iter().filter(|&id| id < VIRTUAL) {
                    tree.remove(id);
                }
                for (id, left, files) in partial {
                    tree.set_size(id, left, files);
                }
                if moved.0 > 0 {
                    if let Some(t) = tree.lookup_abs(&home.join(".Trash")) {
                        let (s, f) = (tree.node(t).size, tree.node(t).files);
                        tree.set_size(t, s + moved.0, f + moved.1);
                    }
                }
            }
            st.refresh();
            st.phase = Phase::Ready;
            let mut cs = lock(&status);
            cs.running = false;
            cs.finished = true;
            cs.current.clear();
        });
        Ok(json!({ "started": true }))
    }
}
